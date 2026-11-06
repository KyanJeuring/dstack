use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::ConfigError;

pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_COMPOSE_FILES: [&str; 4] = [
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
];

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
        let paths = vec![
            PathBuf::from("~/projects"),
            PathBuf::from("~/src"),
            PathBuf::from("~/code"),
            #[cfg(unix)]
            PathBuf::from("/opt/services"),
        ];
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

#[cfg(test)]
mod tests {
    use super::{CONFIG_SCHEMA_VERSION, DEFAULT_COMPOSE_FILES, DstackConfig, expand_home};
    use std::path::PathBuf;

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
}
