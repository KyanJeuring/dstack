use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;

use super::{Registry, RegistryError, StackProfile};

pub(crate) struct RegistryLock(File);

impl Drop for RegistryLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

#[derive(Debug, Clone)]
pub struct RegistryStore {
    registry_path: PathBuf,
    lock_path: PathBuf,
}

impl RegistryStore {
    pub fn new(registry_path: PathBuf, lock_path: PathBuf) -> Self {
        Self {
            registry_path,
            lock_path,
        }
    }
    pub fn path(&self) -> &Path {
        &self.registry_path
    }
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    pub fn load(&self) -> Result<Registry, RegistryError> {
        match fs::read_to_string(&self.registry_path) {
            Ok(contents) => Registry::parse(&contents, &self.registry_path),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(Registry::default()),
            Err(source) => Err(RegistryError::Io {
                action: "read registry",
                path: self.registry_path.clone(),
                source,
            }),
        }
    }

    pub fn add(&self, profile: StackProfile) -> Result<(), RegistryError> {
        self.mutate(|registry| {
            registry.insert(profile)?;
            Ok(((), true))
        })
    }

    pub fn remove(&self, name: &str) -> Result<StackProfile, RegistryError> {
        self.mutate(|registry| {
            let removed = registry.remove(name)?;
            Ok((removed, true))
        })
    }

    pub(crate) fn mutate<T>(
        &self,
        operation: impl FnOnce(&mut Registry) -> Result<(T, bool), RegistryError>,
    ) -> Result<T, RegistryError> {
        let _lock = self.acquire_lock()?;
        let mut registry = self.load()?;
        let (result, changed) = operation(&mut registry)?;
        if changed {
            self.save(&registry)?;
        }
        Ok(result)
    }

    pub(crate) fn acquire_lock(&self) -> Result<RegistryLock, RegistryError> {
        let parent = self.parent_directory()?;
        fs::create_dir_all(parent).map_err(|source| RegistryError::Io {
            action: "create registry directory",
            path: parent.to_path_buf(),
            source,
        })?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.lock_path)
            .map_err(|source| RegistryError::Io {
                action: "open registry lock",
                path: self.lock_path.clone(),
                source,
            })?;
        match lock.try_lock() {
            Ok(()) => Ok(RegistryLock(lock)),
            Err(std::fs::TryLockError::WouldBlock) => {
                Err(RegistryError::LockBusy(self.lock_path.clone()))
            }
            Err(std::fs::TryLockError::Error(source)) => Err(RegistryError::Io {
                action: "lock registry",
                path: self.lock_path.clone(),
                source,
            }),
        }
    }

    pub(crate) fn initialize_unlocked(
        &self,
        phase2_registry: Option<&Path>,
    ) -> Result<Option<PathBuf>, RegistryError> {
        if self.registry_path.exists() {
            self.load()?;
            return Ok(None);
        }
        let (registry, migrated_from) = match phase2_registry.filter(|path| path.is_file()) {
            Some(path) => {
                let contents = fs::read_to_string(path).map_err(|source| RegistryError::Io {
                    action: "read pre-release Phase 2 registry",
                    path: path.to_path_buf(),
                    source,
                })?;
                (Registry::parse(&contents, path)?, Some(path.to_path_buf()))
            }
            None => (Registry::default(), None),
        };
        self.save(&registry)?;
        Ok(migrated_from)
    }

    pub(crate) fn save(&self, registry: &Registry) -> Result<(), RegistryError> {
        let contents = registry.serialize()?;
        let parent = self.parent_directory()?;
        fs::create_dir_all(parent).map_err(|source| RegistryError::Io {
            action: "create registry directory",
            path: parent.to_path_buf(),
            source,
        })?;
        let mut file =
            AtomicWriteFile::open(&self.registry_path).map_err(|source| RegistryError::Io {
                action: "create registry staging file",
                path: self.registry_path.clone(),
                source,
            })?;
        file.write_all(contents.as_bytes())
            .map_err(|source| RegistryError::Io {
                action: "write registry staging file",
                path: self.registry_path.clone(),
                source,
            })?;
        file.commit().map_err(|source| RegistryError::Io {
            action: "replace registry atomically",
            path: self.registry_path.clone(),
            source,
        })
    }

    fn parent_directory(&self) -> Result<&Path, RegistryError> {
        self.registry_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| RegistryError::MissingParent(self.registry_path.clone()))
    }
}
