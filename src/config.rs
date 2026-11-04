use std::{env, ffi::OsString, fmt, path::PathBuf};

/// Overrides DStack's per-user application directory.
///
/// This is primarily intended for tests, CI, and isolated development. When
/// set, the v2 registry is `<value>/registry.toml` and the legacy migration
/// source is `<value>/legacy/registry`.
pub const CONFIG_DIR_ENV: &str = "DSTACK_CONFIG_DIR";

/// Native paths used by the Phase 2 registry subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    application_dir: PathBuf,
    registry_file: PathBuf,
    registry_lock: PathBuf,
    legacy_registry: PathBuf,
}

impl AppPaths {
    /// Resolves DStack's native per-user paths.
    ///
    /// `dirs::data_local_dir()` maps to XDG data on Linux,
    /// `~/Library/Application Support` on macOS, and LocalAppData on Windows.
    pub fn discover() -> Result<Self, ConfigError> {
        Self::from_inputs(
            env::var_os(CONFIG_DIR_ENV),
            dirs::data_local_dir(),
            dirs::home_dir(),
        )
    }

    fn from_inputs(
        override_dir: Option<OsString>,
        native_data_dir: Option<PathBuf>,
        home_dir: Option<PathBuf>,
    ) -> Result<Self, ConfigError> {
        if let Some(override_dir) = override_dir {
            if override_dir.is_empty() {
                return Err(ConfigError::EmptyOverride);
            }

            let application_dir = PathBuf::from(override_dir);
            let legacy_registry = application_dir.join("legacy").join("registry");
            return Ok(Self::new(application_dir, legacy_registry));
        }

        let application_dir = native_data_dir
            .ok_or(ConfigError::NativeDataDirectoryUnavailable)?
            .join("dstack");
        let legacy_registry = home_dir
            .ok_or(ConfigError::HomeDirectoryUnavailable)?
            .join(".config")
            .join("dstack")
            .join("registry");

        Ok(Self::new(application_dir, legacy_registry))
    }

    /// Constructs an explicit path set without reading process environment.
    pub fn new(application_dir: PathBuf, legacy_registry: PathBuf) -> Self {
        let registry_file = application_dir.join("registry.toml");
        let registry_lock = application_dir.join("registry.lock");
        Self {
            application_dir,
            registry_file,
            registry_lock,
            legacy_registry,
        }
    }

    pub fn application_dir(&self) -> &std::path::Path {
        &self.application_dir
    }

    pub fn registry_file(&self) -> &std::path::Path {
        &self.registry_file
    }

    pub fn registry_lock(&self) -> &std::path::Path {
        &self.registry_lock
    }

    pub fn legacy_registry(&self) -> &std::path::Path {
        &self.legacy_registry
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    EmptyOverride,
    NativeDataDirectoryUnavailable,
    HomeDirectoryUnavailable,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOverride => write!(
                formatter,
                "{CONFIG_DIR_ENV} is set but does not name a directory"
            ),
            Self::NativeDataDirectoryUnavailable => {
                formatter.write_str("could not determine the native per-user data directory")
            }
            Self::HomeDirectoryUnavailable => formatter.write_str(
                "could not determine the home directory containing the legacy v1 registry",
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use super::{AppPaths, ConfigError};

    #[test]
    fn explicit_paths_are_deterministic() {
        let paths = AppPaths::new(
            PathBuf::from("state-root"),
            PathBuf::from("legacy-root/registry"),
        );

        assert_eq!(paths.application_dir(), std::path::Path::new("state-root"));
        assert_eq!(
            paths.registry_file(),
            std::path::Path::new("state-root/registry.toml")
        );
        assert_eq!(
            paths.registry_lock(),
            std::path::Path::new("state-root/registry.lock")
        );
        assert_eq!(
            paths.legacy_registry(),
            std::path::Path::new("legacy-root/registry")
        );
    }

    #[test]
    fn native_policy_uses_local_data_and_legacy_home_locations() {
        let paths = AppPaths::from_inputs(
            None,
            Some(PathBuf::from("native-local-data")),
            Some(PathBuf::from("native-home")),
        )
        .expect("complete native inputs should resolve");

        assert_eq!(
            paths.registry_file(),
            std::path::Path::new("native-local-data/dstack/registry.toml")
        );
        assert_eq!(
            paths.legacy_registry(),
            std::path::Path::new("native-home/.config/dstack/registry")
        );
    }

    #[test]
    fn override_is_self_contained_and_rejects_empty_value() {
        let paths = AppPaths::from_inputs(Some(OsString::from("isolated")), None, None)
            .expect("override should not need native directories");
        assert_eq!(
            paths.registry_file(),
            std::path::Path::new("isolated/registry.toml")
        );
        assert_eq!(
            paths.legacy_registry(),
            std::path::Path::new("isolated/legacy/registry")
        );

        assert_eq!(
            AppPaths::from_inputs(Some(OsString::new()), None, None),
            Err(ConfigError::EmptyOverride)
        );
    }
}
