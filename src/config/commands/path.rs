use crate::{config::AppPaths, error::AppError, output::Output};
use std::io::Write;

pub fn run(paths: &AppPaths, output: &mut Output<'_>) -> Result<(), AppError> {
    writeln!(output, "{}", paths.config_file().display())?;
    Ok(())
}
