use clap::Parser;

/// DStack v2 command-line interface.
///
/// Phase 1 intentionally exposes only generated help and version output. The
/// approved command tree will be added incrementally with real behavior.
#[derive(Debug, Parser)]
#[command(
    name = "dstack",
    version,
    about = "Docker Compose stack management from anywhere",
    long_about = None
)]
pub struct Cli {}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::Cli;

    #[test]
    fn parses_phase_one_invocation() {
        Cli::try_parse_from(["dstack"]).expect("an invocation without options should parse");
    }

    #[test]
    fn generated_help_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn generated_version_uses_package_metadata() {
        assert_eq!(
            Cli::command().get_version(),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }
}
