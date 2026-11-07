pub mod commands;
mod environment;
mod error;
mod model;
mod resolver;

pub use environment::{SESSION_VARIABLE, SessionEnvironment};
pub use error::SessionError;
pub use model::{SESSION_SCHEMA_VERSION, SessionOrigin, SessionState};
pub use resolver::resolve_active_session;

#[cfg(test)]
mod tests;
