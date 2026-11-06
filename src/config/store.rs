use super::{ConfigError, DstackConfig};
use atomic_write_file::AtomicWriteFile;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn load(&self) -> Result<DstackConfig, ConfigError> {
        let contents = fs::read_to_string(&self.path).map_err(|source| ConfigError::Io {
            action: "read configuration",
            path: self.path.clone(),
            source,
        })?;
        DstackConfig::parse(&contents, &self.path)
    }
    pub(crate) fn ensure_exists(&self) -> Result<bool, ConfigError> {
        if self.path.exists() {
            self.load()?;
            return Ok(false);
        }
        let parent = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| ConfigError::MissingParent(self.path.clone()))?;
        fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
            action: "create configuration directory",
            path: parent.to_path_buf(),
            source,
        })?;
        let contents = DstackConfig::serialize_default()?;
        let mut file = AtomicWriteFile::open(&self.path).map_err(|source| ConfigError::Io {
            action: "create configuration staging file",
            path: self.path.clone(),
            source,
        })?;
        file.write_all(contents.as_bytes())
            .map_err(|source| ConfigError::Io {
                action: "write configuration staging file",
                path: self.path.clone(),
                source,
            })?;
        file.commit().map_err(|source| ConfigError::Io {
            action: "replace configuration atomically",
            path: self.path.clone(),
            source,
        })?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigStore;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn ensure_exists_creates_once_and_never_replaces_existing_content() {
        let temp = TempDir::new().unwrap();
        let store = ConfigStore::new(temp.path().join("with space λ/dstack.conf"));
        assert!(store.ensure_exists().unwrap());
        let original = fs::read(store.path()).unwrap();
        assert!(!store.ensure_exists().unwrap());
        assert_eq!(fs::read(store.path()).unwrap(), original);
        fs::write(store.path(), "not = [valid").unwrap();
        let malformed = fs::read(store.path()).unwrap();
        assert!(store.ensure_exists().is_err());
        assert_eq!(fs::read(store.path()).unwrap(), malformed);
    }
}
