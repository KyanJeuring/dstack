use std::fmt;

use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

impl fmt::Display for ShellKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Powershell => "powershell",
        })
    }
}
