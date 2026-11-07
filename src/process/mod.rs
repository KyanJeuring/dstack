//! Native child-process execution boundary.

mod error;
mod executor;
mod native;
mod spec;

pub use error::ProcessError;
pub use executor::{ProcessExecutor, ProcessOutputExecutor};
pub use native::NativeProcessExecutor;
pub use spec::{
    CapturePolicy, CapturedProcess, CapturedProcessSpec, ProcessSpec, ProcessStatus, StdioPolicy,
};

#[cfg(test)]
pub(crate) mod testing;
