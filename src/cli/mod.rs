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
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage persistent named stack profiles.
    Registry(RegistryArgs),
    /// Explicitly migrate legacy DStack data.
    Migrate(MigrationArgs),
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

    use super::{Cli, Command, MigrationCommand, RegistryCommand};

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
            files,
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
