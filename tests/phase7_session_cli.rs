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
fn shell_init_and_direct_public_session_commands_are_side_effect_free() {
    let temp = TempDir::new().unwrap();
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let root = temp.path().join(shell);
        let output = run_dstack(&root, temp.path(), &["shell", "init", shell]);
        assert!(output.status.success(), "{}", text(output.stderr));
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        assert!(!root.exists(), "shell init bootstrapped {shell} state");
    }

    let root = temp.path().join("direct");
    for args in [&["use", "missing"][..], &["unset"][..]] {
        let output = run_dstack(&root, temp.path(), args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = text(output.stderr);
        assert!(stderr.contains("requires shell integration"));
        assert!(stderr.contains("dstack shell init"));
        assert!(!root.exists());
    }

    for args in [&["use", "--help"][..], &["unset", "--help"][..]] {
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
eval "$(dstack shell init bash)"
eval "$(dstack shell init bash)"
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

    let script = r#"eval "$(dstack shell init bash)"; dstack use "$1" || exit 20; printf '%s' "$DSTACK_SESSION""#;
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
            r#"eval "$(dstack shell init zsh)"; eval "$(dstack shell init zsh)"; dstack use profile || exit 20; [[ -n "$DSTACK_SESSION" ]] || exit 21; dstack unset || exit 22; [[ -z "${DSTACK_SESSION+x}" ]]"#,
        ),
        (
            "fish",
            r#"dstack shell init fish | source; dstack shell init fish | source; dstack use profile; or exit 20; set -q DSTACK_SESSION; or exit 21; dstack unset; or exit 22; not set -q DSTACK_SESSION"#,
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
