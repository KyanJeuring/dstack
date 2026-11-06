use super::ConfigError;
use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

pub const CONFIG_DIR_ENV: &str = "DSTACK_CONFIG_DIR";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    application_dir: PathBuf,
    config_file: PathBuf,
    registry_file: PathBuf,
    registry_lock: PathBuf,
    legacy_registry: PathBuf,
    phase2_registry: Option<PathBuf>,
    home_dir: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self, ConfigError> {
        Self::from_inputs(
            env::var_os(CONFIG_DIR_ENV),
            dirs::home_dir(),
            dirs::data_local_dir(),
        )
    }

    fn from_inputs(
        override_dir: Option<OsString>,
        home_dir: Option<PathBuf>,
        phase2_data_dir: Option<PathBuf>,
    ) -> Result<Self, ConfigError> {
        let home_dir = home_dir.ok_or(ConfigError::HomeDirectoryUnavailable)?;
        if let Some(override_dir) = override_dir {
            if override_dir.is_empty() {
                return Err(ConfigError::EmptyOverride);
            }
            let application_dir = PathBuf::from(override_dir);
            let legacy_registry = application_dir.join("legacy").join("registry");
            return Ok(Self::with_home(
                application_dir,
                home_dir,
                legacy_registry,
                None,
            ));
        }
        let application_dir = home_dir.join(".config").join("dstack");
        let legacy_registry = application_dir.join("registry");
        let phase2_registry = phase2_data_dir
            .map(|root| root.join("dstack").join("registry.toml"))
            .filter(|path| path != &application_dir.join("registry.toml"));
        Ok(Self::with_home(
            application_dir,
            home_dir,
            legacy_registry,
            phase2_registry,
        ))
    }

    pub fn new(application_dir: PathBuf, legacy_registry: PathBuf) -> Self {
        let home_dir = application_dir.join("test-home");
        Self::with_home(application_dir, home_dir, legacy_registry, None)
    }

    pub fn with_home(
        application_dir: PathBuf,
        home_dir: PathBuf,
        legacy_registry: PathBuf,
        phase2_registry: Option<PathBuf>,
    ) -> Self {
        let config_file = application_dir.join("dstack.conf");
        let registry_file = application_dir.join("registry.toml");
        let registry_lock = application_dir.join("registry.lock");
        Self {
            application_dir,
            config_file,
            registry_file,
            registry_lock,
            legacy_registry,
            phase2_registry,
            home_dir,
        }
    }

    pub fn application_dir(&self) -> &Path {
        &self.application_dir
    }
    pub fn config_file(&self) -> &Path {
        &self.config_file
    }
    pub fn registry_file(&self) -> &Path {
        &self.registry_file
    }
    pub fn registry_lock(&self) -> &Path {
        &self.registry_lock
    }
    pub fn legacy_registry(&self) -> &Path {
        &self.legacy_registry
    }
    pub fn phase2_registry(&self) -> Option<&Path> {
        self.phase2_registry.as_deref()
    }
    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }
}

#[cfg(test)]
mod tests {
    use super::AppPaths;
    use crate::config::ConfigError;
    use std::{ffi::OsString, path::PathBuf};

    #[test]
    fn explicit_paths_are_deterministic() {
        let paths = AppPaths::with_home(
            PathBuf::from("state-root"),
            PathBuf::from("home-root"),
            PathBuf::from("legacy-root/registry"),
            Some(PathBuf::from("old-root/registry.toml")),
        );
        assert_eq!(paths.config_file(), PathBuf::from("state-root/dstack.conf"));
        assert_eq!(
            paths.registry_file(),
            PathBuf::from("state-root/registry.toml")
        );
        assert_eq!(
            paths.registry_lock(),
            PathBuf::from("state-root/registry.lock")
        );
        assert_eq!(
            paths.legacy_registry(),
            PathBuf::from("legacy-root/registry")
        );
        assert_eq!(
            paths.phase2_registry(),
            Some(std::path::Path::new("old-root/registry.toml"))
        );
        assert_eq!(paths.home_dir(), PathBuf::from("home-root"));
    }

    #[test]
    fn native_policy_uses_home_config_root_and_remembers_phase2_location() {
        let paths = AppPaths::from_inputs(
            None,
            Some(PathBuf::from("native-home")),
            Some(PathBuf::from("native-local-data")),
        )
        .unwrap();
        assert_eq!(
            paths.application_dir(),
            std::path::Path::new("native-home/.config/dstack")
        );
        assert_eq!(
            paths.legacy_registry(),
            std::path::Path::new("native-home/.config/dstack/registry")
        );
        assert_eq!(
            paths.phase2_registry(),
            Some(std::path::Path::new(
                "native-local-data/dstack/registry.toml"
            ))
        );
    }

    #[test]
    fn override_is_self_contained_and_rejects_empty_value() {
        let paths = AppPaths::from_inputs(
            Some(OsString::from("isolated")),
            Some(PathBuf::from("home")),
            None,
        )
        .unwrap();
        assert_eq!(
            paths.config_file(),
            std::path::Path::new("isolated/dstack.conf")
        );
        assert_eq!(
            paths.registry_file(),
            std::path::Path::new("isolated/registry.toml")
        );
        assert_eq!(
            paths.legacy_registry(),
            std::path::Path::new("isolated/legacy/registry")
        );
        assert!(paths.phase2_registry().is_none());
        assert!(matches!(
            AppPaths::from_inputs(Some(OsString::new()), Some(PathBuf::from("home")), None),
            Err(ConfigError::EmptyOverride)
        ));
    }
}
