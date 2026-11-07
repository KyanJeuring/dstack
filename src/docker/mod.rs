//! Direct Docker CLI construction and machine-output parsing.

mod error;
mod invocation;
mod parse;

pub use error::DockerOutputError;
pub use invocation::DockerInvocation;
pub(crate) use parse::parse_rows;
