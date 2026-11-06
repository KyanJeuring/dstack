use std::io::Write;

use crate::{error::AppError, output::Output, registry::RegistryStore};

pub fn run(store: &RegistryStore, name: &str, output: &mut Output<'_>) -> Result<(), AppError> {
    store.remove(name)?;
    writeln!(output, "Removed stack profile '{name}'.")?;
    Ok(())
}
