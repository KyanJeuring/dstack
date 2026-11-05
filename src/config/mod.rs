use std::{
    collections::HashSet,
    env,
    ffi::OsString,
    fmt, fs, io,
    io::Write,
    path::{Component, Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};

/// Overrides DStack's complete user configuration root.
pub const CONFIG_DIR_ENV: &str = "DSTACK_CONFIG_DIR";
pub const CONFIG_SCHEMA_VERSION: u32 = 1;

/// Standard Compose names in the compatibility-preserving v1 priority order.
pub const DEFAULT_COMPOSE_FILES: [&str; 4] = [
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
];

/// Paths used by the v2 application.
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
    /// Resolves the canonical cross-platform DStack root under the native home.
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

    /// Constructs explicit paths without reading process environment.
    pub fn new(application_dir: PathBuf, legacy_registry: PathBuf) -> Self {
        let home_dir = application_dir.join("test-home");
        Self::with_home(application_dir, home_dir, legacy_registry, None)
    }

    /// Constructs an explicit path set for isolated tests and application embedding.
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DstackConfig {
    pub version: u32,
    pub discovery: DiscoveryConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryConfig {
    pub paths: Vec<PathBuf>,
    pub compose_files: Vec<PathBuf>,
}

impl DstackConfig {
    pub fn defaults() -> Self {
        let mut paths = vec![
            PathBuf::from("~/projects"),
            PathBuf::from("~/src"),
            PathBuf::from("~/code"),
        ];
        #[cfg(unix)]
        paths.push(PathBuf::from("/opt/services"));

        Self {
            version: CONFIG_SCHEMA_VERSION,
            discovery: DiscoveryConfig {
                paths,
                compose_files: DEFAULT_COMPOSE_FILES.iter().map(PathBuf::from).collect(),
            },
        }
    }

    pub fn parse(contents: &str, path: &Path) -> Result<Self, ConfigError> {
        let config: Self =
            toml::from_str(contents).map_err(|source| ConfigError::MalformedConfig {
                path: path.to_path_buf(),
                source,
            })?;
        config.validate()?;
        Ok(config)
    }

    pub fn serialize_default() -> Result<String, ConfigError> {
        let config = Self::defaults();
        config.validate()?;
        let body = toml::to_string_pretty(&config).map_err(ConfigError::Serialize)?;
        Ok(format!(
            "# DStack v2 user configuration. This file uses TOML syntax.\n\
             # Missing discovery paths are ignored; add or remove roots as needed.\n\
             # Compose filenames are checked in order and the first match is selected.\n\n\
             {body}"
        ))
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.version != CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchemaVersion {
                found: self.version,
                supported: CONFIG_SCHEMA_VERSION,
            });
        }

        validate_unique_paths("discovery path", &self.discovery.paths, false)?;
        validate_unique_paths("Compose filename", &self.discovery.compose_files, true)?;
        Ok(())
    }

    pub fn expanded_discovery_paths(&self, home: &Path) -> Vec<PathBuf> {
        self.discovery
            .paths
            .iter()
            .map(|path| expand_home(path, home))
            .collect()
    }
}

fn validate_unique_paths(
    label: &'static str,
    paths: &[PathBuf],
    require_filename: bool,
) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();
    for path in paths {
        if path.as_os_str().is_empty() {
            return Err(ConfigError::InvalidValue(format!(
                "{label} must not be empty"
            )));
        }
        let text = path.to_str().ok_or_else(|| {
            ConfigError::InvalidValue(format!("{label} must be valid UTF-8 in dstack.conf"))
        })?;
        if text.chars().any(char::is_control) {
            return Err(ConfigError::InvalidValue(format!(
                "{label} '{}' contains a control character",
                path.display()
            )));
        }
        if !require_filename {
            let starts_with_home =
                matches!(path.components().next(), Some(Component::Normal(part)) if part == "~");
            if !path.is_absolute() && !starts_with_home {
                return Err(ConfigError::InvalidValue(format!(
                    "discovery path '{}' must be absolute or begin with '~'",
                    path.display()
                )));
            }
        }
        if require_filename {
            let mut components = path.components();
            let single_normal = matches!(components.next(), Some(Component::Normal(_)))
                && components.next().is_none();
            if !single_normal {
                return Err(ConfigError::InvalidValue(format!(
                    "Compose filename '{}' must be a relative filename, not a path",
                    path.display()
                )));
            }
        }
        if !seen.insert(path) {
            return Err(ConfigError::InvalidValue(format!(
                "{label} '{}' appears more than once",
                path.display()
            )));
        }
    }
    if paths.is_empty() && require_filename {
        return Err(ConfigError::InvalidValue(
            "at least one Compose filename must be configured".to_owned(),
        ));
    }
    Ok(())
}

/// Expands only a leading standalone `~`; no shell or environment syntax is interpreted.
pub fn expand_home(path: &Path, home: &Path) -> PathBuf {
    let mut components = path.components();
    if matches!(components.next(), Some(Component::Normal(part)) if part == "~") {
        let mut expanded = home.to_path_buf();
        expanded.extend(components);
        expanded
    } else {
        path.to_path_buf()
    }
}

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

    /// Creates a human-editable default only when the configuration is absent.
    ///
    /// The application holds the registry lock while calling this method, so
    /// concurrent DStack bootstrap processes cannot overwrite one another.
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

#[derive(Debug)]
pub enum ConfigError {
    EmptyOverride,
    HomeDirectoryUnavailable,
    UnsupportedSchemaVersion {
        found: u32,
        supported: u32,
    },
    MalformedConfig {
        path: PathBuf,
        source: toml::de::Error,
    },
    Serialize(toml::ser::Error),
    InvalidValue(String),
    MissingParent(PathBuf),
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOverride => write!(
                formatter,
                "{CONFIG_DIR_ENV} is set but does not name a directory"
            ),
            Self::HomeDirectoryUnavailable => {
                formatter.write_str("could not determine the current user's home directory")
            }
            Self::UnsupportedSchemaVersion { found, supported } => write!(
                formatter,
                "configuration schema version {found} is not supported (this DStack supports version {supported})"
            ),
            Self::MalformedConfig { path, source } => write!(
                formatter,
                "could not parse configuration '{}': {source}",
                path.display()
            ),
            Self::Serialize(source) => {
                write!(
                    formatter,
                    "could not serialize default configuration: {source}"
                )
            }
            Self::InvalidValue(reason) => {
                write!(formatter, "invalid DStack configuration: {reason}")
            }
            Self::MissingParent(path) => write!(
                formatter,
                "configuration path '{}' has no usable parent directory",
                path.display()
            ),
            Self::Io {
                action,
                path,
                source,
            } => write!(
                formatter,
                "could not {action} '{}': {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MalformedConfig { source, .. } => Some(source),
            Self::Serialize(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs, path::PathBuf};

    use tempfile::TempDir;

    use super::{
        AppPaths, CONFIG_SCHEMA_VERSION, ConfigError, ConfigStore, DEFAULT_COMPOSE_FILES,
        DstackConfig, expand_home,
    };

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
        .expect("complete native inputs should resolve");

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
        .expect("override should not need native data directories");
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

    #[test]
    fn generated_configuration_is_readable_strict_and_human_friendly() {
        let text = DstackConfig::serialize_default().unwrap();
        assert!(text.contains("# Missing discovery paths are ignored"));
        let config = DstackConfig::parse(&text, std::path::Path::new("dstack.conf")).unwrap();
        assert_eq!(config.version, CONFIG_SCHEMA_VERSION);
        assert_eq!(
            config.discovery.compose_files,
            DEFAULT_COMPOSE_FILES.map(PathBuf::from)
        );
        assert_eq!(
            config.discovery.paths[..3],
            ["~/projects", "~/src", "~/code"].map(PathBuf::from)
        );
    }

    #[test]
    fn strict_validation_rejects_future_unknown_empty_and_duplicate_values() {
        for text in [
            "version = 999\n[discovery]\npaths=[]\ncompose_files=['compose.yaml']\n",
            "version = 1\nunknown=true\n[discovery]\npaths=[]\ncompose_files=['compose.yaml']\n",
            "version = 1\n[discovery]\npaths=['']\ncompose_files=['compose.yaml']\n",
            "version = 1\n[discovery]\npaths=['~/src','~/src']\ncompose_files=['compose.yaml']\n",
            "version = 1\n[discovery]\npaths=['relative/root']\ncompose_files=['compose.yaml']\n",
            "version = 1\n[discovery]\npaths=[]\ncompose_files=[]\n",
            "version = 1\n[discovery]\npaths=[]\ncompose_files=['nested/compose.yaml']\n",
        ] {
            assert!(DstackConfig::parse(text, std::path::Path::new("dstack.conf")).is_err());
        }
    }

    #[test]
    fn home_expansion_is_narrow_and_never_shell_interprets_values() {
        let home = std::path::Path::new("/native/home");
        assert_eq!(
            expand_home(std::path::Path::new("~/projects"), home),
            home.join("projects")
        );
        for literal in ["$HOME/projects", "$(command)", "`command`", "*", "?"] {
            assert_eq!(
                expand_home(std::path::Path::new(literal), home),
                PathBuf::from(literal)
            );
        }
    }

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
