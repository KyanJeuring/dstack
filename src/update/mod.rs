mod backend;
mod command;
mod error;
mod platform;

pub use backend::{GitHubBackend, UpdateBackend};
pub use command::{
    PACKAGE_MARKER_PATH, PACKAGE_MARKER_VALUE, UpdateEnvironment, UpdateOutcome, run,
};
pub use error::{BackendFailureKind, UpdateError};
pub use platform::{Platform, ReleaseVersion};

#[cfg(test)]
mod tests;
