use std::collections::HashSet;

use super::DockerOutputError;

/// Keeps host-wide Docker target argv conservative on Unix and Windows.
pub(crate) const TARGET_BATCH_SIZE: usize = 64;

pub(crate) fn parse_identifiers(
    command: &'static str,
    output: &[u8],
) -> Result<Vec<String>, DockerOutputError> {
    let output = std::str::from_utf8(output)
        .map_err(|source| DockerOutputError::InvalidUtf8 { command, source })?;
    let mut seen = HashSet::new();
    let mut identifiers = Vec::new();
    for (index, line) in output.split('\n').enumerate() {
        let identifier = line.strip_suffix('\r').unwrap_or(line);
        if identifier.is_empty() {
            continue;
        }
        if identifier
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(DockerOutputError::InvalidIdentifier {
                command,
                line: index + 1,
            });
        }
        if seen.insert(identifier.to_owned()) {
            identifiers.push(identifier.to_owned());
        }
    }
    Ok(identifiers)
}

#[cfg(test)]
mod tests {
    use super::parse_identifiers;

    #[test]
    fn accepts_crlf_final_lines_blank_lines_and_punctuation_and_deduplicates_in_order() {
        assert_eq!(
            parse_identifiers(
                "test",
                b"sha256:aaa\r\nname.with_punctuation-1\r\n\r\nsha256:aaa\r\nlast"
            )
            .unwrap(),
            ["sha256:aaa", "name.with_punctuation-1", "last"]
        );
        assert!(parse_identifiers("test", b"\n\r\n").unwrap().is_empty());
    }

    #[test]
    fn rejects_invalid_utf8_and_non_line_identifier_whitespace() {
        assert!(matches!(
            parse_identifiers("test", &[0xff]),
            Err(crate::docker::DockerOutputError::InvalidUtf8 { .. })
        ));
        for output in [
            b"bad id\n".as_slice(),
            b"bad\tid\n",
            b"bad\rid\n",
            b"bad\0id\n",
        ] {
            assert!(matches!(
                parse_identifiers("test", output),
                Err(crate::docker::DockerOutputError::InvalidIdentifier { .. })
            ));
        }
    }
}
