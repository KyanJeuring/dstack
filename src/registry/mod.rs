//! Persistent named stack profiles.

pub mod commands;
mod error;
mod model;
mod store;

pub use error::RegistryError;
pub use model::{REGISTRY_SCHEMA_VERSION, Registry, StackProfile};
pub use store::RegistryStore;

#[cfg(test)]
mod tests;
