use super::DockerOutputError;

pub(crate) fn parse_rows(
    command: &'static str,
    output: &[u8],
    expected_fields: usize,
) -> Result<Vec<Vec<String>>, DockerOutputError> {
    let output = std::str::from_utf8(output)
        .map_err(|source| DockerOutputError::InvalidUtf8 { command, source })?;
    let mut rows = Vec::new();
    for (index, line) in output.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let fields = line.split('\t').map(str::to_owned).collect::<Vec<_>>();
        if fields.len() != expected_fields {
            return Err(DockerOutputError::UnexpectedFieldCount {
                command,
                line: index + 1,
                expected: expected_fields,
                actual: fields.len(),
            });
        }
        rows.push(fields);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::parse_rows;

    #[test]
    fn parses_empty_and_tab_delimited_utf8_rows_without_loss() {
        assert!(parse_rows("dstack test", b"", 2).unwrap().is_empty());
        assert_eq!(
            parse_rows("dstack test", "服务\tUp 2 hours\r\nname\t\n".as_bytes(), 2).unwrap(),
            [
                vec!["服务".to_owned(), "Up 2 hours".to_owned()],
                vec!["name".to_owned(), String::new()],
            ]
        );
    }

    #[test]
    fn rejects_invalid_utf8_and_unexpected_field_counts() {
        let utf8 = parse_rows("dstack test", &[0xff], 1).unwrap_err();
        assert!(utf8.to_string().contains("not valid UTF-8"));

        let fields = parse_rows("dstack test", b"one\ttwo\n", 3).unwrap_err();
        assert!(fields.to_string().contains("row 1"));
        assert!(fields.to_string().contains("2 fields; expected 3"));
    }
}
