use std::io::{self, Write};

use unicode_width::UnicodeWidthStr;

use super::{Output, TextStyle};

const COLUMN_PADDING: usize = 2;

/// A small fixed-column human-readable table.
pub struct Table<const COLUMNS: usize> {
    headers: [String; COLUMNS],
    rows: Vec<[String; COLUMNS]>,
}

impl<const COLUMNS: usize> Table<COLUMNS> {
    pub fn new(headers: [&str; COLUMNS]) -> Self {
        Self {
            headers: headers.map(str::to_owned),
            rows: Vec::new(),
        }
    }

    pub fn push(&mut self, row: [String; COLUMNS]) {
        self.rows.push(row);
    }

    pub fn write(&self, output: &mut Output<'_>) -> io::Result<()> {
        let widths = self.widths();
        let header = render_row(&self.headers, &widths);
        output.write_styled_line(TextStyle::Header, &header)?;
        for row in &self.rows {
            writeln!(output, "{}", render_row(row, &widths))?;
        }
        Ok(())
    }

    fn widths(&self) -> [usize; COLUMNS] {
        let mut widths = self.headers.each_ref().map(|value| display_width(value));
        for row in &self.rows {
            for (index, value) in row.iter().enumerate() {
                widths[index] = widths[index].max(display_width(value));
            }
        }
        widths
    }
}

fn render_row<const COLUMNS: usize>(row: &[String; COLUMNS], widths: &[usize; COLUMNS]) -> String {
    let capacity =
        widths.iter().sum::<usize>() + COLUMN_PADDING.saturating_mul(COLUMNS.saturating_sub(1));
    let mut rendered = String::with_capacity(capacity);
    for (index, value) in row.iter().enumerate() {
        rendered.push_str(value);
        if index + 1 < COLUMNS {
            let spaces = widths[index] - display_width(value) + COLUMN_PADDING;
            rendered.extend(std::iter::repeat_n(' ', spaces));
        }
    }
    rendered
}

fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

#[cfg(test)]
mod tests {
    use super::Table;
    use crate::output::{ColorPolicy, Output};

    fn render<const COLUMNS: usize>(table: &Table<COLUMNS>, color: ColorPolicy) -> String {
        let mut bytes = Vec::new();
        table.write(&mut Output::new(&mut bytes, color)).unwrap();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn header_only_table_is_stable_and_contains_no_tabs() {
        let table = Table::new(["NAME", "SOURCE", "PATH"]);
        let rendered = render(&table, ColorPolicy::disabled());
        assert_eq!(rendered, "NAME  SOURCE  PATH\n");
        assert!(!rendered.contains('\t'));
    }

    #[test]
    fn columns_expand_for_varying_values_and_ambiguous_sources() {
        let mut table = Table::new(["NAME", "SOURCE", "PATH"]);
        table.push(["a".to_owned(), "discovered".to_owned(), "/short".to_owned()]);
        table.push([
            "babylonianvisualizer".to_owned(),
            "discovered (ambiguous)".to_owned(),
            "/a/much/longer/path".to_owned(),
        ]);
        assert_eq!(
            render(&table, ColorPolicy::disabled()),
            concat!(
                "NAME                  SOURCE                  PATH\n",
                "a                     discovered              /short\n",
                "babylonianvisualizer  discovered (ambiguous)  /a/much/longer/path\n",
            )
        );
    }

    #[test]
    fn registry_files_column_expands_without_truncation() {
        let mut table = Table::new(["NAME", "PATH", "FILES"]);
        table.push([
            "jellyfin".to_owned(),
            "/srv/jellyfin".to_owned(),
            "-".to_owned(),
        ]);
        table.push([
            "jellyfin-prod".to_owned(),
            "/srv/jellyfin/production".to_owned(),
            "compose.yaml, compose.prod.yaml".to_owned(),
        ]);
        assert_eq!(
            render(&table, ColorPolicy::disabled()),
            concat!(
                "NAME           PATH                      FILES\n",
                "jellyfin       /srv/jellyfin             -\n",
                "jellyfin-prod  /srv/jellyfin/production  compose.yaml, compose.prod.yaml\n",
            )
        );
    }

    #[test]
    fn unicode_display_width_aligns_columns() {
        let mut table = Table::new(["NAME", "PATH"]);
        table.push(["服务".to_owned(), "/项目/服务".to_owned()]);
        table.push(["cafe\u{301}".to_owned(), "/tmp/café".to_owned()]);
        assert_eq!(
            render(&table, ColorPolicy::disabled()),
            concat!(
                "NAME  PATH\n",
                "服务  /项目/服务\n",
                "cafe\u{301}  /tmp/café\n",
            )
        );
    }

    #[test]
    fn styling_does_not_change_visible_alignment() {
        let mut table = Table::new(["NAME", "PATH"]);
        table.push(["alpha".to_owned(), "/tmp/a".to_owned()]);
        let plain = render(&table, ColorPolicy::disabled());
        let colored = render(&table, ColorPolicy::enabled());
        let stripped = colored
            .replace("\u{1b}[1m\u{1b}[34m", "")
            .replace("\u{1b}[0m", "");
        assert_eq!(stripped, plain);
        assert!(!colored.contains('\t'));
    }
}
