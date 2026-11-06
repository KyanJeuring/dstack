//! Native child-process execution boundary.

mod error;
mod executor;
mod native;
mod spec;

pub use error::ProcessError;
pub use executor::ProcessExecutor;
pub use native::NativeProcessExecutor;
pub use spec::{ProcessSpec, ProcessStatus, StdioPolicy};

#[cfg(test)]
pub(crate) mod testing;
