mod compose;
mod config;
mod context;
mod migration;
mod registry;

use std::{ffi::OsString, path::PathBuf};

use clap::{Parser, Subcommand};

pub use compose::ComposeArgs;
pub use config::{ConfigArgs, ConfigCommand};
pub use context::{ContextArgs, ContextCommand, ContextOptions};
pub use migration::{MigrationArgs, MigrationCommand};
pub use registry::{RegistryArgs, RegistryCommand};

#[derive(Debug, Parser)]
#[command(
  name = "dstack",
  version,
  about = "Docker Compose stack management from anywhere",
  long_about = None
)]
pub struct Cli {
    /// Target a registered or discovered stack by exact name.
    #[arg(short = 's', long = "stack", value_name = "NAME")]
    pub stack: Option<String>,

    /// Select a Compose file explicitly; repeat to preserve a multi-file order.
    #[arg(long = "file", value_name = "FILE", action = clap::ArgAction::Append)]
    pub files: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show all effective registered and discovered stacks.
    List,
    /// Run raw Docker Compose arguments against the resolved stack.
    Compose(ComposeArgs),
    /// Inspect DStack's user configuration.
    Config(ConfigArgs),
    /// Inspect the resolved stack and Compose-file context.
    Context(ContextArgs),
    /// Manage persistent named stack profiles.
    Registry(RegistryArgs),
    /// Explicitly migrate legacy DStack data.
    Migrate(MigrationArgs),
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse_from(compose::normalize_invocation(std::env::args_os()))
    }

    pub fn try_parse_from<I, T>(arguments: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        <Self as Parser>::try_parse_from(compose::normalize_invocation(arguments))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::CommandFactory;

    use super::{Cli, Command, ContextCommand, MigrationCommand, RegistryCommand};

    #[test]
    fn parses_bare_invocation_for_application_diagnostic() {
        let cli = Cli::try_parse_from(["dstack"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_repeated_registry_files() {
        let cli = Cli::try_parse_from([
            "dstack",
            "registry",
            "add",
            "jellyfin-prod",
            "/srv/jellyfin",
            "--file",
            "compose.yaml",
            "--file=compose.prod.yaml",
        ])
        .unwrap();
        let Some(Command::Registry(registry)) = cli.command else {
            panic!("registry command should be selected");
        };
        let RegistryCommand::Add { name, path, files } = registry.command else {
            panic!("registry add should be selected");
        };
        assert_eq!(name, "jellyfin-prod");
        assert_eq!(path, PathBuf::from("/srv/jellyfin"));
        assert_eq!(
            [cli.files, files].concat(),
            [
                PathBuf::from("compose.yaml"),
                PathBuf::from("compose.prod.yaml")
            ]
        );
    }

    #[test]
    fn parses_migration_dry_run() {
        let cli = Cli::try_parse_from(["dstack", "migrate", "registry", "--dry-run"]).unwrap();
        let Some(Command::Migrate(migration)) = cli.command else {
            panic!("migration command should be selected");
        };
        assert!(matches!(
            migration.command,
            MigrationCommand::Registry { dry_run: true }
        ));
    }

    #[test]
    fn global_target_and_files_parse_around_nested_context_command() {
        for argv in [
            vec![
                "dstack",
                "-s",
                "jellyfin",
                "context",
                "show",
                "--file",
                "compose.prod.yaml",
            ],
            vec![
                "dstack",
                "context",
                "show",
                "--file",
                "compose.prod.yaml",
                "-s",
                "jellyfin",
            ],
            vec![
                "dstack",
                "--file",
                "compose.prod.yaml",
                "context",
                "show",
                "-s",
                "jellyfin",
            ],
        ] {
            let cli = Cli::try_parse_from(argv).unwrap();
            let root_stack = cli.stack;
            let root_files = cli.files;
            let Some(Command::Context(context)) = cli.command else {
                panic!("context command should parse");
            };
            let ContextCommand::Show(options) = context.command;
            assert_eq!(
                root_stack.as_deref().or(options.stack.as_deref()),
                Some("jellyfin")
            );
            assert_eq!(
                [root_files, options.files].concat(),
                [PathBuf::from("compose.prod.yaml")]
            );
        }
    }

    #[test]
    fn repeated_files_across_root_and_leaf_keep_argv_order() {
        let cli = Cli::try_parse_from([
            "dstack",
            "--file",
            "compose.yaml",
            "context",
            "show",
            "--file",
            "compose.prod.yaml",
        ])
        .unwrap();
        let root_files = cli.files;
        let Some(Command::Context(context)) = cli.command else {
            panic!("context command should parse");
        };
        let ContextCommand::Show(options) = context.command;
        assert_eq!(
            [root_files, options.files].concat(),
            [
                PathBuf::from("compose.yaml"),
                PathBuf::from("compose.prod.yaml")
            ]
        );
    }

    #[test]
    fn raw_compose_preserves_args_and_recognizes_globals_before_boundary() {
        let cli = Cli::try_parse_from([
            "dstack",
            "compose",
            "down",
            "--file",
            "compose.prod.yaml",
            "-s",
            "jellyfin",
            "-v",
        ])
        .unwrap();

        assert_eq!(cli.stack.as_deref(), Some("jellyfin"));
        assert_eq!(cli.files, [PathBuf::from("compose.prod.yaml")]);
        let Some(Command::Compose(compose)) = cli.command else {
            panic!("compose command should parse");
        };
        assert_eq!(compose.args, ["down", "-v"].map(std::ffi::OsString::from));
    }

    #[test]
    fn hard_boundary_makes_every_following_value_raw() {
        let cli = Cli::try_parse_from([
            "dstack",
            "-s",
            "jellyfin",
            "compose",
            "--",
            "down",
            "--file",
            "special.yaml",
            "-s",
            "raw-stack",
        ])
        .unwrap();

        assert_eq!(cli.stack.as_deref(), Some("jellyfin"));
        assert!(cli.files.is_empty());
        let Some(Command::Compose(compose)) = cli.command else {
            panic!("compose command should parse");
        };
        assert_eq!(
            compose.args,
            ["down", "--file", "special.yaml", "-s", "raw-stack"].map(std::ffi::OsString::from)
        );
    }

    #[test]
    fn compose_short_f_is_always_raw() {
        let cli = Cli::try_parse_from(["dstack", "compose", "logs", "-f"]).unwrap();
        let Some(Command::Compose(compose)) = cli.command else {
            panic!("compose command should parse");
        };
        assert_eq!(compose.args, ["logs", "-f"].map(std::ffi::OsString::from));
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
