use std::io::Write;

use crate::{
    config::{AppPaths, ConfigStore, DstackConfig},
    error::AppError,
    output::Output,
    registry::{Registry, RegistryStore},
};

pub(crate) struct RuntimeState {
    pub(crate) config: DstackConfig,
    pub(crate) registry: Registry,
    pub(crate) registry_store: RegistryStore,
}

pub(crate) fn run(paths: &AppPaths, stderr: &mut Output<'_>) -> Result<RuntimeState, AppError> {
    let registry_store = RegistryStore::new(
        paths.registry_file().to_path_buf(),
        paths.registry_lock().to_path_buf(),
    );
    let config_store = ConfigStore::new(paths.config_file().to_path_buf());

    let migrated_from = {
        let _lock = registry_store.acquire_lock()?;
        let migrated = registry_store.initialize_unlocked(paths.phase2_registry())?;
        config_store.ensure_exists()?;
        migrated
    };

    if let Some(source) = migrated_from {
        writeln!(
            stderr,
            "dstack: copied pre-release Phase 2 registry '{}' to '{}'; the source was left untouched",
            source.display(),
            paths.registry_file().display()
        )?;
    }

    Ok(RuntimeState {
        config: config_store.load()?,
        registry: registry_store.load()?,
        registry_store,
    })
}
