use crate::{
    config::{AppPaths, DstackConfig},
    discovery::discover_stacks,
    error::AppError,
    output::{Output, Table},
    registry::Registry,
    resolution::effective_stacks,
};

pub fn run(
    config: &DstackConfig,
    registry: &Registry,
    paths: &AppPaths,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    let discovered = discover_stacks(config, paths.home_dir())?;
    let mut table = Table::new(["NAME", "SOURCE", "PATH"]);
    for stack in effective_stacks(registry, &discovered) {
        let source = if stack.ambiguous {
            "discovered (ambiguous)".to_owned()
        } else {
            stack.source.to_string()
        };
        table.push([stack.name, source, stack.project.display().to_string()]);
    }
    table.write(output)?;
    Ok(())
}
