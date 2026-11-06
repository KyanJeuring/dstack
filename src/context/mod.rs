//! Effective stack and Compose-file context.

pub mod commands;
mod resolver;

pub use resolver::{ResolvedContext, resolve_context};
