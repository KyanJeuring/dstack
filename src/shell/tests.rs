use std::process::{Command, Stdio};

use super::{ShellKind, ShellMutation, init_script, render_mutation};

const ADVERSARIAL: &str =
    "space ' \" $ ` ; & ( ) [ ] { } λ $(touch nope) \"; Write-Host nope \\\\windows";

#[test]
fn each_shell_uses_its_native_literal_quoting_rules() {
    assert_eq!(
        render_mutation(ShellKind::Bash, ShellMutation::SetSession("a'b")),
        "export DSTACK_SESSION='a'\\''b'"
    );
    assert_eq!(
        render_mutation(ShellKind::Zsh, ShellMutation::SetSession("a'b")),
        "export DSTACK_SESSION='a'\\''b'"
    );
    assert_eq!(
        render_mutation(ShellKind::Fish, ShellMutation::SetSession("a\\b'c")),
        "set -gx DSTACK_SESSION 'a\\\\b\\'c'"
    );
    assert_eq!(
        render_mutation(ShellKind::Powershell, ShellMutation::SetSession("a'b")),
        "$env:DSTACK_SESSION = 'a''b'"
    );
}

#[test]
fn unset_mutations_are_shell_specific_and_do_not_inspect_old_state() {
    assert_eq!(
        render_mutation(ShellKind::Bash, ShellMutation::UnsetSession),
        "unset DSTACK_SESSION"
    );
    assert_eq!(
        render_mutation(ShellKind::Zsh, ShellMutation::UnsetSession),
        "unset DSTACK_SESSION"
    );
    assert_eq!(
        render_mutation(ShellKind::Fish, ShellMutation::UnsetSession),
        "set -e DSTACK_SESSION"
    );
    assert_eq!(
        render_mutation(ShellKind::Powershell, ShellMutation::UnsetSession),
        "Remove-Item Env:DSTACK_SESSION -ErrorAction SilentlyContinue"
    );
}

#[cfg(unix)]
#[test]
fn bash_mutation_round_trips_adversarial_data_without_execution() {
    let script = format!(
        "{}\nprintf '%s' \"$DSTACK_SESSION\"",
        render_mutation(ShellKind::Bash, ShellMutation::SetSession(ADVERSARIAL))
    );
    let output = Command::new("bash")
        .args(["-c", &script])
        .output()
        .expect("Bash is required by the Unix development environment");
    assert!(output.status.success());
    assert_eq!(output.stdout, ADVERSARIAL.as_bytes());
}

#[cfg(unix)]
#[test]
fn generated_bash_adapter_is_valid_syntax() {
    let mut child = Command::new("bash")
        .args(["-n"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("Bash is required by the Unix development environment");
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(init_script(ShellKind::Bash).as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
}

#[cfg(unix)]
#[test]
fn generated_zsh_init_is_valid_syntax_when_zsh_is_available() {
    if Command::new("zsh").arg("--version").output().is_err() {
        return;
    }
    let mut child = Command::new("zsh")
        .args(["-n"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(init_script(ShellKind::Zsh).as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
}

#[test]
fn adapters_are_native_thin_idempotent_function_definitions() {
    let bash = init_script(ShellKind::Bash);
    assert!(bash.starts_with("dstack()"));
    assert!(bash.contains("DSTACK_SHELL_ADAPTER=bash command dstack shell __activate bash"));
    assert!(bash.contains("DSTACK_SHELL_ADAPTER=bash command dstack \"$@\""));
    assert!(bash.contains("--help"));

    let zsh = init_script(ShellKind::Zsh);
    assert!(zsh.starts_with("dstack()"));
    assert!(zsh.contains("DSTACK_SHELL_ADAPTER=zsh command dstack shell __activate zsh"));

    let fish = init_script(ShellKind::Fish);
    assert!(fish.starts_with("function dstack"));
    assert!(fish.contains("command dstack shell __activate fish"));
    assert!(fish.contains("set -lx DSTACK_SHELL_ADAPTER fish"));
    assert!(!fish.contains("export DSTACK_SESSION"));

    let powershell = init_script(ShellKind::Powershell);
    assert!(powershell.contains("Get-Command dstack -CommandType Application"));
    assert!(powershell.contains("function global:dstack"));
    assert!(powershell.contains("& $global:__DStackExecutable"));
    assert!(powershell.contains("$env:DSTACK_SHELL_ADAPTER = 'powershell'"));
    assert!(powershell.contains("Remove-Item Env:DSTACK_SHELL_ADAPTER"));
    assert!(!powershell.contains("bash"));
    assert!(!powershell.contains("cmd /C"));
}

#[test]
fn full_init_payloads_are_deterministic_and_define_adapters_before_completion() {
    for shell in [
        ShellKind::Bash,
        ShellKind::Zsh,
        ShellKind::Fish,
        ShellKind::Powershell,
    ] {
        assert_eq!(init_script(shell), init_script(shell));
    }

    let bash = init_script(ShellKind::Bash);
    assert!(bash.find("dstack() {").unwrap() < bash.find("_dstack() {").unwrap());

    let zsh = init_script(ShellKind::Zsh);
    assert!(zsh.find("dstack() {").unwrap() < zsh.find("autoload -Uz compinit").unwrap());

    let fish = init_script(ShellKind::Fish);
    assert!(fish.find("function dstack").unwrap() < fish.find("complete -c dstack").unwrap());

    let powershell = init_script(ShellKind::Powershell);
    assert!(
        powershell.find("function global:dstack").unwrap()
            < powershell.find("Register-ArgumentCompleter").unwrap()
    );
}
