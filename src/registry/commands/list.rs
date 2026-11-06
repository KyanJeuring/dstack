use std::path::PathBuf;

use crate::{
    error::AppError,
    output::{Output, Table},
    registry::Registry,
};

pub fn run(registry: &Registry, output: &mut Output<'_>) -> Result<(), AppError> {
    let mut table = Table::new(["NAME", "PATH", "FILES"]);
    for profile in &registry.stacks {
        table.push([
            profile.name.clone(),
            profile.path.display().to_string(),
            display_files(&profile.files),
        ]);
    }
    table.write(output)?;
    Ok(())
}

fn display_files(files: &[PathBuf]) -> String {
    if files.is_empty() {
        "-".to_owned()
    } else {
        files
            .iter()
            .map(|file| file.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}
