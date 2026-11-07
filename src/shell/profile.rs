use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, Metadata},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

use atomic_write_file::AtomicWriteFile;

use super::{
    DetectedShell, ProfileIntegration, ProfileIntegrationAnalysis, ShellError, ShellKind,
    integration::analyze_profile_contents, managed_block,
};

#[derive(Debug, Clone, Default)]
pub struct ProfileEnvironment {
    pub home_dir: Option<PathBuf>,
    pub xdg_config_home: Option<PathBuf>,
}

impl ProfileEnvironment {
    pub fn from_process() -> Self {
        Self {
            home_dir: dirs::home_dir(),
            xdg_config_home: env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        }
    }
}

pub trait PowerShellProfileQuery {
    fn query(&self, executable_hint: Option<&OsStr>) -> Result<PathBuf, ShellError>;
}

#[derive(Debug, Default)]
pub struct NativePowerShellProfileQuery;

impl PowerShellProfileQuery for NativePowerShellProfileQuery {
    fn query(&self, executable_hint: Option<&OsStr>) -> Result<PathBuf, ShellError> {
        let mut candidates = Vec::<OsString>::new();
        if let Some(hint) = executable_hint.filter(|hint| is_powershell_name(hint)) {
            candidates.push(hint.to_os_string());
        }
        for fallback in ["pwsh", "powershell"] {
            if !candidates.iter().any(|candidate| candidate == fallback) {
                candidates.push(fallback.into());
            }
        }

        let mut failures = Vec::new();
        for candidate in candidates {
            let result = Command::new(&candidate)
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); [Console]::Write($PROFILE.CurrentUserCurrentHost)",
                ])
                .output();
            match result {
                Ok(output) if output.status.success() => {
                    let value = String::from_utf8(output.stdout).map_err(|_| {
                        ShellError::PowerShellProfileUnavailable(format!(
                            "{} returned a non-UTF-8 path",
                            candidate.to_string_lossy()
                        ))
                    })?;
                    let value = value.trim_end_matches(['\r', '\n']);
                    if !value.is_empty() {
                        return Ok(PathBuf::from(value));
                    }
                    failures.push(format!(
                        "{} returned an empty path",
                        candidate.to_string_lossy()
                    ));
                }
                Ok(output) => failures.push(format!(
                    "{} exited with {}",
                    candidate.to_string_lossy(),
                    output.status
                )),
                Err(error) => failures.push(format!(
                    "{} could not be started: {error}",
                    candidate.to_string_lossy()
                )),
            }
        }
        Err(ShellError::PowerShellProfileUnavailable(
            failures.join("; "),
        ))
    }
}

pub fn resolve_profile_path(
    detected: &DetectedShell,
    explicit_profile: Option<&Path>,
    environment: &ProfileEnvironment,
    powershell: &dyn PowerShellProfileQuery,
) -> Result<PathBuf, ShellError> {
    if let Some(path) = explicit_profile {
        if path.as_os_str().is_empty() {
            return Err(ShellError::InvalidProfilePath(path.to_path_buf()));
        }
        return Ok(path.to_path_buf());
    }

    match detected.kind {
        ShellKind::Bash => environment
            .home_dir
            .as_ref()
            .map(|home| home.join(".bashrc"))
            .ok_or(ShellError::MissingHome(ShellKind::Bash)),
        ShellKind::Zsh => environment
            .home_dir
            .as_ref()
            .map(|home| home.join(".zshrc"))
            .ok_or(ShellError::MissingHome(ShellKind::Zsh)),
        ShellKind::Fish => {
            let root = environment
                .xdg_config_home
                .as_ref()
                .filter(|root| root.is_absolute())
                .cloned()
                .or_else(|| {
                    environment
                        .home_dir
                        .as_ref()
                        .map(|home| home.join(".config"))
                })
                .ok_or(ShellError::MissingHome(ShellKind::Fish))?;
            Ok(root.join("fish").join("config.fish"))
        }
        ShellKind::Powershell => powershell.query(detected.parent_process.as_deref()),
    }
}

fn is_powershell_name(value: &OsStr) -> bool {
    let value = value.to_string_lossy();
    let name = value
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&value)
        .to_ascii_lowercase();
    matches!(
        name.strip_suffix(".exe").unwrap_or(&name),
        "pwsh" | "powershell"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallAction {
    AddManaged,
    ReplaceManaged,
    AlreadyManaged,
    AlreadyManual,
    BlockedLegacyManual,
    BlockedDuplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallAction {
    RemoveManaged,
    NoManagedBlock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOutcome {
    pub action: InstallAction,
    pub analysis: ProfileIntegrationAnalysis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallOutcome {
    pub action: UninstallAction,
    pub analysis: ProfileIntegrationAnalysis,
}

pub fn profile_analysis(
    path: &Path,
    shell: ShellKind,
) -> Result<ProfileIntegrationAnalysis, ShellError> {
    inspect(path, shell).map(|profile| profile.analysis)
}

pub fn install_profile(
    path: &Path,
    shell: ShellKind,
    dry_run: bool,
) -> Result<InstallOutcome, ShellError> {
    let profile = inspect(path, shell)?;
    let action = match profile.analysis.integration() {
        ProfileIntegration::NotInstalled => InstallAction::AddManaged,
        ProfileIntegration::ManagedOutdated => InstallAction::ReplaceManaged,
        ProfileIntegration::ManagedCurrent => InstallAction::AlreadyManaged,
        ProfileIntegration::ManualCurrent => InstallAction::AlreadyManual,
        ProfileIntegration::ManualLegacy => InstallAction::BlockedLegacyManual,
        ProfileIntegration::Duplicate => InstallAction::BlockedDuplicate,
    };
    if dry_run
        || !matches!(
            action,
            InstallAction::AddManaged | InstallAction::ReplaceManaged
        )
    {
        return Ok(InstallOutcome {
            action,
            analysis: profile.analysis,
        });
    }
    ensure_writable(&profile)?;

    let block = managed_block(shell, profile.line_ending);
    let contents = match profile.analysis.managed_range.clone() {
        Some(range) => {
            let mut replacement = block;
            if profile.contents[range.clone()].ends_with('\n') {
                replacement.push_str(profile.line_ending);
            }
            let mut contents = profile.contents.clone();
            contents.replace_range(range, &replacement);
            contents
        }
        None => format!(
            "{block}{}{contents}",
            profile.line_ending,
            contents = profile.contents
        ),
    };
    write_profile(&profile, contents.as_bytes())?;
    Ok(InstallOutcome {
        action,
        analysis: profile.analysis,
    })
}

pub fn uninstall_profile(
    path: &Path,
    shell: ShellKind,
    dry_run: bool,
) -> Result<UninstallOutcome, ShellError> {
    let profile = inspect(path, shell)?;
    let Some(range) = profile.analysis.managed_range.clone() else {
        return Ok(UninstallOutcome {
            action: UninstallAction::NoManagedBlock,
            analysis: profile.analysis,
        });
    };
    if dry_run {
        return Ok(UninstallOutcome {
            action: UninstallAction::RemoveManaged,
            analysis: profile.analysis,
        });
    }
    ensure_writable(&profile)?;
    let mut contents = profile.contents.clone();
    contents.replace_range(range, "");
    write_profile(&profile, contents.as_bytes())?;
    Ok(UninstallOutcome {
        action: UninstallAction::RemoveManaged,
        analysis: profile.analysis,
    })
}

#[derive(Debug)]
struct InspectedProfile {
    display_path: PathBuf,
    target_path: PathBuf,
    metadata: Option<Metadata>,
    contents: String,
    line_ending: &'static str,
    analysis: ProfileIntegrationAnalysis,
}

fn inspect(path: &Path, shell: ShellKind) -> Result<InspectedProfile, ShellError> {
    let (target_path, metadata, bytes) = read_profile(path)?;
    let contents = String::from_utf8(bytes)
        .map_err(|_| ShellError::InvalidProfileEncoding(path.to_path_buf()))?;
    let line_ending = detect_line_ending(&contents);
    let analysis = analyze_profile_contents(path, shell, &contents, line_ending)?;
    Ok(InspectedProfile {
        display_path: path.to_path_buf(),
        target_path,
        metadata,
        contents,
        line_ending,
        analysis,
    })
}

fn read_profile(path: &Path) -> Result<(PathBuf, Option<Metadata>, Vec<u8>), ShellError> {
    match fs::symlink_metadata(path) {
        Ok(link_metadata) if link_metadata.file_type().is_symlink() => {
            let target = fs::canonicalize(path)
                .map_err(|_| ShellError::DanglingProfileSymlink(path.to_path_buf()))?;
            let metadata = fs::metadata(&target).map_err(|source| ShellError::Io {
                action: "inspect resolved shell profile",
                path: target.clone(),
                source,
            })?;
            if !metadata.is_file() {
                return Err(ShellError::ProfileNotFile(path.to_path_buf()));
            }
            let bytes = fs::read(&target).map_err(|source| ShellError::Io {
                action: "read shell profile",
                path: path.to_path_buf(),
                source,
            })?;
            Ok((target, Some(metadata), bytes))
        }
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(ShellError::ProfileNotFile(path.to_path_buf()));
            }
            let bytes = fs::read(path).map_err(|source| ShellError::Io {
                action: "read shell profile",
                path: path.to_path_buf(),
                source,
            })?;
            Ok((path.to_path_buf(), Some(metadata), bytes))
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            Ok((path.to_path_buf(), None, Vec::new()))
        }
        Err(source) => Err(ShellError::Io {
            action: "inspect shell profile",
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn detect_line_ending(contents: &str) -> &'static str {
    match contents.find('\n') {
        Some(index) if index > 0 && contents.as_bytes()[index - 1] == b'\r' => "\r\n",
        Some(_) => "\n",
        None if cfg!(windows) => "\r\n",
        None => "\n",
    }
}

fn ensure_writable(profile: &InspectedProfile) -> Result<(), ShellError> {
    if profile
        .metadata
        .as_ref()
        .is_some_and(|metadata| metadata.permissions().readonly())
    {
        return Err(ShellError::ReadOnlyProfile(profile.display_path.clone()));
    }
    Ok(())
}

fn write_profile(profile: &InspectedProfile, contents: &[u8]) -> Result<(), ShellError> {
    let parent = profile
        .target_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| ShellError::Io {
        action: "create shell profile directory",
        path: parent.to_path_buf(),
        source,
    })?;
    let mut file =
        AtomicWriteFile::open(&profile.target_path).map_err(|source| ShellError::Io {
            action: "create shell profile staging file",
            path: profile.display_path.clone(),
            source,
        })?;
    if let Some(metadata) = &profile.metadata {
        file.as_file()
            .set_permissions(metadata.permissions())
            .map_err(|source| ShellError::Io {
                action: "preserve shell profile permissions",
                path: profile.display_path.clone(),
                source,
            })?;
    }
    file.write_all(contents).map_err(|source| ShellError::Io {
        action: "write shell profile staging file",
        path: profile.display_path.clone(),
        source,
    })?;
    file.commit().map_err(|source| ShellError::Io {
        action: "replace shell profile atomically",
        path: profile.display_path.clone(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{
        DetectionSource, MANAGED_BLOCK_END, MANAGED_BLOCK_START, legacy_manual_loader,
        manual_loader,
    };
    use tempfile::TempDir;

    struct FixedPowerShell(PathBuf);

    impl PowerShellProfileQuery for FixedPowerShell {
        fn query(&self, _executable_hint: Option<&OsStr>) -> Result<PathBuf, ShellError> {
            Ok(self.0.clone())
        }
    }

    fn detected(kind: ShellKind) -> DetectedShell {
        DetectedShell {
            kind,
            source: DetectionSource::Explicit,
            parent_process: None,
        }
    }

    #[test]
    fn resolves_default_and_explicit_profiles_without_shell_parsing() {
        let temp = TempDir::new().unwrap();
        let home = temp.path().join("person with spaces/λ");
        let xdg = temp.path().join("custom config");
        let environment = ProfileEnvironment {
            home_dir: Some(home.clone()),
            xdg_config_home: Some(xdg.clone()),
        };
        let query = FixedPowerShell(PathBuf::from("C:\\Users\\Person\\profile.ps1"));
        assert_eq!(
            resolve_profile_path(&detected(ShellKind::Bash), None, &environment, &query).unwrap(),
            home.join(".bashrc")
        );
        assert_eq!(
            resolve_profile_path(&detected(ShellKind::Zsh), None, &environment, &query).unwrap(),
            home.join(".zshrc")
        );
        assert_eq!(
            resolve_profile_path(&detected(ShellKind::Fish), None, &environment, &query).unwrap(),
            xdg.join("fish/config.fish")
        );
        assert_eq!(
            resolve_profile_path(&detected(ShellKind::Powershell), None, &environment, &query)
                .unwrap(),
            PathBuf::from("C:\\Users\\Person\\profile.ps1")
        );
        let explicit = PathBuf::from("profiles/custom profile's λ");
        assert_eq!(
            resolve_profile_path(
                &detected(ShellKind::Bash),
                Some(&explicit),
                &ProfileEnvironment::default(),
                &query
            )
            .unwrap(),
            explicit
        );
    }

    #[test]
    fn fish_ignores_relative_xdg_root_and_falls_back_to_home() {
        let environment = ProfileEnvironment {
            home_dir: Some(PathBuf::from("/home/me")),
            xdg_config_home: Some(PathBuf::from("relative")),
        };
        assert_eq!(
            resolve_profile_path(
                &detected(ShellKind::Fish),
                None,
                &environment,
                &FixedPowerShell(PathBuf::new())
            )
            .unwrap(),
            PathBuf::from("/home/me/.config/fish/config.fish")
        );
    }

    #[test]
    fn installs_new_nested_profile_and_is_byte_idempotent() {
        let temp = TempDir::new().unwrap();
        let profile = temp.path().join("nested/配置/profile");
        assert_eq!(
            install_profile(&profile, ShellKind::Bash, false)
                .unwrap()
                .action,
            InstallAction::AddManaged
        );
        let first = fs::read(&profile).unwrap();
        assert_eq!(
            install_profile(&profile, ShellKind::Bash, false)
                .unwrap()
                .action,
            InstallAction::AlreadyManaged
        );
        assert_eq!(fs::read(&profile).unwrap(), first);
        assert_eq!(
            profile_analysis(&profile, ShellKind::Bash)
                .unwrap()
                .integration(),
            ProfileIntegration::ManagedCurrent
        );
    }

    #[test]
    fn install_preserves_unrelated_content_and_missing_final_newline() {
        let temp = TempDir::new().unwrap();
        let profile = temp.path().join("profile");
        let original = "before\nafter-without-newline";
        fs::write(&profile, original).unwrap();
        install_profile(&profile, ShellKind::Zsh, false).unwrap();
        let installed = fs::read_to_string(&profile).unwrap();
        assert!(installed.starts_with(MANAGED_BLOCK_START));
        assert!(installed.ends_with(original));
        uninstall_profile(&profile, ShellKind::Zsh, false).unwrap();
        assert_eq!(fs::read_to_string(&profile).unwrap(), original);
    }

    #[test]
    fn replaces_only_an_outdated_block_in_the_middle() {
        let temp = TempDir::new().unwrap();
        let profile = temp.path().join("profile");
        fs::write(
            &profile,
            format!("before\n{MANAGED_BLOCK_START}\nold command\n{MANAGED_BLOCK_END}\nafter\n"),
        )
        .unwrap();
        assert_eq!(
            install_profile(&profile, ShellKind::Fish, false)
                .unwrap()
                .action,
            InstallAction::ReplaceManaged
        );
        let value = fs::read_to_string(&profile).unwrap();
        assert!(value.starts_with("before\n"));
        assert!(value.ends_with("after\n"));
        assert!(value.contains("command dstack shell __init fish | source"));
        assert!(!value.contains("old command"));
    }

    #[test]
    fn preserves_crlf_when_adding_and_replacing_blocks() {
        let temp = TempDir::new().unwrap();
        let profile = temp.path().join("profile");
        fs::write(&profile, b"one\r\ntwo\r\n").unwrap();
        install_profile(&profile, ShellKind::Bash, false).unwrap();
        let first = fs::read(&profile).unwrap();
        assert!(first.windows(2).any(|pair| pair == b"\r\n"));
        assert!(
            !first
                .windows(2)
                .any(|pair| pair[0] != b'\r' && pair[1] == b'\n')
        );

        let old =
            format!("before\r\n{MANAGED_BLOCK_START}\r\nold\r\n{MANAGED_BLOCK_END}\r\nafter\r\n");
        fs::write(&profile, old).unwrap();
        install_profile(&profile, ShellKind::Zsh, false).unwrap();
        let replaced = fs::read(&profile).unwrap();
        assert!(
            !replaced
                .windows(2)
                .any(|pair| pair[0] != b'\r' && pair[1] == b'\n')
        );
    }

    #[test]
    fn dry_run_reports_actions_without_creating_or_changing_files() {
        let temp = TempDir::new().unwrap();
        let missing = temp.path().join("missing/parent/profile");
        assert_eq!(
            install_profile(&missing, ShellKind::Bash, true)
                .unwrap()
                .action,
            InstallAction::AddManaged
        );
        assert!(!missing.exists());

        let profile = temp.path().join("existing");
        fs::write(
            &profile,
            format!("{MANAGED_BLOCK_START}\nold\n{MANAGED_BLOCK_END}\n"),
        )
        .unwrap();
        let before = fs::read(&profile).unwrap();
        assert_eq!(
            install_profile(&profile, ShellKind::Bash, true)
                .unwrap()
                .action,
            InstallAction::ReplaceManaged
        );
        assert_eq!(
            uninstall_profile(&profile, ShellKind::Bash, true)
                .unwrap()
                .action,
            UninstallAction::RemoveManaged
        );
        assert_eq!(fs::read(&profile).unwrap(), before);
    }

    #[test]
    fn malformed_marker_forms_fail_without_modification() {
        let temp = TempDir::new().unwrap();
        let forms = [
            format!("{MANAGED_BLOCK_START}\nvalue\n"),
            format!("value\n{MANAGED_BLOCK_END}\n"),
            format!(
                "{MANAGED_BLOCK_START}\n{MANAGED_BLOCK_END}\n{MANAGED_BLOCK_START}\n{MANAGED_BLOCK_END}\n"
            ),
            format!(
                "{MANAGED_BLOCK_START}\n{MANAGED_BLOCK_START}\n{MANAGED_BLOCK_END}\n{MANAGED_BLOCK_END}\n"
            ),
        ];
        for (index, value) in forms.iter().enumerate() {
            let profile = temp.path().join(format!("profile-{index}"));
            fs::write(&profile, value).unwrap();
            let before = fs::read(&profile).unwrap();
            assert!(install_profile(&profile, ShellKind::Bash, false).is_err());
            assert!(uninstall_profile(&profile, ShellKind::Bash, false).is_err());
            assert_eq!(fs::read(&profile).unwrap(), before);
        }
    }

    #[test]
    fn uninstall_is_idempotent_preserves_content_and_keeps_empty_profile() {
        let temp = TempDir::new().unwrap();
        let profile = temp.path().join("profile");
        fs::write(&profile, "user line\n").unwrap();
        install_profile(&profile, ShellKind::Bash, false).unwrap();
        assert_eq!(
            uninstall_profile(&profile, ShellKind::Bash, false)
                .unwrap()
                .action,
            UninstallAction::RemoveManaged
        );
        assert_eq!(fs::read_to_string(&profile).unwrap(), "user line\n");
        assert_eq!(
            uninstall_profile(&profile, ShellKind::Bash, false)
                .unwrap()
                .action,
            UninstallAction::NoManagedBlock
        );

        let empty = temp.path().join("empty");
        install_profile(&empty, ShellKind::Bash, false).unwrap();
        uninstall_profile(&empty, ShellKind::Bash, false).unwrap();
        assert!(empty.is_file());
        assert!(fs::read(&empty).unwrap().is_empty());
    }

    #[test]
    fn canonical_manual_installations_are_byte_idempotent_for_every_shell() {
        let temp = TempDir::new().unwrap();
        for shell in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::Fish,
            ShellKind::Powershell,
        ] {
            let profile = temp.path().join(format!("manual-{shell}"));
            let original = format!(
                "export BEFORE=1\n{}\nexport AFTER=1\n",
                manual_loader(shell, "\n")
            );
            fs::write(&profile, &original).unwrap();
            let before = fs::read(&profile).unwrap();
            let outcome = install_profile(&profile, shell, false).unwrap();
            assert_eq!(outcome.action, InstallAction::AlreadyManual);
            assert_eq!(
                outcome.analysis.integration(),
                ProfileIntegration::ManualCurrent
            );
            assert_eq!(fs::read(&profile).unwrap(), before);
            assert!(!original.contains(MANAGED_BLOCK_START));
            assert_eq!(original.matches(&manual_loader(shell, "\n")).count(), 1);
        }
    }

    #[test]
    fn legacy_and_duplicate_installations_are_blocked_before_writing() {
        let temp = TempDir::new().unwrap();
        for shell in [ShellKind::Bash, ShellKind::Zsh] {
            let profile = temp.path().join(format!("legacy-{shell}"));
            fs::write(&profile, format!("{}\n", legacy_manual_loader(shell))).unwrap();
            let before = fs::read(&profile).unwrap();
            let outcome = install_profile(&profile, shell, false).unwrap();
            assert_eq!(outcome.action, InstallAction::BlockedLegacyManual);
            assert_eq!(fs::read(&profile).unwrap(), before);
        }

        let cases = [
            format!(
                "{}\n{}\n",
                manual_loader(ShellKind::Bash, "\n"),
                manual_loader(ShellKind::Bash, "\n")
            ),
            format!(
                "{}\n{}\n",
                manual_loader(ShellKind::Bash, "\n"),
                legacy_manual_loader(ShellKind::Bash)
            ),
            format!(
                "{}\n{}\n",
                managed_block(ShellKind::Bash, "\n"),
                manual_loader(ShellKind::Bash, "\n")
            ),
        ];
        for (index, contents) in cases.iter().enumerate() {
            let profile = temp.path().join(format!("duplicate-{index}"));
            fs::write(&profile, contents).unwrap();
            let before = fs::read(&profile).unwrap();
            let outcome = install_profile(&profile, ShellKind::Bash, false).unwrap();
            assert_eq!(outcome.action, InstallAction::BlockedDuplicate);
            assert_eq!(
                outcome.analysis.integration(),
                ProfileIntegration::Duplicate
            );
            assert_eq!(fs::read(&profile).unwrap(), before);
        }
    }

    #[test]
    fn uninstall_removes_only_managed_content_and_never_manual_loaders() {
        let temp = TempDir::new().unwrap();
        let manual = manual_loader(ShellKind::Bash, "\n");
        let manual_only = temp.path().join("manual-only");
        fs::write(&manual_only, format!("before\n{manual}\nafter\n")).unwrap();
        let before = fs::read(&manual_only).unwrap();
        let outcome = uninstall_profile(&manual_only, ShellKind::Bash, false).unwrap();
        assert_eq!(outcome.action, UninstallAction::NoManagedBlock);
        assert_eq!(fs::read(&manual_only).unwrap(), before);

        let mixed = temp.path().join("managed-and-manual");
        fs::write(
            &mixed,
            format!(
                "{}\nbefore\n{manual}\nafter\n",
                managed_block(ShellKind::Bash, "\n")
            ),
        )
        .unwrap();
        let outcome = uninstall_profile(&mixed, ShellKind::Bash, false).unwrap();
        assert_eq!(outcome.action, UninstallAction::RemoveManaged);
        assert!(outcome.analysis.has_manual_integration());
        assert_eq!(
            fs::read_to_string(&mixed).unwrap(),
            format!("before\n{manual}\nafter\n")
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_target_and_permissions_are_preserved() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temp = TempDir::new().unwrap();
        let target = temp.path().join("real dotfile");
        let profile = temp.path().join("profile link");
        fs::write(&target, "user\n").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
        symlink(&target, &profile).unwrap();

        install_profile(&profile, ShellKind::Bash, false).unwrap();
        assert!(
            fs::symlink_metadata(&profile)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o640
        );
        uninstall_profile(&profile, ShellKind::Bash, false).unwrap();
        assert!(
            fs::symlink_metadata(&profile)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "user\n");
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_and_read_only_profile_fail_without_changes() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temp = TempDir::new().unwrap();
        let dangling = temp.path().join("dangling");
        symlink(temp.path().join("absent"), &dangling).unwrap();
        assert!(matches!(
            install_profile(&dangling, ShellKind::Bash, false),
            Err(ShellError::DanglingProfileSymlink(_))
        ));

        let read_only = temp.path().join("read-only");
        fs::write(&read_only, "user\n").unwrap();
        fs::set_permissions(&read_only, fs::Permissions::from_mode(0o444)).unwrap();
        let before = fs::read(&read_only).unwrap();
        assert!(matches!(
            install_profile(&read_only, ShellKind::Bash, false),
            Err(ShellError::ReadOnlyProfile(_))
        ));
        assert_eq!(fs::read(&read_only).unwrap(), before);
    }
}
