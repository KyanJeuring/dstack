//! Explicit migrations from legacy DStack storage.

pub mod command;
mod registry;

pub use registry::{MigrationError, MigrationIssue, MigrationReport, migrate_registry};
