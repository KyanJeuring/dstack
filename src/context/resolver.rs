use std::path::{Path, PathBuf};

use crate::{
    compose::{ComposeSelection, resolve_compose_files},
    config::DstackConfig,
    discovery::discover_stacks,
    error::AppError,
    registry::Registry,
    resolution::{ResolvedStack, resolve_current_directory_stack, resolve_named_stack},
    session::{SessionEnvironment, resolve_active_session},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContext {
    pub stack: ResolvedStack,
    pub compose: ComposeSelection,
}

pub fn resolve_context(
    config: &DstackConfig,
    registry: &Registry,
    home_dir: &Path,
    explicit_stack: Option<&str>,
    explicit_files: &[PathBuf],
    current_dir: &Path,
    session: &SessionEnvironment,
) -> Result<ResolvedContext, AppError> {
    let stack = if let Some(name) = explicit_stack {
        let discovered = discover_stacks(config, home_dir)?;
        resolve_named_stack(name, registry, &discovered)?
    } else if let Some(stack) = resolve_active_session(session)? {
        stack
    } else {
        resolve_current_directory_stack(current_dir, config)?
    };
    let compose = resolve_compose_files(&stack, explicit_files, config)?;
    Ok(ResolvedContext { stack, compose })
}
