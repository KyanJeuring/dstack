use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, Metadata},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

use atomic_write_file::AtomicWriteFile;

use super::{DetectedShell, ShellError, ShellKind};

pub const MANAGED_BLOCK_START: &str = "# >>> DStack shell integration >>>";
pub const MANAGED_BLOCK_END: &str = "# <<< DStack shell integration <<<";

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

pub fn managed_block(shell: ShellKind, line_ending: &str) -> String {
    let body = match shell {
        ShellKind::Bash => [
            MANAGED_BLOCK_START,
            "if command -v dstack >/dev/null 2>&1; then",
            "  eval \"$(command dstack shell __init bash)\"",
            "fi",
            MANAGED_BLOCK_END,
        ]
        .as_slice(),
        ShellKind::Zsh => [
            MANAGED_BLOCK_START,
            "if command -v dstack >/dev/null 2>&1; then",
            "  eval \"$(command dstack shell __init zsh)\"",
            "fi",
            MANAGED_BLOCK_END,
        ]
        .as_slice(),
        ShellKind::Fish => [
            MANAGED_BLOCK_START,
            "if type -q dstack",
            "    command dstack shell __init fish | source",
            "end",
            MANAGED_BLOCK_END,
        ]
        .as_slice(),
        ShellKind::Powershell => [
            MANAGED_BLOCK_START,
            "& {",
            "  $dstackApplication = (Get-Command dstack -CommandType Application -ErrorAction SilentlyContinue).Source",
            "  if ($null -ne $dstackApplication) {",
            "    Invoke-Expression ((& $dstackApplication shell __init powershell) -join \"`n\")",
            "  }",
            "}",
            MANAGED_BLOCK_END,
        ]
        .as_slice(),
    };
    body.join(line_ending)
}

pub fn manual_loader(shell: ShellKind, line_ending: &str) -> String {
    match shell {
        ShellKind::Bash => "eval \"$(command dstack shell __init bash)\"".to_owned(),
        ShellKind::Zsh => "eval \"$(command dstack shell __init zsh)\"".to_owned(),
        ShellKind::Fish => "command dstack shell __init fish | source".to_owned(),
        ShellKind::Powershell => [
            "& {",
            "  $dstackApplication = (Get-Command dstack -CommandType Application -ErrorAction Stop).Source",
            "  Invoke-Expression ((& $dstackApplication shell __init powershell) -join \"`n\")",
            "}",
        ]
        .join(line_ending),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileIntegration {
    Missing,
    Current,
    Outdated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallAction {
    Add,
    Replace,
    NoOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallAction {
    Remove,
    NoOp,
}

pub fn profile_status(path: &Path, shell: ShellKind) -> Result<ProfileIntegration, ShellError> {
    inspect(path, shell).map(|profile| profile.integration)
}

pub fn install_profile(
    path: &Path,
    shell: ShellKind,
    dry_run: bool,
) -> Result<InstallAction, ShellError> {
    let profile = inspect(path, shell)?;
    let action = match profile.integration {
        ProfileIntegration::Missing => InstallAction::Add,
        ProfileIntegration::Outdated => InstallAction::Replace,
        ProfileIntegration::Current => InstallAction::NoOp,
    };
    if dry_run || action == InstallAction::NoOp {
        return Ok(action);
    }
    ensure_writable(&profile)?;

    let block = managed_block(shell, profile.line_ending);
    let contents = match profile.block.clone() {
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
    Ok(action)
}

pub fn uninstall_profile(path: &Path, dry_run: bool) -> Result<UninstallAction, ShellError> {
    let profile = inspect_without_expected(path)?;
    let Some(range) = profile.block.clone() else {
        return Ok(UninstallAction::NoOp);
    };
    if dry_run {
        return Ok(UninstallAction::Remove);
    }
    ensure_writable(&profile)?;
    let mut contents = profile.contents.clone();
    contents.replace_range(range, "");
    write_profile(&profile, contents.as_bytes())?;
    Ok(UninstallAction::Remove)
}

#[derive(Debug)]
struct InspectedProfile {
    display_path: PathBuf,
    target_path: PathBuf,
    metadata: Option<Metadata>,
    contents: String,
    line_ending: &'static str,
    block: Option<std::ops::Range<usize>>,
    integration: ProfileIntegration,
}

fn inspect(path: &Path, shell: ShellKind) -> Result<InspectedProfile, ShellError> {
    let mut profile = inspect_without_expected(path)?;
    profile.integration = match profile.block.clone() {
        None => ProfileIntegration::Missing,
        Some(range) => {
            let mut expected = managed_block(shell, profile.line_ending);
            if profile.contents[range.clone()].ends_with('\n') {
                expected.push_str(profile.line_ending);
            }
            if profile.contents[range] == expected {
                ProfileIntegration::Current
            } else {
                ProfileIntegration::Outdated
            }
        }
    };
    Ok(profile)
}

fn inspect_without_expected(path: &Path) -> Result<InspectedProfile, ShellError> {
    let (target_path, metadata, bytes) = read_profile(path)?;
    let contents = String::from_utf8(bytes)
        .map_err(|_| ShellError::InvalidProfileEncoding(path.to_path_buf()))?;
    let block = parse_managed_block(path, &contents)?;
    let line_ending = detect_line_ending(&contents);
    Ok(InspectedProfile {
        display_path: path.to_path_buf(),
        target_path,
        metadata,
        contents,
        line_ending,
        block,
        integration: ProfileIntegration::Missing,
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

fn parse_managed_block(
    path: &Path,
    contents: &str,
) -> Result<Option<std::ops::Range<usize>>, ShellError> {
    #[derive(Clone, Copy)]
    struct Line<'a> {
        text: &'a str,
        start: usize,
        full_end: usize,
    }

    let mut lines = Vec::new();
    let mut offset = 0;
    while offset < contents.len() {
        let remaining = &contents[offset..];
        let (raw_end, full_end) = match remaining.find('\n') {
            Some(index) => (offset + index, offset + index + 1),
            None => (contents.len(), contents.len()),
        };
        let text_end = if raw_end > offset && contents.as_bytes()[raw_end - 1] == b'\r' {
            raw_end - 1
        } else {
            raw_end
        };
        lines.push(Line {
            text: &contents[offset..text_end],
            start: offset,
            full_end,
        });
        offset = full_end;
    }

    let starts = lines
        .iter()
        .filter(|line| line.text == MANAGED_BLOCK_START)
        .collect::<Vec<_>>();
    let ends = lines
        .iter()
        .filter(|line| line.text == MANAGED_BLOCK_END)
        .collect::<Vec<_>>();
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(None),
        ([start], [end]) if start.start < end.start => Ok(Some(start.start..end.full_end)),
        ([start], [end]) => Err(ShellError::CorruptManagedBlock {
            path: path.to_path_buf(),
            reason: format!(
                "closing marker at byte {} precedes opening marker at byte {}",
                end.start, start.start
            ),
        }),
        _ => Err(ShellError::CorruptManagedBlock {
            path: path.to_path_buf(),
            reason: format!(
                "found {} opening marker(s) and {} closing marker(s)",
                starts.len(),
                ends.len()
            ),
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
    use crate::shell::DetectionSource;
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
            install_profile(&profile, ShellKind::Bash, false).unwrap(),
            InstallAction::Add
        );
        let first = fs::read(&profile).unwrap();
        assert_eq!(
            install_profile(&profile, ShellKind::Bash, false).unwrap(),
            InstallAction::NoOp
        );
        assert_eq!(fs::read(&profile).unwrap(), first);
        assert_eq!(
            profile_status(&profile, ShellKind::Bash).unwrap(),
            ProfileIntegration::Current
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
        uninstall_profile(&profile, false).unwrap();
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
            install_profile(&profile, ShellKind::Fish, false).unwrap(),
            InstallAction::Replace
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
            install_profile(&missing, ShellKind::Bash, true).unwrap(),
            InstallAction::Add
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
            install_profile(&profile, ShellKind::Bash, true).unwrap(),
            InstallAction::Replace
        );
        assert_eq!(
            uninstall_profile(&profile, true).unwrap(),
            UninstallAction::Remove
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
            assert!(uninstall_profile(&profile, false).is_err());
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
            uninstall_profile(&profile, false).unwrap(),
            UninstallAction::Remove
        );
        assert_eq!(fs::read_to_string(&profile).unwrap(), "user line\n");
        assert_eq!(
            uninstall_profile(&profile, false).unwrap(),
            UninstallAction::NoOp
        );

        let empty = temp.path().join("empty");
        install_profile(&empty, ShellKind::Bash, false).unwrap();
        uninstall_profile(&empty, false).unwrap();
        assert!(empty.is_file());
        assert!(fs::read(&empty).unwrap().is_empty());
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
        uninstall_profile(&profile, false).unwrap();
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
