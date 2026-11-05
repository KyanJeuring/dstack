use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// DStack v2 command-line interface.
///
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
    #[arg(
        long = "file",
        value_name = "FILE",
        action = clap::ArgAction::Append
    )]
    pub files: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show all effective registered and discovered stacks.
    List,
    /// Inspect DStack's user configuration.
    Config(ConfigArgs),
    /// Inspect the resolved stack and Compose-file context.
    Context(ContextArgs),
    /// Manage persistent named stack profiles.
    Registry(RegistryArgs),
    /// Explicitly migrate legacy DStack data.
    Migrate(MigrationArgs),
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the effective dstack.conf path.
    Path,
    /// Show the parsed effective configuration.
    Show,
}

#[derive(Debug, Args)]
pub struct ContextArgs {
    #[command(subcommand)]
    pub command: ContextCommand,
}

#[derive(Debug, Subcommand)]
pub enum ContextCommand {
    /// Show the effective stack, source, project, and Compose files.
    Show(ContextOptions),
}

#[derive(Debug, Args, Default)]
pub struct ContextOptions {
    /// Target a registered or discovered stack by exact name.
    #[arg(short = 's', long = "stack", value_name = "NAME")]
    pub stack: Option<String>,

    /// Select a Compose file explicitly; repeat to preserve order.
    #[arg(long = "file", value_name = "FILE", action = clap::ArgAction::Append)]
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct RegistryArgs {
    #[command(subcommand)]
    pub command: RegistryCommand,
}

#[derive(Debug, Subcommand)]
pub enum RegistryCommand {
    /// List explicitly registered stack profiles.
    List,
    /// Show one registered stack profile.
    Show {
        /// Registered profile name.
        name: String,
    },
    /// Add a named stack profile.
    Add {
        /// Profile name.
        name: String,
        /// Existing project directory.
        path: PathBuf,
        /// Default Compose file, relative to the project or absolute.
        #[arg(long = "file", value_name = "FILE", action = clap::ArgAction::Append)]
        files: Vec<PathBuf>,
    },
    /// Remove a persistent profile without touching project files.
    Remove {
        /// Registered profile name.
        name: String,
    },
}

#[derive(Debug, Args)]
pub struct MigrationArgs {
    #[command(subcommand)]
    pub command: MigrationCommand,
}

#[derive(Debug, Subcommand)]
pub enum MigrationCommand {
    /// Import the legacy v1 registry without modifying its source file.
    Registry {
        /// Preview accepted, skipped, and conflicting entries without writing.
        #[arg(long)]
        dry_run: bool,
    },
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::{CommandFactory, Parser};

    use super::{Cli, Command, ContextCommand, MigrationCommand, RegistryCommand};

    #[test]
    fn parses_bare_invocation_for_application_diagnostic() {
        let cli = Cli::try_parse_from(["dstack"])
            .expect("an invocation without options should reach the application");
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
        .expect("registry add should parse");

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
        let cli = Cli::try_parse_from(["dstack", "migrate", "registry", "--dry-run"])
            .expect("migration dry run should parse");
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
            let cli =
                Cli::try_parse_from(argv).expect("global options should be position-flexible");
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
