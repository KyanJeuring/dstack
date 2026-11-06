use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::RegistryError;

pub const REGISTRY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackProfile {
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub files: Vec<PathBuf>,
}

impl StackProfile {
    pub fn for_registration(
        name: String,
        path: PathBuf,
        files: Vec<PathBuf>,
    ) -> Result<Self, RegistryError> {
        validate_name(&name)?;
        validate_project_path_input(&path)?;
        validate_file_list(&files)?;
        if !path.is_dir() {
            return Err(RegistryError::InvalidProjectDirectory(path));
        }
        let path = fs::canonicalize(&path).map_err(|source| RegistryError::Io {
            action: "resolve project directory",
            path: path.clone(),
            source,
        })?;
        validate_stored_path("project path", &path)?;
        Ok(Self { name, path, files })
    }

    fn validate_stored(&self) -> Result<(), RegistryError> {
        validate_name(&self.name)?;
        validate_stored_path("project path", &self.path)?;
        if !self.path.is_absolute() {
            return Err(RegistryError::InvalidStoredProfile {
                name: self.name.clone(),
                reason: "project path is not absolute".to_owned(),
            });
        }
        validate_file_list(&self.files)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub version: u32,
    #[serde(default)]
    pub stacks: Vec<StackProfile>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            version: REGISTRY_SCHEMA_VERSION,
            stacks: Vec::new(),
        }
    }
}

impl Registry {
    pub fn parse(contents: &str, path: &Path) -> Result<Self, RegistryError> {
        let mut registry: Self =
            toml::from_str(contents).map_err(|source| RegistryError::MalformedRegistry {
                path: path.to_path_buf(),
                source,
            })?;
        registry.validate()?;
        registry.sort();
        Ok(registry)
    }

    pub fn serialize(&self) -> Result<String, RegistryError> {
        let mut registry = self.clone();
        registry.validate()?;
        registry.sort();
        toml::to_string_pretty(&registry).map_err(RegistryError::Serialize)
    }

    pub fn get(&self, name: &str) -> Option<&StackProfile> {
        self.stacks.iter().find(|profile| profile.name == name)
    }

    pub fn insert(&mut self, profile: StackProfile) -> Result<(), RegistryError> {
        profile.validate_stored()?;
        if self.get(&profile.name).is_some() {
            return Err(RegistryError::DuplicateName(profile.name));
        }
        self.stacks.push(profile);
        self.sort();
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Result<StackProfile, RegistryError> {
        let index = self
            .stacks
            .iter()
            .position(|profile| profile.name == name)
            .ok_or_else(|| RegistryError::UnknownProfile(name.to_owned()))?;
        Ok(self.stacks.remove(index))
    }

    fn validate(&self) -> Result<(), RegistryError> {
        if self.version != REGISTRY_SCHEMA_VERSION {
            return Err(RegistryError::UnsupportedSchemaVersion {
                found: self.version,
                supported: REGISTRY_SCHEMA_VERSION,
            });
        }
        let mut names = HashSet::new();
        for profile in &self.stacks {
            profile.validate_stored()?;
            if !names.insert(profile.name.as_str()) {
                return Err(RegistryError::DuplicateName(profile.name.clone()));
            }
        }
        Ok(())
    }

    fn sort(&mut self) {
        self.stacks
            .sort_by(|left, right| left.name.cmp(&right.name));
    }
}

fn validate_name(name: &str) -> Result<(), RegistryError> {
    if name.is_empty() || name.chars().all(char::is_whitespace) {
        return Err(RegistryError::InvalidName(
            "profile name must not be empty or only whitespace".to_owned(),
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(RegistryError::InvalidName(
            "profile name must not contain control characters".to_owned(),
        ));
    }
    Ok(())
}

fn validate_project_path_input(path: &Path) -> Result<(), RegistryError> {
    if path.as_os_str().is_empty() {
        return Err(RegistryError::InvalidProjectDirectory(path.to_path_buf()));
    }
    validate_stored_path("project path", path)
}

fn validate_file_list(files: &[PathBuf]) -> Result<(), RegistryError> {
    let mut unique = HashSet::new();
    for file in files {
        if file.as_os_str().is_empty() {
            return Err(RegistryError::InvalidComposeFile(
                "Compose-file value must not be empty".to_owned(),
            ));
        }
        validate_stored_path("Compose-file value", file)?;
        if !unique.insert(file) {
            return Err(RegistryError::DuplicateComposeFile(file.clone()));
        }
    }
    Ok(())
}

fn validate_stored_path(label: &'static str, path: &Path) -> Result<(), RegistryError> {
    let text = path.to_str().ok_or_else(|| RegistryError::NonUtf8Path {
        label,
        path: path.to_path_buf(),
    })?;
    if text.chars().any(char::is_control) {
        return Err(RegistryError::InvalidPathText {
            label,
            path: path.to_path_buf(),
        });
    }
    Ok(())
}
