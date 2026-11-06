use std::{io::Write, path::PathBuf};

use crate::{
    error::AppError,
    output::Output,
    registry::{RegistryStore, StackProfile},
};

pub fn run(
    store: &RegistryStore,
    name: String,
    path: PathBuf,
    files: Vec<PathBuf>,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    let profile = StackProfile::for_registration(name, path, files)?;
    let registered_name = profile.name.clone();
    store.add(profile)?;
    writeln!(output, "Registered stack profile '{registered_name}'.")?;
    Ok(())
}
