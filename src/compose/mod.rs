//! Compose-file resolution and Docker Compose invocation construction.

pub mod commands;
mod invocation;
mod resolution;
pub(crate) mod shell;

pub use invocation::ComposeInvocation;
pub use resolution::{
    ComposeError, ComposeSelection, ComposeSelectionSource, resolve_compose_files,
};
