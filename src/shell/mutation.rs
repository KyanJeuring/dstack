use super::{ShellKind, quote};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellMutation<'a> {
    SetSession(&'a str),
    UnsetSession,
}

pub fn render_mutation(shell: ShellKind, mutation: ShellMutation<'_>) -> String {
    match (shell, mutation) {
        (ShellKind::Bash, ShellMutation::SetSession(value)) => {
            format!("export DSTACK_SESSION={}", quote::bash(value))
        }
        (ShellKind::Zsh, ShellMutation::SetSession(value)) => {
            format!("export DSTACK_SESSION={}", quote::zsh(value))
        }
        (ShellKind::Fish, ShellMutation::SetSession(value)) => {
            format!("set -gx DSTACK_SESSION {}", quote::fish(value))
        }
        (ShellKind::Powershell, ShellMutation::SetSession(value)) => {
            format!("$env:DSTACK_SESSION = {}", quote::powershell(value))
        }
        (ShellKind::Bash | ShellKind::Zsh, ShellMutation::UnsetSession) => {
            "unset DSTACK_SESSION".to_owned()
        }
        (ShellKind::Fish, ShellMutation::UnsetSession) => "set -e DSTACK_SESSION".to_owned(),
        (ShellKind::Powershell, ShellMutation::UnsetSession) => {
            "Remove-Item Env:DSTACK_SESSION -ErrorAction SilentlyContinue".to_owned()
        }
    }
}
