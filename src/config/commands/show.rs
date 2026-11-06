use crate::{
    config::{AppPaths, DstackConfig},
    error::AppError,
    output::Output,
};
use std::io::Write;

pub fn run(
    paths: &AppPaths,
    config: &DstackConfig,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    output.write_label_value("Config:", &paths.config_file().display().to_string())?;
    output.write_label_value("Version:", &config.version.to_string())?;
    output.write_label_line("Discovery paths:")?;
    for path in &config.discovery.paths {
        writeln!(output, "  - {}", path.display())?;
    }
    output.write_label_line("Compose filenames (priority order):")?;
    for file in &config.discovery.compose_files {
        writeln!(output, "  - {}", file.display())?;
    }
    Ok(())
}
