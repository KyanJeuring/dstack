use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

use dstack_compose::session::SessionState;

fn run_dstack(root: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dstack"))
        .args(args)
        .current_dir(cwd)
        .env("DSTACK_CONFIG_DIR", root)
        .env_remove("DSTACK_SESSION")
        .env_remove("DSTACK_SHELL_ADAPTER")
        .output()
        .expect("the dstack test binary should run")
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).expect("DStack text output should be UTF-8")
}

fn project(parent: &Path, name: &str, files: &[&str]) -> PathBuf {
    let project = parent.join(name);
    fs::create_dir_all(&project).unwrap();
    for file in files {
        fs::write(project.join(file), "services: {}\n").unwrap();
    }
    fs::canonicalize(project).unwrap()
}

#[test]
fn hidden_shell_init_management_and_direct_public_commands_are_bootstrap_free() {
    let temp = TempDir::new().unwrap();
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let root = temp.path().join(shell);
        let output = run_dstack(&root, temp.path(), &["shell", "__init", shell]);
        assert!(output.status.success(), "{}", text(output.stderr));
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        assert!(!root.exists(), "shell init bootstrapped {shell} state");
    }

    let old_init_root = temp.path().join("old-init");
    let old_init = run_dstack(&old_init_root, temp.path(), &["shell", "init", "bash"]);
    assert_eq!(old_init.status.code(), Some(2));
    assert!(!old_init_root.exists());
    assert!(!temp.path().join("old-init").exists());

    let profile = temp.path().join("profiles/custom profile λ");
    let management_root = temp.path().join("management");
    for args in [
        vec![
            "shell",
            "install",
            "--shell",
            "bash",
            "--profile",
            profile.to_str().unwrap(),
            "--dry-run",
        ],
        vec![
            "shell",
            "status",
            "--shell",
            "bash",
            "--profile",
            profile.to_str().unwrap(),
        ],
    ] {
        let output = run_dstack(&management_root, temp.path(), &args);
        assert!(output.status.success(), "{}", text(output.stderr));
        assert!(!management_root.exists());
    }
    assert!(!profile.exists());
    let malformed_session_status = Command::new(env!("CARGO_BIN_EXE_dstack"))
        .args([
            "shell",
            "status",
            "--shell",
            "bash",
            "--profile",
            profile.to_str().unwrap(),
        ])
        .current_dir(temp.path())
        .env("DSTACK_CONFIG_DIR", &management_root)
        .env("DSTACK_SESSION", "malformed session must stay lazy")
        .env_remove("DSTACK_SHELL_ADAPTER")
        .output()
        .unwrap();
    assert!(
        malformed_session_status.status.success(),
        "{}",
        text(malformed_session_status.stderr)
    );
    assert!(!management_root.exists());
    let install = run_dstack(
        &management_root,
        temp.path(),
        &[
            "shell",
            "install",
            "--shell",
            "bash",
            "--profile",
            profile.to_str().unwrap(),
        ],
    );
    assert!(install.status.success(), "{}", text(install.stderr));
    assert!(profile.is_file());
    assert!(!management_root.exists());
    let uninstall = run_dstack(
        &management_root,
        temp.path(),
        &[
            "shell",
            "uninstall",
            "--shell",
            "bash",
            "--profile",
            profile.to_str().unwrap(),
        ],
    );
    assert!(uninstall.status.success(), "{}", text(uninstall.stderr));
    assert!(profile.is_file());
    assert!(!management_root.exists());

    let root = temp.path().join("direct");
    for args in [&["use", "missing"][..], &["unset"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_dstack"))
            .args(args)
            .current_dir(temp.path())
            .env("DSTACK_CONFIG_DIR", &root)
            .env("HOME", temp.path())
            .env("USERPROFILE", temp.path())
            .env("SHELL", "/bin/bash")
            .env_remove("DSTACK_SESSION")
            .env_remove("DSTACK_SHELL_ADAPTER")
            .env_remove("MSYSTEM")
            .env_remove("POWERSHELL_DISTRIBUTION_CHANNEL")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = text(output.stderr);
        assert!(stderr.contains("requires DStack shell integration"));
        assert!(stderr.contains("dstack shell install"));
        assert!(stderr.contains("Detected shell: Bash"));
        assert!(stderr.contains(".bashrc"));
        assert!(stderr.contains("dstack shell __init bash"));
        assert!(stderr.contains("source"));
        assert!(!root.exists());
    }

    for args in [
        &["use", "--help"][..],
        &["unset", "--help"][..],
        &["shell", "--help"][..],
        &["shell", "install", "--help"][..],
        &["shell", "status", "--help"][..],
        &["shell", "uninstall", "--help"][..],
    ] {
        let output = run_dstack(&root, temp.path(), args);
        assert!(output.status.success());
        assert!(text(output.stdout).contains("Usage:"));
        assert!(!root.exists());
    }
}

#[test]
fn activation_and_clear_emit_only_code_and_never_persist_a_session() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let project = project(temp.path(), "project with spaces", &["compose.yaml"]);
    let add = run_dstack(
        &root,
        temp.path(),
        &["registry", "add", "saved", project.to_str().unwrap()],
    );
    assert!(add.status.success(), "{}", text(add.stderr));
    let config_before = fs::read(root.join("dstack.conf")).unwrap();
    let registry_before = fs::read(root.join("registry.toml")).unwrap();

    let activate = run_dstack(
        &root,
        temp.path(),
        &["shell", "__activate", "bash", "saved"],
    );
    assert!(activate.status.success(), "{}", text(activate.stderr));
    let stdout = text(activate.stdout);
    assert!(stdout.starts_with("export DSTACK_SESSION='{"));
    assert_eq!(stdout.lines().count(), 1);
    assert!(text(activate.stderr).contains("[OK] Using stack 'saved'"));
    assert_eq!(fs::read(root.join("dstack.conf")).unwrap(), config_before);
    assert_eq!(
        fs::read(root.join("registry.toml")).unwrap(),
        registry_before
    );
    let names = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert!(
        !names
            .iter()
            .any(|name| name.to_string_lossy().contains("session"))
    );

    let clear_root = temp.path().join("clear-state");
    let clear = run_dstack(
        &clear_root,
        temp.path(),
        &["shell", "__clear", "powershell"],
    );
    assert!(clear.status.success());
    assert_eq!(
        text(clear.stdout),
        "Remove-Item Env:DSTACK_SESSION -ErrorAction SilentlyContinue\n"
    );
    assert!(text(clear.stderr).contains("terminal context cleared"));
    assert!(!clear_root.exists());
}

#[test]
fn legacy_v1_environment_variables_do_not_activate_v2_state() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let cwd = project(temp.path(), "cwd", &["compose.yaml"]);
    let legacy = project(temp.path(), "legacy", &["legacy.yaml"]);
    let output = Command::new(env!("CARGO_BIN_EXE_dstack"))
        .args(["context", "show"])
        .current_dir(&cwd)
        .env("DSTACK_CONFIG_DIR", &root)
        .env_remove("DSTACK_SESSION")
        .env("DSTACK", &legacy)
        .env("DSTACK_COMPOSE_FILE", "legacy.yaml")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(output.stderr));
    let stdout = text(output.stdout);
    assert!(stdout.contains("Source: cwd\n"));
    assert!(stdout.contains(&cwd.join("compose.yaml").display().to_string()));
    assert!(!stdout.contains(&legacy.display().to_string()));
}

#[cfg(unix)]
fn shell_path() -> std::ffi::OsString {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_dstack"));
    let mut paths = vec![binary.parent().unwrap().to_path_buf()];
    if let Some(current) = env::var_os("PATH") {
        paths.extend(env::split_paths(&current));
    }
    env::join_paths(paths).unwrap()
}

#[cfg(unix)]
fn register(root: &Path, project: &Path) {
    let output = run_dstack(
        root,
        project,
        &["registry", "add", "profile", project.to_str().unwrap()],
    );
    assert!(output.status.success(), "{}", text(output.stderr));
}

#[cfg(unix)]
#[test]
fn bash_adapter_preserves_help_status_activation_failure_and_unset_semantics() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let active = project(temp.path(), "active project", &["compose.yaml"]);
    let cwd = project(temp.path(), "cwd project", &["compose.yaml"]);
    register(&root, &active);
    let script = r#"
eval "$(dstack shell __init bash)"
eval "$(dstack shell __init bash)"
dstack use profile || exit 20
first="$DSTACK_SESSION"
dstack use --help >/dev/null || exit 21
test "$DSTACK_SESSION" = "$first" || exit 22
if dstack use missing; then exit 23; fi
test "$DSTACK_SESSION" = "$first" || exit 24
cd "$1" || exit 25
dstack context show || exit 26
dstack unset || exit 27
test -z "${DSTACK_SESSION+x}" || exit 28
dstack context show || exit 29
dstack --definitely-invalid >/dev/null 2>/dev/null
test "$?" -eq 2 || exit 30
"#;
    let output = Command::new("bash")
        .args(["-c", script, "--", cwd.to_str().unwrap()])
        .env("PATH", shell_path())
        .env("DSTACK_CONFIG_DIR", &root)
        .env_remove("DSTACK_SESSION")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(output.stderr.clone()));
    let stdout = text(output.stdout);
    assert!(stdout.contains("Source: session\n"));
    assert!(stdout.contains("Source: cwd\n"));
    let stderr = text(output.stderr);
    assert!(stderr.contains("[OK] Using stack 'profile'"));
    assert!(stderr.contains("stack 'missing' was not found"));
    assert!(stderr.contains("[OK] DStack terminal context cleared"));
    let profile = run_dstack(&root, &cwd, &["registry", "show", "profile"]);
    assert!(
        profile.status.success(),
        "unset removed the registry profile: {}",
        text(profile.stderr)
    );
}

#[cfg(unix)]
#[test]
fn bash_adapter_keeps_shell_looking_session_data_literal() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let project = project(
        temp.path(),
        "project's $value; $(touch path-injected) & λ",
        &["compose file.yaml"],
    );
    let stack_name = "stack'; $(touch name-injected); `touch backtick-injected`; & λ";
    let add = run_dstack(
        &root,
        temp.path(),
        &[
            "registry",
            "add",
            stack_name,
            project.to_str().unwrap(),
            "--file",
            "compose file.yaml",
        ],
    );
    assert!(add.status.success(), "{}", text(add.stderr));

    let script = r#"eval "$(dstack shell __init bash)"; dstack use "$1" || exit 20; printf '%s' "$DSTACK_SESSION""#;
    let output = Command::new("bash")
        .args(["-c", script, "--", stack_name])
        .current_dir(temp.path())
        .env("PATH", shell_path())
        .env("DSTACK_CONFIG_DIR", &root)
        .env_remove("DSTACK_SESSION")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(output.stderr));
    let state = SessionState::parse(&text(output.stdout)).unwrap();
    assert_eq!(state.name, stack_name);
    assert_eq!(state.project, project);
    for marker in ["path-injected", "name-injected", "backtick-injected"] {
        assert!(!temp.path().join(marker).exists(), "executed {marker}");
    }
}

#[cfg(unix)]
#[test]
fn available_zsh_and_fish_adapters_smoke_activation_and_unset() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let active = project(temp.path(), "active", &["compose.yaml"]);
    register(&root, &active);

    let cases = [
        (
            "zsh",
            r#"eval "$(dstack shell __init zsh)"; eval "$(dstack shell __init zsh)"; dstack use profile || exit 20; [[ -n "$DSTACK_SESSION" ]] || exit 21; dstack unset || exit 22; [[ -z "${DSTACK_SESSION+x}" ]]"#,
        ),
        (
            "fish",
            r#"dstack shell __init fish | source; dstack shell __init fish | source; dstack use profile; or exit 20; set -q DSTACK_SESSION; or exit 21; dstack unset; or exit 22; not set -q DSTACK_SESSION"#,
        ),
    ];
    for (shell, script) in cases {
        if Command::new(shell).arg("--version").output().is_err() {
            continue;
        }
        let output = Command::new(shell)
            .args(["-c", script])
            .env("PATH", shell_path())
            .env("DSTACK_CONFIG_DIR", &root)
            .env_remove("DSTACK_SESSION")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{shell} adapter failed: {}",
            text(output.stderr)
        );
    }
}

#[cfg(unix)]
#[test]
fn available_zsh_and_fish_profile_management_smoke() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let active = project(temp.path(), "active", &["compose.yaml"]);
    register(&root, &active);

    for shell in ["zsh", "fish"] {
        if Command::new(shell).arg("--version").output().is_err() {
            continue;
        }
        let profile = temp.path().join(format!("{shell} profile λ"));
        let install = run_dstack(
            &root,
            temp.path(),
            &[
                "shell",
                "install",
                "--shell",
                shell,
                "--profile",
                profile.to_str().unwrap(),
            ],
        );
        assert!(install.status.success(), "{}", text(install.stderr));

        let script = if shell == "zsh" {
            r#"source "$1"; source "$1"; dstack use profile || exit 20; dstack shell status --shell zsh --profile "$1" || exit 21; dstack unset || exit 22; dstack shell uninstall --shell zsh --profile "$1" || exit 23"#
        } else {
            r#"source $argv[1]; source $argv[1]; dstack use profile; or exit 20; dstack shell status --shell fish --profile $argv[1]; or exit 21; dstack unset; or exit 22; dstack shell uninstall --shell fish --profile $argv[1]; or exit 23"#
        };
        let mut command = Command::new(shell);
        command.args(["-c", script]);
        if shell == "zsh" {
            command.args(["--", profile.to_str().unwrap()]);
        } else {
            command.arg(profile.to_str().unwrap());
        }
        let output = command
            .env("PATH", shell_path())
            .env("DSTACK_CONFIG_DIR", &root)
            .env_remove("DSTACK_SESSION")
            .env_remove("DSTACK_SHELL_ADAPTER")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{shell} profile workflow failed: {}",
            text(output.stderr)
        );
        let stdout = text(output.stdout);
        assert!(stdout.contains("Profile integration: installed"));
        assert!(stdout.contains("Current terminal adapter: active"));
        assert!(profile.is_file());
        assert!(fs::read(&profile).unwrap().is_empty());
    }
}

#[cfg(unix)]
#[test]
fn bash_profile_install_status_use_unset_and_uninstall_workflow() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let active = project(temp.path(), "active", &["compose.yaml"]);
    register(&root, &active);
    let profile = temp.path().join("dotfiles/profile's λ.bash");

    let dry_run = Command::new("bash")
        .args([
            "-c",
            r#"dstack shell install --profile "$1" --dry-run"#,
            "--",
            profile.to_str().unwrap(),
        ])
        .env("PATH", shell_path())
        .env("DSTACK_CONFIG_DIR", &root)
        .env_remove("DSTACK_SHELL_ADAPTER")
        .output()
        .unwrap();
    assert!(dry_run.status.success(), "{}", text(dry_run.stderr));
    assert!(text(dry_run.stdout).contains("Detected shell: Bash"));
    assert!(!profile.exists());

    let install = run_dstack(
        &root,
        temp.path(),
        &[
            "shell",
            "install",
            "--shell",
            "bash",
            "--profile",
            profile.to_str().unwrap(),
        ],
    );
    assert!(install.status.success(), "{}", text(install.stderr));
    let installed = fs::read_to_string(&profile).unwrap();
    assert_eq!(
        installed
            .matches("# >>> DStack shell integration >>>")
            .count(),
        1
    );

    let script = r#"
source "$1" || exit 20
dstack use profile || exit 21
test -n "$DSTACK_SESSION" || exit 22
session_before_status="$DSTACK_SESSION"
dstack shell status --profile "$1" || exit 23
test "$DSTACK_SESSION" = "$session_before_status" || exit 30
test -z "${DSTACK_SHELL_ADAPTER+x}" || exit 28
dstack context show || exit 24
dstack unset || exit 25
test -z "${DSTACK_SESSION+x}" || exit 26
dstack shell uninstall --profile "$1" || exit 27
dstack shell status --profile "$1" || exit 29
"#;
    let output = Command::new("bash")
        .args(["-c", script, "--", profile.to_str().unwrap()])
        .env("PATH", shell_path())
        .env("DSTACK_CONFIG_DIR", &root)
        .env_remove("DSTACK_SESSION")
        .env_remove("DSTACK_SHELL_ADAPTER")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(output.stderr));
    let stdout = text(output.stdout);
    assert!(stdout.contains("Profile integration: installed"));
    assert!(stdout.contains("Profile integration: not installed"));
    assert!(stdout.contains("Current terminal adapter: active"));
    assert!(stdout.contains("Source: session"));
    assert!(stdout.contains("currently running shell may still have the adapter loaded"));
    assert!(profile.is_file());
    assert!(fs::read(&profile).unwrap().is_empty());
}

#[test]
fn unknown_detection_fails_without_profile_or_dstack_state_changes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let output = Command::new(env!("CARGO_BIN_EXE_dstack"))
        .args(["shell", "install"])
        .current_dir(temp.path())
        .env("DSTACK_CONFIG_DIR", &root)
        .env_remove("DSTACK_SHELL_ADAPTER")
        .env_remove("SHELL")
        .env_remove("MSYSTEM")
        .env_remove("POWERSHELL_DISTRIBUTION_CHANNEL")
        .env_remove("FISH_VERSION")
        .env_remove("ZSH_VERSION")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(output.stderr);
    assert!(stderr.contains("could not reliably detect"));
    assert!(stderr.contains("--shell bash"));
    assert!(!root.exists());

    for arguments in [["use", "missing"].as_slice(), ["unset"].as_slice()] {
        let direct = Command::new(env!("CARGO_BIN_EXE_dstack"))
            .args(arguments)
            .current_dir(temp.path())
            .env("DSTACK_CONFIG_DIR", &root)
            .env_remove("DSTACK_SHELL_ADAPTER")
            .env_remove("SHELL")
            .env_remove("MSYSTEM")
            .env_remove("POWERSHELL_DISTRIBUTION_CHANNEL")
            .env_remove("FISH_VERSION")
            .env_remove("ZSH_VERSION")
            .output()
            .unwrap();
        assert_eq!(direct.status.code(), Some(1));
        let diagnostic = text(direct.stderr);
        assert!(diagnostic.contains("dstack shell install --shell bash"));
        assert!(!diagnostic.contains("Profile:"));
        assert!(!root.exists());
    }
}
