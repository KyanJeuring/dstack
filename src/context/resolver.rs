use std::path::{Path, PathBuf};

use crate::{
    compose::{ComposeSelection, resolve_compose_files},
    config::DstackConfig,
    discovery::discover_stacks,
    error::AppError,
    registry::Registry,
    resolution::{ResolvedStack, resolve_stack},
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
) -> Result<ResolvedContext, AppError> {
    let discovered = discover_stacks(config, home_dir)?;
    let stack = resolve_stack(explicit_stack, current_dir, registry, &discovered, config)?;
    let compose = resolve_compose_files(&stack, explicit_files, config)?;
    Ok(ResolvedContext { stack, compose })
}
