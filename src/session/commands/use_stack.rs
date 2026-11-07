use std::{io::Write, path::Path};

use crate::{
    compose::resolve_compose_files,
    config::DstackConfig,
    context::ResolvedContext,
    discovery::discover_stacks,
    error::AppError,
    output::{Output, TextStyle},
    registry::Registry,
    resolution::resolve_named_stack,
    shell::{ShellKind, ShellMutation, render_mutation},
};

use super::super::SessionState;

pub struct Environment<'a> {
    pub config: &'a DstackConfig,
    pub registry: &'a Registry,
    pub home_dir: &'a Path,
}

pub struct Request<'a> {
    pub name: &'a str,
    pub explicit_files: &'a [std::path::PathBuf],
    pub shell: ShellKind,
}

pub fn run(
    environment: Environment<'_>,
    request: Request<'_>,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<(), AppError> {
    let discovered = discover_stacks(environment.config, environment.home_dir)?;
    let stack = resolve_named_stack(request.name, environment.registry, &discovered)?;
    let compose = resolve_compose_files(&stack, request.explicit_files, environment.config)?;
    let context = ResolvedContext { stack, compose };
    let state = SessionState::from_context(&context)?;
    let payload = state.serialize()?;
    let mutation = render_mutation(request.shell, ShellMutation::SetSession(&payload));
    writeln!(stdout, "{mutation}")?;
    stderr.write_styled(TextStyle::Success, "[OK]")?;
    writeln!(
        stderr,
        " Using stack '{}' for this terminal",
        context.stack.name
    )?;
    Ok(())
}
