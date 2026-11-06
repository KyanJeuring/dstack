//! Compose-file resolution and Docker Compose invocation construction.

mod invocation;
mod resolution;

pub use invocation::ComposeInvocation;
pub use resolution::{
    ComposeError, ComposeSelection, ComposeSelectionSource, resolve_compose_files,
};
