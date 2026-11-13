use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

use dstack_compose::{
    session::SessionState,
    shell::{ShellKind, legacy_manual_loader, managed_block, manual_loader},
};

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
        for protocol in ["__completions", "__init"] {
            let root = temp.path().join(format!("{shell}-{protocol}"));
            let output = Command::new(env!("CARGO_BIN_EXE_dstack"))
                .args(["shell", protocol, shell])
                .current_dir(temp.path())
                .env("DSTACK_CONFIG_DIR", &root)
                .env("DSTACK_SESSION", "malformed session must stay lazy")
                .output()
                .unwrap();
            assert!(output.status.success(), "{}", text(output.stderr));
            assert!(!output.stdout.is_empty());
            assert!(output.stderr.is_empty());
            assert!(
                !root.exists(),
                "shell {protocol} bootstrapped {shell} state"
            );
        }
    }

    let old_init_root = temp.path().join("old-init");
    let old_init = run_dstack(&old_init_root, temp.path(), &["shell", "init", "bash"]);
    assert_eq!(old_init.status.code(), Some(2));
    assert!(!old_init_root.exists());
    assert!(!temp.path().join("old-init").exists());

    for arguments in [
        &["shell", "__completions"][..],
        &["shell", "__completions", "elvish"][..],
        &["shell", "__completions", "nushell"][..],
    ] {
        let root = temp.path().join("invalid-completion");
        let output = run_dstack(&root, temp.path(), arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(!root.exists());
    }

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
        assert!(stderr.contains("requires the DStack shell adapter in the current terminal"));
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
fn bash_completion_is_registered_for_the_wrapper_and_excludes_hidden_protocol() {
    let script = r#"
eval "$(command dstack shell __init bash)" || exit 20
eval "$(command dstack shell __init bash)" || exit 21
[[ "$(type -t dstack)" == function ]] || exit 22
[[ "$(complete -p dstack)" == *"-F _dstack"* ]] || exit 23

COMP_WORDS=(dstack sys)
COMP_CWORD=1
_dstack dstack sys ""
[[ " ${COMPREPLY[*]} " == *" system "* ]] || exit 24

COMP_WORDS=(dstack shell "")
COMP_CWORD=2
_dstack dstack "" shell
[[ " ${COMPREPLY[*]} " == *" install "* ]] || exit 25
[[ " ${COMPREPLY[*]} " == *" status "* ]] || exit 26
[[ " ${COMPREPLY[*]} " == *" uninstall "* ]] || exit 27
for hidden in __init __activate __clear __completions; do
  [[ " ${COMPREPLY[*]} " != *" $hidden "* ]] || exit 28
done
"#;
    let output = Command::new("bash")
        .args(["-c", script])
        .env("PATH", shell_path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Bash completion smoke failed: {}",
        text(output.stderr)
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
            r#"eval "$(dstack shell __init zsh)"; eval "$(dstack shell __init zsh)"; (( $+functions[dstack] )) || exit 18; [[ "${_comps[dstack]}" = _dstack ]] || exit 19; dstack use profile || exit 20; [[ -n "$DSTACK_SESSION" ]] || exit 21; dstack unset || exit 22; [[ -z "${DSTACK_SESSION+x}" ]]"#,
        ),
        (
            "fish",
            r#"dstack shell __init fish | source; dstack shell __init fish | source; functions -q dstack; or exit 18; set -l matches (complete -C 'dstack sys' | string match -r '^system\t'); test (count $matches) -eq 1; or exit 19; dstack use profile; or exit 20; set -q DSTACK_SESSION; or exit 21; dstack unset; or exit 22; not set -q DSTACK_SESSION"#,
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
fn zsh_completion_initialization_is_safe_when_ready_or_unavailable() {
    if Command::new("zsh").arg("--version").output().is_err() {
        return;
    }

    let initialized = r#"
autoload -Uz compinit
compinit
__dstack_compinit_called=0
compinit() { __dstack_compinit_called=1; return 91; }
eval "$(command dstack shell __init zsh)" || exit 20
(( __dstack_compinit_called == 0 )) || exit 21
(( $+functions[dstack] )) || exit 22
[[ "${_comps[dstack]}" = _dstack ]] || exit 23
"#;
    let output = Command::new("zsh")
        .args(["-f", "-c", initialized])
        .env("PATH", shell_path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "initialized Zsh completion smoke failed: {}",
        text(output.stderr)
    );

    let temp = TempDir::new().unwrap();
    let empty_fpath = temp.path().join("empty-fpath");
    fs::create_dir(&empty_fpath).unwrap();
    let unavailable = r#"
fpath=("$1")
eval "$(command dstack shell __init zsh)" 2>/dev/null
(( $+functions[dstack] )) || exit 30
(( ! $+functions[_dstack] )) || exit 31
"#;
    let output = Command::new("zsh")
        .args(["-f", "-c", unavailable, "--", empty_fpath.to_str().unwrap()])
        .env("PATH", shell_path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "unavailable Zsh completion lost the wrapper: {}",
        text(output.stderr)
    );
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

        let kind = if shell == "zsh" {
            ShellKind::Zsh
        } else {
            ShellKind::Fish
        };
        let manual_profile = temp.path().join(format!("manual {shell} profile λ"));
        let manual_contents = format!("{}\n", manual_loader(kind, "\n"));
        fs::write(&manual_profile, &manual_contents).unwrap();
        let install = run_dstack(
            &root,
            temp.path(),
            &[
                "shell",
                "install",
                "--shell",
                shell,
                "--profile",
                manual_profile.to_str().unwrap(),
            ],
        );
        assert!(install.status.success(), "{}", text(install.stderr));
        assert!(text(install.stdout).contains("already installed manually"));
        assert_eq!(
            fs::read_to_string(&manual_profile).unwrap(),
            manual_contents
        );

        let script = if shell == "zsh" {
            r#"source "$1"; dstack shell status --shell zsh --profile "$1" || exit 30; dstack use profile || exit 31; dstack unset || exit 32; dstack shell uninstall --shell zsh --profile "$1" || exit 33"#
        } else {
            r#"source $argv[1]; dstack shell status --shell fish --profile $argv[1]; or exit 30; dstack use profile; or exit 31; dstack unset; or exit 32; dstack shell uninstall --shell fish --profile $argv[1]; or exit 33"#
        };
        let mut command = Command::new(shell);
        command.args(["-c", script]);
        if shell == "zsh" {
            command.args(["--", manual_profile.to_str().unwrap()]);
        } else {
            command.arg(manual_profile.to_str().unwrap());
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
            "{shell} manual profile workflow failed: {}",
            text(output.stderr)
        );
        let stdout = text(output.stdout);
        assert!(stdout.contains("Profile integration: installed (manual)"));
        assert!(stdout.contains("Current terminal adapter: active"));
        assert!(stdout.contains("No DStack-managed block was removed"));
        assert_eq!(
            fs::read_to_string(&manual_profile).unwrap(),
            manual_contents
        );
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
            r#"dstack shell install --profile "$1" --dry-run; status=$?; exit "$status""#,
            "--",
            profile.to_str().unwrap(),
        ])
        .env("PATH", shell_path())
        .env("DSTACK_CONFIG_DIR", &root)
        .env_remove("DSTACK_SHELL_ADAPTER")
        .env_remove("SHELL")
        .output()
        .unwrap();
    assert!(dry_run.status.success(), "{}", text(dry_run.stderr));
    let dry_run_stdout = text(dry_run.stdout);
    assert!(
        dry_run_stdout.contains("Detected shell: Bash"),
        "{dry_run_stdout}"
    );
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

#[cfg(unix)]
#[test]
fn bash_manual_profile_workflow_remains_user_owned() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let active = project(temp.path(), "active", &["compose.yaml"]);
    register(&root, &active);
    let profile = temp.path().join("manual bash profile λ");
    let original = format!(
        "export BEFORE=1\n{}\nexport AFTER=1\n",
        manual_loader(ShellKind::Bash, "\n")
    );
    fs::write(&profile, &original).unwrap();

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
    assert_eq!(fs::read_to_string(&profile).unwrap(), original);

    let script = r#"
source "$1" || exit 20
dstack shell status --shell bash --profile "$1" || exit 21
dstack use profile || exit 22
test -n "$DSTACK_SESSION" || exit 23
dstack unset || exit 24
test -z "${DSTACK_SESSION+x}" || exit 25
dstack shell uninstall --shell bash --profile "$1" || exit 26
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
    assert!(stdout.contains("Profile integration: installed (manual)"));
    assert!(stdout.contains("Current terminal adapter: active"));
    assert!(stdout.contains("No DStack-managed block was removed"));
    assert_eq!(fs::read_to_string(&profile).unwrap(), original);
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

#[test]
fn canonical_manual_installations_are_detected_without_duplicate_profile_changes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let cases = [
        ("bash", ShellKind::Bash),
        ("zsh", ShellKind::Zsh),
        ("fish", ShellKind::Fish),
        ("powershell", ShellKind::Powershell),
    ];
    for (name, shell) in cases {
        let profile = temp.path().join(format!("manual {name} λ"));
        let loader = manual_loader(shell, "\n");
        let original = format!("export BEFORE=1\n{loader}\nexport AFTER=1\n");
        fs::write(&profile, &original).unwrap();
        let before = fs::read(&profile).unwrap();

        let install = run_dstack(
            &root,
            temp.path(),
            &[
                "shell",
                "install",
                "--shell",
                name,
                "--profile",
                profile.to_str().unwrap(),
            ],
        );
        assert!(install.status.success(), "{}", text(install.stderr));
        let stdout = text(install.stdout);
        assert!(stdout.contains("already installed manually"), "{stdout}");
        assert!(stdout.contains("No managed block was added"), "{stdout}");
        assert_eq!(fs::read(&profile).unwrap(), before);
        assert!(!original.contains("# >>> DStack shell integration >>>"));
        assert_eq!(original.matches(&loader).count(), 1);

        let status = run_dstack(
            &root,
            temp.path(),
            &[
                "shell",
                "status",
                "--shell",
                name,
                "--profile",
                profile.to_str().unwrap(),
            ],
        );
        assert!(status.status.success(), "{}", text(status.stderr));
        assert!(text(status.stdout).contains("Profile integration: installed (manual)"));

        let dry_run = run_dstack(
            &root,
            temp.path(),
            &[
                "shell",
                "install",
                "--shell",
                name,
                "--profile",
                profile.to_str().unwrap(),
                "--dry-run",
            ],
        );
        assert!(dry_run.status.success(), "{}", text(dry_run.stderr));
        assert!(text(dry_run.stdout).contains("already installed manually"));
        assert_eq!(fs::read(&profile).unwrap(), before);

        let uninstall_dry_run = run_dstack(
            &root,
            temp.path(),
            &[
                "shell",
                "uninstall",
                "--shell",
                name,
                "--profile",
                profile.to_str().unwrap(),
                "--dry-run",
            ],
        );
        assert!(
            uninstall_dry_run.status.success(),
            "{}",
            text(uninstall_dry_run.stderr)
        );
        let dry_stdout = text(uninstall_dry_run.stdout);
        assert!(dry_stdout.contains("managed block is absent"));
        assert!(dry_stdout.contains("Manual integration: detected; it would remain"));
        assert_eq!(fs::read(&profile).unwrap(), before);

        let uninstall = run_dstack(
            &root,
            temp.path(),
            &[
                "shell",
                "uninstall",
                "--shell",
                name,
                "--profile",
                profile.to_str().unwrap(),
            ],
        );
        assert!(uninstall.status.success(), "{}", text(uninstall.stderr));
        let stdout = text(uninstall.stdout);
        assert!(stdout.contains("installed manually"), "{stdout}");
        assert!(
            stdout.contains("No DStack-managed block was removed"),
            "{stdout}"
        );
        assert_eq!(fs::read(&profile).unwrap(), before);
    }
    assert!(!root.exists());
}

#[test]
fn legacy_duplicate_and_corrupt_profile_states_are_truthful_and_never_installed_over() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let profile = temp.path().join("profile");

    let missing = run_dstack(
        &root,
        temp.path(),
        &[
            "shell",
            "status",
            "--shell",
            "bash",
            "--profile",
            profile.to_str().unwrap(),
        ],
    );
    assert!(missing.status.success(), "{}", text(missing.stderr));
    assert!(text(missing.stdout).contains("Profile integration: not installed"));

    fs::write(
        &profile,
        "# >>> DStack shell integration >>>\nold loader\n# <<< DStack shell integration <<<\n",
    )
    .unwrap();
    let outdated = run_dstack(
        &root,
        temp.path(),
        &[
            "shell",
            "status",
            "--shell",
            "bash",
            "--profile",
            profile.to_str().unwrap(),
        ],
    );
    assert!(outdated.status.success(), "{}", text(outdated.stderr));
    assert!(text(outdated.stdout).contains("Profile integration: update available (managed)"));

    let legacy = legacy_manual_loader(ShellKind::Bash);
    fs::write(&profile, format!("before\n{legacy}\nafter\n")).unwrap();
    let before = fs::read(&profile).unwrap();

    let status = run_dstack(
        &root,
        temp.path(),
        &[
            "shell",
            "status",
            "--shell",
            "bash",
            "--profile",
            profile.to_str().unwrap(),
        ],
    );
    assert!(status.status.success(), "{}", text(status.stderr));
    let stdout = text(status.stdout);
    assert!(stdout.contains("Profile integration: outdated (manual)"));
    assert!(stdout.contains(legacy));
    assert!(stdout.contains(&manual_loader(ShellKind::Bash, "\n")));

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
    assert_eq!(install.status.code(), Some(1));
    assert!(text(install.stderr).contains("older DStack shell loader"));
    assert_eq!(fs::read(&profile).unwrap(), before);

    let uninstall = run_dstack(
        &root,
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
    assert!(text(uninstall.stdout).contains("outdated manual integration"));
    assert_eq!(fs::read(&profile).unwrap(), before);

    let current = manual_loader(ShellKind::Bash, "\n");
    let duplicate_forms = [
        format!("{current}\n{current}\n"),
        format!("{current}\n{legacy}\n"),
        format!("{}\n{current}\n", managed_block(ShellKind::Bash, "\n")),
    ];
    for contents in duplicate_forms {
        fs::write(&profile, &contents).unwrap();
        let before = fs::read(&profile).unwrap();
        let status = run_dstack(
            &root,
            temp.path(),
            &[
                "shell",
                "status",
                "--shell",
                "bash",
                "--profile",
                profile.to_str().unwrap(),
            ],
        );
        assert_eq!(status.status.code(), Some(1));
        assert!(text(status.stdout).contains("Profile integration: duplicate"));
        assert!(text(status.stderr).contains("Duplicate DStack shell integration"));
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
                "--dry-run",
            ],
        );
        assert_eq!(install.status.code(), Some(1));
        assert!(text(install.stdout).contains("Profile integration: duplicate"));
        assert_eq!(fs::read(&profile).unwrap(), before);
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
        assert_eq!(install.status.code(), Some(1));
        assert!(text(install.stderr).contains("Duplicate DStack shell integration"));
        assert_eq!(fs::read(&profile).unwrap(), before);
        if !contents.contains("# >>> DStack shell integration >>>") {
            let uninstall = run_dstack(
                &root,
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
            let stdout = text(uninstall.stdout);
            assert!(stdout.contains("No DStack-managed block was removed"));
            assert!(stdout.contains("Manual profile content remains user-owned"));
            assert_eq!(fs::read(&profile).unwrap(), before);
        }
    }

    fs::write(&profile, "# >>> DStack shell integration >>>\nvalue\n").unwrap();
    let corrupt = run_dstack(
        &root,
        temp.path(),
        &[
            "shell",
            "status",
            "--shell",
            "bash",
            "--profile",
            profile.to_str().unwrap(),
        ],
    );
    assert_eq!(corrupt.status.code(), Some(1));
    assert!(text(corrupt.stderr).contains("invalid DStack managed markers"));
    assert!(!root.exists());
}

#[test]
fn managed_and_manual_uninstall_removes_only_the_managed_block() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let profile = temp.path().join("mixed profile");
    let manual = manual_loader(ShellKind::Bash, "\n");
    fs::write(
        &profile,
        format!(
            "{}\nexport BEFORE=1\n{manual}\nexport AFTER=1\n",
            managed_block(ShellKind::Bash, "\n")
        ),
    )
    .unwrap();

    let dry_run = run_dstack(
        &root,
        temp.path(),
        &[
            "shell",
            "uninstall",
            "--shell",
            "bash",
            "--profile",
            profile.to_str().unwrap(),
            "--dry-run",
        ],
    );
    assert!(dry_run.status.success(), "{}", text(dry_run.stderr));
    let stdout = text(dry_run.stdout);
    assert!(stdout.contains("Would: remove managed block"));
    assert!(stdout.contains("Manual integration: detected; it would remain"));

    let uninstall = run_dstack(
        &root,
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
    let stdout = text(uninstall.stdout);
    assert!(stdout.contains("Removed DStack-managed"));
    assert!(stdout.contains("manual DStack loader still remains"));
    assert_eq!(
        fs::read_to_string(&profile).unwrap(),
        format!("export BEFORE=1\n{manual}\nexport AFTER=1\n")
    );
    assert!(!root.exists());
}

#[cfg(unix)]
#[test]
fn direct_session_guidance_uses_profile_ownership_and_reload_state() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("state");
    let cases = [
        ("missing", String::new(), "Set it up automatically", false),
        (
            "managed",
            format!("{}\n", managed_block(ShellKind::Bash, "\n")),
            "already installed as a managed block",
            false,
        ),
        (
            "manual",
            format!("{}\n", manual_loader(ShellKind::Bash, "\n")),
            "already installed manually",
            false,
        ),
        (
            "legacy",
            format!("{}\n", legacy_manual_loader(ShellKind::Bash)),
            "outdated manual shell loader",
            true,
        ),
        (
            "duplicate",
            format!(
                "{}\n{}\n",
                managed_block(ShellKind::Bash, "\n"),
                manual_loader(ShellKind::Bash, "\n")
            ),
            "duplicate or conflicting shell integrations",
            false,
        ),
    ];
    for (name, contents, expected, replacement) in cases {
        let home = temp.path().join(name);
        fs::create_dir_all(&home).unwrap();
        if !contents.is_empty() {
            fs::write(home.join(".bashrc"), contents).unwrap();
        }
        for args in [&["use", "profile"][..], &["unset"][..]] {
            let output = Command::new(env!("CARGO_BIN_EXE_dstack"))
                .args(args)
                .current_dir(temp.path())
                .env("DSTACK_CONFIG_DIR", &root)
                .env("HOME", &home)
                .env("USERPROFILE", &home)
                .env("SHELL", "/bin/bash")
                .env_remove("DSTACK_SESSION")
                .env_remove("DSTACK_SHELL_ADAPTER")
                .env_remove("MSYSTEM")
                .env_remove("POWERSHELL_DISTRIBUTION_CHANNEL")
                .env_remove("FISH_VERSION")
                .env_remove("ZSH_VERSION")
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1));
            let stderr = text(output.stderr);
            assert!(stderr.contains(expected), "{name}: {stderr}");
            if name == "managed" || name == "manual" {
                assert!(stderr.contains("current terminal has not loaded"));
                assert!(stderr.contains("source"));
                assert!(!stderr.contains("Set it up automatically"));
            }
            if replacement {
                assert!(stderr.contains("Replace:"));
                assert!(stderr.contains("shell __init bash"));
            }
            if name == "duplicate" {
                assert!(stderr.contains("Do not install another loader"));
                assert!(stderr.contains("dstack shell status"));
            }
        }
    }
    assert!(!root.exists());
}
