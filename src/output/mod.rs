//! Human-facing output primitives shared by application commands.

mod style;
mod table;

use std::io::{self, Write};

pub use style::{ColorPolicy, TextStyle};
pub use table::Table;

/// A writer paired with a deterministic styling policy.
pub struct Output<'a> {
    writer: &'a mut dyn Write,
    color: ColorPolicy,
}

impl<'a> Output<'a> {
    pub fn new(writer: &'a mut dyn Write, color: ColorPolicy) -> Self {
        Self { writer, color }
    }

    pub fn plain(writer: &'a mut dyn Write) -> Self {
        Self::new(writer, ColorPolicy::disabled())
    }

    pub fn color_policy(&self) -> ColorPolicy {
        self.color
    }

    pub fn write_styled(&mut self, style: TextStyle, text: &str) -> io::Result<()> {
        if self.color.is_enabled() {
            let ansi = style.ansi();
            write!(self.writer, "{ansi}{text}{ansi:#}")
        } else {
            self.writer.write_all(text.as_bytes())
        }
    }

    pub fn write_styled_line(&mut self, style: TextStyle, text: &str) -> io::Result<()> {
        self.write_styled(style, text)?;
        self.writer.write_all(b"\n")
    }

    pub fn write_label(&mut self, label: &str) -> io::Result<()> {
        self.write_styled(TextStyle::Label, label)
    }

    pub fn write_label_line(&mut self, label: &str) -> io::Result<()> {
        self.write_styled_line(TextStyle::Label, label)
    }

    pub fn write_label_value(&mut self, label: &str, value: &str) -> io::Result<()> {
        self.write_label(label)?;
        writeln!(self.writer, " {value}")
    }
}

impl Write for Output<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.writer.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorPolicy, Output, TextStyle};

    #[test]
    fn enabled_style_emits_ansi_and_disabled_style_does_not() {
        let mut colored = Vec::new();
        Output::new(&mut colored, ColorPolicy::enabled())
            .write_styled_line(TextStyle::Header, "NAME  PATH")
            .unwrap();
        let colored = String::from_utf8(colored).unwrap();
        assert!(colored.starts_with("\u{1b}[1m\u{1b}[34m"));
        assert!(colored.ends_with("\u{1b}[0m\n"));

        let mut plain = Vec::new();
        Output::plain(&mut plain)
            .write_styled_line(TextStyle::Header, "NAME  PATH")
            .unwrap();
        assert_eq!(plain, b"NAME  PATH\n");
    }

    #[test]
    fn label_text_is_identical_after_ansi_is_removed() {
        let mut colored = Vec::new();
        Output::new(&mut colored, ColorPolicy::enabled())
            .write_label_value("Stack:", "example")
            .unwrap();
        let colored = String::from_utf8(colored).unwrap();
        let plain = colored
            .replace("\u{1b}[1m\u{1b}[34m", "")
            .replace("\u{1b}[0m", "");
        assert_eq!(plain, "Stack: example\n");
    }
}
