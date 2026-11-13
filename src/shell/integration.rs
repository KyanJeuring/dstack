use std::{ops::Range, path::Path};

use super::{ShellError, ShellKind};

pub const MANAGED_BLOCK_START: &str = "# >>> DStack shell integration >>>";
pub const MANAGED_BLOCK_END: &str = "# <<< DStack shell integration <<<";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedIntegration {
    Missing,
    Current,
    Outdated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualLoaderOccurrence {
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileIntegration {
    NotInstalled,
    ManagedCurrent,
    ManagedOutdated,
    ManualCurrent,
    ManualLegacy,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileIntegrationAnalysis {
    pub managed: ManagedIntegration,
    pub manual_current: Vec<ManualLoaderOccurrence>,
    pub manual_legacy: Vec<ManualLoaderOccurrence>,
    pub(crate) managed_range: Option<Range<usize>>,
}

impl ProfileIntegrationAnalysis {
    pub fn integration(&self) -> ProfileIntegration {
        let manual_count = self.manual_current.len() + self.manual_legacy.len();
        if (self.managed != ManagedIntegration::Missing && manual_count != 0) || manual_count > 1 {
            return ProfileIntegration::Duplicate;
        }

        match (
            self.managed,
            self.manual_current.len(),
            self.manual_legacy.len(),
        ) {
            (ManagedIntegration::Current, 0, 0) => ProfileIntegration::ManagedCurrent,
            (ManagedIntegration::Outdated, 0, 0) => ProfileIntegration::ManagedOutdated,
            (ManagedIntegration::Missing, 1, 0) => ProfileIntegration::ManualCurrent,
            (ManagedIntegration::Missing, 0, 1) => ProfileIntegration::ManualLegacy,
            (ManagedIntegration::Missing, 0, 0) => ProfileIntegration::NotInstalled,
            _ => ProfileIntegration::Duplicate,
        }
    }

    pub fn has_manual_integration(&self) -> bool {
        !self.manual_current.is_empty() || !self.manual_legacy.is_empty()
    }
}

pub fn manual_loader(shell: ShellKind, line_ending: &str) -> String {
    match shell {
        ShellKind::Bash => "eval \"$(command dstack shell __init bash)\"".to_owned(),
        ShellKind::Zsh => "eval \"$(command dstack shell __init zsh)\"".to_owned(),
        ShellKind::Fish => "command dstack shell __init fish | source".to_owned(),
        ShellKind::Powershell => [
            "& {",
            "  $dstackApplication = (Get-Command dstack -CommandType Application -ErrorAction Stop).Source",
            "  Invoke-Expression ((& $dstackApplication shell __init powershell) -join \"`n\")",
            "}",
        ]
        .join(line_ending),
    }
}

pub fn legacy_manual_loader(shell: ShellKind) -> &'static str {
    match shell {
        ShellKind::Bash => "eval \"$(dstack shell init bash)\"",
        ShellKind::Zsh => "eval \"$(dstack shell init zsh)\"",
        ShellKind::Fish => "dstack shell init fish | source",
        ShellKind::Powershell => {
            "Invoke-Expression ((& dstack shell init powershell) -join \"`n\")"
        }
    }
}

pub fn managed_block(shell: ShellKind, line_ending: &str) -> String {
    let loader = manual_loader(shell, line_ending);
    match shell {
        ShellKind::Bash | ShellKind::Zsh => [
            MANAGED_BLOCK_START.to_owned(),
            "if command -v dstack >/dev/null 2>&1; then".to_owned(),
            format!("  {loader}"),
            "fi".to_owned(),
            MANAGED_BLOCK_END.to_owned(),
        ]
        .join(line_ending),
        ShellKind::Fish => [
            MANAGED_BLOCK_START.to_owned(),
            "if type -q dstack".to_owned(),
            format!("    {loader}"),
            "end".to_owned(),
            MANAGED_BLOCK_END.to_owned(),
        ]
        .join(line_ending),
        ShellKind::Powershell => [
            MANAGED_BLOCK_START,
            "& {",
            "  $dstackApplication = (Get-Command dstack -CommandType Application -ErrorAction SilentlyContinue).Source",
            "  if ($null -ne $dstackApplication) {",
            "    Invoke-Expression ((& $dstackApplication shell __init powershell) -join \"`n\")",
            "  }",
            "}",
            MANAGED_BLOCK_END,
        ]
        .join(line_ending),
    }
}

pub fn analyze_profile_contents(
    path: &Path,
    shell: ShellKind,
    contents: &str,
    line_ending: &str,
) -> Result<ProfileIntegrationAnalysis, ShellError> {
    let lines = profile_lines(contents);
    let managed_range = parse_managed_block(path, &lines)?;
    let managed = match managed_range.clone() {
        None => ManagedIntegration::Missing,
        Some(range) => {
            let mut expected = managed_block(shell, line_ending);
            if contents[range.clone()].ends_with('\n') {
                expected.push_str(line_ending);
            }
            if contents[range] == expected {
                ManagedIntegration::Current
            } else {
                ManagedIntegration::Outdated
            }
        }
    };
    let external = lines
        .iter()
        .map(|line| {
            let inside_managed = managed_range
                .as_ref()
                .is_some_and(|range| line.start >= range.start && line.start < range.end);
            ExternalLine {
                text: line.text,
                number: line.number,
                inside_managed,
            }
        })
        .collect::<Vec<_>>();
    let manual_current = find_current_loaders(shell, &external);
    let manual_legacy = find_legacy_loaders(shell, &external);

    Ok(ProfileIntegrationAnalysis {
        managed,
        manual_current,
        manual_legacy,
        managed_range,
    })
}

#[derive(Debug, Clone, Copy)]
struct ProfileLine<'a> {
    text: &'a str,
    number: usize,
    start: usize,
    full_end: usize,
}

#[derive(Debug, Clone, Copy)]
struct ExternalLine<'a> {
    text: &'a str,
    number: usize,
    inside_managed: bool,
}

fn profile_lines(contents: &str) -> Vec<ProfileLine<'_>> {
    let mut lines = Vec::new();
    let mut offset = 0;
    let mut number = 1;
    while offset < contents.len() {
        let remaining = &contents[offset..];
        let (raw_end, full_end) = match remaining.find('\n') {
            Some(index) => (offset + index, offset + index + 1),
            None => (contents.len(), contents.len()),
        };
        let text_end = if raw_end > offset && contents.as_bytes()[raw_end - 1] == b'\r' {
            raw_end - 1
        } else {
            raw_end
        };
        lines.push(ProfileLine {
            text: &contents[offset..text_end],
            number,
            start: offset,
            full_end,
        });
        offset = full_end;
        number += 1;
    }
    lines
}

fn parse_managed_block(
    path: &Path,
    lines: &[ProfileLine<'_>],
) -> Result<Option<Range<usize>>, ShellError> {
    let starts = lines
        .iter()
        .filter(|line| line.text == MANAGED_BLOCK_START)
        .collect::<Vec<_>>();
    let ends = lines
        .iter()
        .filter(|line| line.text == MANAGED_BLOCK_END)
        .collect::<Vec<_>>();
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(None),
        ([start], [end]) if start.start < end.start => Ok(Some(start.start..end.full_end)),
        ([start], [end]) => Err(ShellError::CorruptManagedBlock {
            path: path.to_path_buf(),
            reason: format!(
                "closing marker at byte {} precedes opening marker at byte {}",
                end.start, start.start
            ),
        }),
        _ => Err(ShellError::CorruptManagedBlock {
            path: path.to_path_buf(),
            reason: format!(
                "found {} opening marker(s) and {} closing marker(s)",
                starts.len(),
                ends.len()
            ),
        }),
    }
}

fn find_current_loaders(
    shell: ShellKind,
    lines: &[ExternalLine<'_>],
) -> Vec<ManualLoaderOccurrence> {
    let loader = manual_loader(shell, "\n");
    let expected = loader.lines().map(str::trim).collect::<Vec<_>>();
    find_exact_sequences(lines, &expected)
}

fn find_legacy_loaders(
    shell: ShellKind,
    lines: &[ExternalLine<'_>],
) -> Vec<ManualLoaderOccurrence> {
    find_exact_sequences(lines, &[legacy_manual_loader(shell)])
}

fn find_exact_sequences(
    lines: &[ExternalLine<'_>],
    expected: &[&str],
) -> Vec<ManualLoaderOccurrence> {
    if expected.is_empty() || lines.len() < expected.len() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for index in 0..=lines.len() - expected.len() {
        let candidate = &lines[index..index + expected.len()];
        if candidate.iter().any(|line| line.inside_managed) {
            continue;
        }
        if candidate
            .iter()
            .zip(expected)
            .all(|(line, expected)| line.text.trim() == *expected)
        {
            matches.push(ManualLoaderOccurrence {
                line: candidate[0].number,
            });
        }
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyze(shell: ShellKind, contents: &str) -> ProfileIntegrationAnalysis {
        analyze_profile_contents(Path::new("profile"), shell, contents, "\n").unwrap()
    }

    #[test]
    fn canonical_manual_loaders_are_detected_for_every_shell() {
        for shell in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::Fish,
            ShellKind::Powershell,
        ] {
            let contents = format!("before\n{}\nafter\n", manual_loader(shell, "\n"));
            let analysis = analyze(shell, &contents);
            assert_eq!(analysis.integration(), ProfileIntegration::ManualCurrent);
            assert_eq!(analysis.manual_current[0].line, 2);
        }
    }

    #[test]
    fn managed_loader_is_excluded_from_manual_detection() {
        for shell in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::Fish,
            ShellKind::Powershell,
        ] {
            let analysis = analyze(shell, &format!("{}\n", managed_block(shell, "\n")));
            assert_eq!(analysis.integration(), ProfileIntegration::ManagedCurrent);
            assert!(analysis.manual_current.is_empty());
            assert!(analysis.manual_legacy.is_empty());
        }
    }

    #[test]
    fn comments_wrong_shell_and_string_literals_are_not_loaders() {
        for shell in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::Fish,
            ShellKind::Powershell,
        ] {
            let commented = manual_loader(shell, "\n")
                .lines()
                .map(|line| format!("# {line}"))
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(
                analyze(shell, &commented).integration(),
                ProfileIntegration::NotInstalled
            );
        }
        let contents = r#"# eval "$(command dstack shell __init bash)"
eval "$(command dstack shell __init zsh)"
echo 'eval "$(command dstack shell __init bash)"'
MY_VALUE='command dstack shell __init fish | source'
"#;
        assert_eq!(
            analyze(ShellKind::Bash, contents).integration(),
            ProfileIntegration::NotInstalled
        );
    }

    #[test]
    fn legacy_and_duplicate_forms_are_classified_without_repair() {
        let legacy = legacy_manual_loader(ShellKind::Bash);
        assert_eq!(
            analyze(ShellKind::Bash, legacy).integration(),
            ProfileIntegration::ManualLegacy
        );
        let current = manual_loader(ShellKind::Bash, "\n");
        assert_eq!(
            analyze(ShellKind::Bash, &format!("{current}\n{current}\n")).integration(),
            ProfileIntegration::Duplicate
        );
        assert_eq!(
            analyze(ShellKind::Bash, &format!("{current}\n{legacy}\n")).integration(),
            ProfileIntegration::Duplicate
        );
        assert_eq!(
            analyze(
                ShellKind::Bash,
                &format!("{}\n{current}\n", managed_block(ShellKind::Bash, "\n"))
            )
            .integration(),
            ProfileIntegration::Duplicate
        );
    }

    #[test]
    fn detection_accepts_only_line_whitespace_and_crlf_differences() {
        for shell in [
            ShellKind::Bash,
            ShellKind::Zsh,
            ShellKind::Fish,
            ShellKind::Powershell,
        ] {
            let current = manual_loader(shell, "\r\n");
            let contents = current
                .lines()
                .map(|line| format!("  {line}  \r\n"))
                .collect::<String>();
            let analysis =
                analyze_profile_contents(Path::new("profile"), shell, &contents, "\r\n").unwrap();
            assert_eq!(analysis.integration(), ProfileIntegration::ManualCurrent);
        }
        let current = manual_loader(ShellKind::Bash, "\n");
        assert_eq!(
            analyze(ShellKind::Bash, &format!("{current}; echo later\n")).integration(),
            ProfileIntegration::NotInstalled
        );
    }

    #[test]
    fn phase7_manual_loaders_and_managed_blocks_remain_byte_for_byte_stable() {
        let cases = [
            (
                ShellKind::Bash,
                "eval \"$(command dstack shell __init bash)\"",
                "# >>> DStack shell integration >>>\nif command -v dstack >/dev/null 2>&1; then\n  eval \"$(command dstack shell __init bash)\"\nfi\n# <<< DStack shell integration <<<",
            ),
            (
                ShellKind::Zsh,
                "eval \"$(command dstack shell __init zsh)\"",
                "# >>> DStack shell integration >>>\nif command -v dstack >/dev/null 2>&1; then\n  eval \"$(command dstack shell __init zsh)\"\nfi\n# <<< DStack shell integration <<<",
            ),
            (
                ShellKind::Fish,
                "command dstack shell __init fish | source",
                "# >>> DStack shell integration >>>\nif type -q dstack\n    command dstack shell __init fish | source\nend\n# <<< DStack shell integration <<<",
            ),
            (
                ShellKind::Powershell,
                "& {\n  $dstackApplication = (Get-Command dstack -CommandType Application -ErrorAction Stop).Source\n  Invoke-Expression ((& $dstackApplication shell __init powershell) -join \"`n\")\n}",
                "# >>> DStack shell integration >>>\n& {\n  $dstackApplication = (Get-Command dstack -CommandType Application -ErrorAction SilentlyContinue).Source\n  if ($null -ne $dstackApplication) {\n    Invoke-Expression ((& $dstackApplication shell __init powershell) -join \"`n\")\n  }\n}\n# <<< DStack shell integration <<<",
            ),
        ];

        for (shell, expected_loader, expected_block) in cases {
            assert_eq!(manual_loader(shell, "\n"), expected_loader);
            assert_eq!(managed_block(shell, "\n"), expected_block);
        }
    }
}
