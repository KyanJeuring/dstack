use std::{collections::HashSet, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{context::ResolvedContext, resolution::StackSource};

use super::SessionError;

pub const SESSION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionOrigin {
    Registered,
    Discovered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionState {
    pub version: u32,
    pub name: String,
    pub project: PathBuf,
    pub files: Vec<PathBuf>,
    pub origin: SessionOrigin,
}

impl SessionState {
    pub fn from_context(context: &ResolvedContext) -> Result<Self, SessionError> {
        let origin = match context.stack.source {
            StackSource::Registered => SessionOrigin::Registered,
            StackSource::Discovered => SessionOrigin::Discovered,
            StackSource::CurrentDirectory | StackSource::Session => {
                return Err(SessionError::InvalidState(
                    "activation must resolve an explicit named stack".to_owned(),
                ));
            }
        };
        let state = Self {
            version: SESSION_SCHEMA_VERSION,
            name: context.stack.name.clone(),
            project: context.stack.project.clone(),
            files: context.compose.files.clone(),
            origin,
        };
        state.validate_structure()?;
        Ok(state)
    }

    pub fn parse(payload: &str) -> Result<Self, SessionError> {
        let state: Self = serde_json::from_str(payload).map_err(SessionError::Malformed)?;
        state.validate_structure()?;
        Ok(state)
    }

    pub fn serialize(&self) -> Result<String, SessionError> {
        self.validate_structure()?;
        serde_json::to_string(self).map_err(SessionError::Serialize)
    }

    pub(crate) fn validate_usable(&self) -> Result<(), SessionError> {
        if !self.project.is_dir() {
            return Err(SessionError::StaleProject(self.project.clone()));
        }
        let canonical =
            fs::canonicalize(&self.project).map_err(|source| SessionError::ResolveProject {
                path: self.project.clone(),
                source,
            })?;
        if canonical != self.project {
            return Err(SessionError::InvalidState(format!(
                "project path '{}' is not canonical",
                self.project.display()
            )));
        }
        for file in &self.files {
            if !file.is_file() {
                return Err(SessionError::StaleComposeFile(file.clone()));
            }
        }
        Ok(())
    }

    fn validate_structure(&self) -> Result<(), SessionError> {
        if self.version != SESSION_SCHEMA_VERSION {
            return Err(SessionError::UnsupportedSchemaVersion {
                found: self.version,
                supported: SESSION_SCHEMA_VERSION,
            });
        }
        if self.name.is_empty() || self.name.chars().all(char::is_whitespace) {
            return Err(SessionError::InvalidState(
                "stack name must not be empty or only whitespace".to_owned(),
            ));
        }
        if self.name.chars().any(char::is_control) {
            return Err(SessionError::InvalidState(
                "stack name must not contain control characters".to_owned(),
            ));
        }
        validate_path("project path", &self.project)?;
        if !self.project.is_absolute() {
            return Err(SessionError::InvalidState(
                "project path must be absolute".to_owned(),
            ));
        }
        if self.files.is_empty() {
            return Err(SessionError::InvalidState(
                "at least one Compose file is required".to_owned(),
            ));
        }
        let mut unique = HashSet::new();
        for file in &self.files {
            validate_path("Compose-file path", file)?;
            if !file.is_absolute() {
                return Err(SessionError::InvalidState(format!(
                    "Compose-file path '{}' must be absolute",
                    file.display()
                )));
            }
            if !unique.insert(file) {
                return Err(SessionError::InvalidState(format!(
                    "Compose-file path '{}' appears more than once",
                    file.display()
                )));
            }
        }
        Ok(())
    }
}

fn validate_path(label: &'static str, path: &std::path::Path) -> Result<(), SessionError> {
    if path.as_os_str().is_empty() {
        return Err(SessionError::InvalidState(format!(
            "{label} must not be empty"
        )));
    }
    let text = path.to_str().ok_or_else(|| SessionError::NonUtf8Path {
        label,
        path: path.to_path_buf(),
    })?;
    if text.chars().any(char::is_control) {
        return Err(SessionError::InvalidState(format!(
            "{label} must not contain control characters"
        )));
    }
    Ok(())
}
