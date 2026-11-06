use std::io::Write;

use crate::{
    error::AppError,
    output::Output,
    registry::{Registry, RegistryError},
};

pub fn run(registry: &Registry, name: &str, output: &mut Output<'_>) -> Result<(), AppError> {
    let profile = registry
        .get(name)
        .ok_or_else(|| RegistryError::UnknownProfile(name.to_owned()))?;
    output.write_label_value("Name:", &profile.name)?;
    output.write_label_value("Path:", &profile.path.display().to_string())?;
    if profile.files.is_empty() {
        output.write_label_value("Files:", "-")?;
    } else {
        output.write_label_line("Files:")?;
        for file in &profile.files {
            writeln!(output, "  - {}", file.display())?;
        }
    }
    Ok(())
}
