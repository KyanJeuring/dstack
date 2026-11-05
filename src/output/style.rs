use std::{env, ffi::OsStr};

use anstream::ColorChoice;
use anstyle::{AnsiColor, Color, Style};

const BLUE_BOLD: Style = Style::new()
    .bold()
    .fg_color(Some(Color::Ansi(AnsiColor::Blue)));
const GREEN_BOLD: Style = Style::new()
    .bold()
    .fg_color(Some(Color::Ansi(AnsiColor::Green)));
const YELLOW_BOLD: Style = Style::new()
    .bold()
    .fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
const RED_BOLD: Style = Style::new()
    .bold()
    .fg_color(Some(Color::Ansi(AnsiColor::Red)));

/// Whether optional terminal styling should be rendered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorPolicy {
    enabled: bool,
}

impl ColorPolicy {
    pub const fn enabled() -> Self {
        Self { enabled: true }
    }

    pub const fn disabled() -> Self {
        Self { enabled: false }
    }

    /// Determines color without reading global state, allowing deterministic tests.
    pub fn detect(is_terminal: bool, no_color: bool, term: Option<&OsStr>) -> Self {
        let dumb_terminal = term
            .and_then(OsStr::to_str)
            .is_some_and(|value| value.eq_ignore_ascii_case("dumb"));
        Self {
            enabled: is_terminal && !no_color && !dumb_terminal,
        }
    }

    pub fn from_environment(is_terminal: bool) -> Self {
        Self::detect(
            is_terminal,
            env::var_os("NO_COLOR").is_some(),
            env::var_os("TERM").as_deref(),
        )
    }

    pub const fn is_enabled(self) -> bool {
        self.enabled
    }

    pub const fn stream_choice(self) -> ColorChoice {
        if self.enabled {
            ColorChoice::Always
        } else {
            ColorChoice::Never
        }
    }
}

/// Semantic styles available to current and future command output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextStyle {
    Header,
    Label,
    Info,
    Success,
    Warning,
    Error,
}

impl TextStyle {
    pub(crate) const fn ansi(self) -> Style {
        match self {
            Self::Header | Self::Label | Self::Info => BLUE_BOLD,
            Self::Success => GREEN_BOLD,
            Self::Warning => YELLOW_BOLD,
            Self::Error => RED_BOLD,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::ColorPolicy;

    #[test]
    fn color_requires_a_terminal() {
        assert!(ColorPolicy::detect(true, false, Some(OsStr::new("xterm"))).is_enabled());
        assert!(!ColorPolicy::detect(false, false, Some(OsStr::new("xterm"))).is_enabled());
    }

    #[test]
    fn no_color_presence_disables_color() {
        assert!(!ColorPolicy::detect(true, true, Some(OsStr::new("xterm"))).is_enabled());
    }

    #[test]
    fn dumb_terminal_disables_color_case_insensitively() {
        assert!(!ColorPolicy::detect(true, false, Some(OsStr::new("dumb"))).is_enabled());
        assert!(!ColorPolicy::detect(true, false, Some(OsStr::new("DUMB"))).is_enabled());
    }

    #[test]
    fn missing_term_does_not_disable_an_interactive_terminal() {
        assert!(ColorPolicy::detect(true, false, None).is_enabled());
    }
}
