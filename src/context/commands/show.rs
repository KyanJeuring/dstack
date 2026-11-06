use std::io::Write;

use crate::{context::ResolvedContext, error::AppError, output::Output};

pub fn run(context: ResolvedContext, output: &mut Output<'_>) -> Result<(), AppError> {
    output.write_label_value("Stack:", &context.stack.name)?;
    output.write_label_value("Source:", &context.stack.source.to_string())?;
    output.write_label_value("Project:", &context.stack.project.display().to_string())?;
    output.write_label_line("Compose files:")?;
    for file in context.compose.files {
        writeln!(output, "  - {}", file.display())?;
    }
    Ok(())
}
