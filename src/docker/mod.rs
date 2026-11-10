//! Direct Docker CLI construction and machine-output parsing.

mod error;
mod inspect_all;
mod invocation;
mod parse;
mod targets;

pub use error::DockerOutputError;
pub(crate) use inspect_all::{InspectAllResult, InspectResource, inspect_all, write_inspect_json};
pub use invocation::DockerInvocation;
pub(crate) use parse::parse_rows;
pub(crate) use targets::{TARGET_BATCH_SIZE, parse_identifiers};
