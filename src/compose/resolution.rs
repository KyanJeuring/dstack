use std::{collections::HashSet, fmt, path::PathBuf};

use crate::{config::DstackConfig, resolution::ResolvedStack};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeSelectionSource {
    Explicit,
    RegisteredProfile,
    Session,
    Automatic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeSelection {
    pub source: ComposeSelectionSource,
    pub files: Vec<PathBuf>,
}

/// Resolves Compose files without invoking Docker or interpreting shell syntax.
pub fn resolve_compose_files(
    stack: &ResolvedStack,
    explicit_files: &[PathBuf],
    config: &DstackConfig,
) -> Result<ComposeSelection, ComposeError> {
    let (source, selected) = if !explicit_files.is_empty() {
        (ComposeSelectionSource::Explicit, explicit_files)
    } else if !stack.profile_files.is_empty() {
        (
            if stack.source == crate::resolution::StackSource::Session {
                ComposeSelectionSource::Session
            } else {
                ComposeSelectionSource::RegisteredProfile
            },
            stack.profile_files.as_slice(),
        )
    } else {
        let file = config
            .discovery
            .compose_files
            .iter()
            .map(|candidate| stack.project.join(candidate))
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| ComposeError::NoComposeFile(stack.project.clone()))?;
        return Ok(ComposeSelection {
            source: ComposeSelectionSource::Automatic,
            files: vec![file],
        });
    };

    let mut seen = HashSet::new();
    let mut files = Vec::with_capacity(selected.len());
    for selected_file in selected {
        if selected_file.as_os_str().is_empty() {
            return Err(ComposeError::EmptyFileSelection);
        }
        let resolved = if selected_file.is_absolute() {
            selected_file.clone()
        } else {
            stack.project.join(selected_file)
        };
        if !seen.insert(resolved.clone()) {
            return Err(ComposeError::DuplicateFile(resolved));
        }
        if !resolved.is_file() {
            return Err(ComposeError::MissingSelectedFile {
                source,
                path: resolved,
            });
        }
        files.push(resolved);
    }

    Ok(ComposeSelection { source, files })
}

#[derive(Debug)]
pub enum ComposeError {
    NoComposeFile(PathBuf),
    EmptyFileSelection,
    DuplicateFile(PathBuf),
    MissingSelectedFile {
        source: ComposeSelectionSource,
        path: PathBuf,
    },
}

impl fmt::Display for ComposeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoComposeFile(project) => write!(
                formatter,
                "no configured Compose file was found in project '{}'",
                project.display()
            ),
            Self::EmptyFileSelection => {
                formatter.write_str("Compose-file selection must not be empty")
            }
            Self::DuplicateFile(path) => write!(
                formatter,
                "Compose file '{}' was selected more than once",
                path.display()
            ),
            Self::MissingSelectedFile { source, path } => write!(
                formatter,
                "{} Compose file '{}' does not exist or is not a regular file",
                match source {
                    ComposeSelectionSource::Explicit => "explicitly selected",
                    ComposeSelectionSource::RegisteredProfile => "registered profile",
                    ComposeSelectionSource::Session => "terminal session",
                    ComposeSelectionSource::Automatic => "automatically selected",
                },
                path.display()
            ),
        }
    }
}

impl std::error::Error for ComposeError {}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::TempDir;

    use super::{ComposeSelectionSource, resolve_compose_files};
    use crate::{
        config::{CONFIG_SCHEMA_VERSION, DiscoveryConfig, DstackConfig},
        resolution::{ResolvedStack, StackSource},
    };

    fn config(candidates: &[&str]) -> DstackConfig {
        DstackConfig {
            version: CONFIG_SCHEMA_VERSION,
            discovery: DiscoveryConfig {
                paths: Vec::new(),
                compose_files: candidates.iter().map(PathBuf::from).collect(),
            },
        }
    }

    fn stack(temp: &TempDir, profile_files: &[&str]) -> ResolvedStack {
        ResolvedStack {
            name: "stack".to_owned(),
            source: StackSource::Registered,
            project: temp.path().to_path_buf(),
            profile_files: profile_files.iter().map(PathBuf::from).collect(),
        }
    }

    fn file(temp: &TempDir, name: &str) -> PathBuf {
        let path = temp.path().join(name);
        fs::write(&path, "services: {}\n").unwrap();
        path
    }

    #[test]
    fn explicit_files_replace_profile_defaults_and_preserve_order() {
        let temp = TempDir::new().unwrap();
        file(&temp, "profile.yaml");
        let first = file(&temp, "-explicit one.yaml");
        let second = file(&temp, "explicit two.yaml");

        let selection = resolve_compose_files(
            &stack(&temp, &["profile.yaml"]),
            &[PathBuf::from("-explicit one.yaml"), second.clone()],
            &config(&["profile.yaml"]),
        )
        .unwrap();
        assert_eq!(selection.source, ComposeSelectionSource::Explicit);
        assert_eq!(selection.files, [first, second]);
    }

    #[test]
    fn profile_files_resolve_relative_and_absolute_values_in_order() {
        let temp = TempDir::new().unwrap();
        let relative = file(&temp, "compose.yaml");
        let external = TempDir::new().unwrap();
        let absolute = file(&external, "compose.prod.yaml");
        let stack = ResolvedStack {
            profile_files: vec![PathBuf::from("compose.yaml"), absolute.clone()],
            ..stack(&temp, &[])
        };

        let selection = resolve_compose_files(&stack, &[], &config(&["other.yaml"])).unwrap();
        assert_eq!(selection.source, ComposeSelectionSource::RegisteredProfile);
        assert_eq!(selection.files, [relative, absolute]);
    }

    #[test]
    fn automatic_selection_uses_first_configured_existing_candidate() {
        let temp = TempDir::new().unwrap();
        let lower_priority = file(&temp, "compose.yaml");
        let preferred = file(&temp, "docker-compose.yml");

        let selection = resolve_compose_files(
            &stack(&temp, &[]),
            &[],
            &config(&["docker-compose.yml", "compose.yaml"]),
        )
        .unwrap();
        assert_eq!(selection.source, ComposeSelectionSource::Automatic);
        assert_eq!(selection.files, [preferred]);
        assert_ne!(selection.files, [lower_priority]);
    }

    #[test]
    fn missing_explicit_or_profile_file_never_falls_back() {
        let temp = TempDir::new().unwrap();
        file(&temp, "compose.yaml");

        assert!(
            resolve_compose_files(
                &stack(&temp, &[]),
                &[PathBuf::from("missing.yaml")],
                &config(&["compose.yaml"]),
            )
            .is_err()
        );
        assert!(
            resolve_compose_files(
                &stack(&temp, &["missing.yaml"]),
                &[],
                &config(&["compose.yaml"]),
            )
            .is_err()
        );
    }

    #[test]
    fn absolute_explicit_file_stays_absolute_and_duplicate_selection_fails() {
        let temp = TempDir::new().unwrap();
        let external = TempDir::new().unwrap();
        let absolute = file(&external, "outside compose.yaml");
        let selection = resolve_compose_files(
            &stack(&temp, &[]),
            std::slice::from_ref(&absolute),
            &config(&["compose.yaml"]),
        )
        .unwrap();
        assert_eq!(selection.files.as_slice(), std::slice::from_ref(&absolute));

        assert!(
            resolve_compose_files(
                &stack(&temp, &[]),
                &[absolute.clone(), absolute],
                &config(&["compose.yaml"]),
            )
            .is_err()
        );
    }

    #[test]
    fn missing_automatic_candidate_is_a_typed_error() {
        let temp = TempDir::new().unwrap();
        let error = resolve_compose_files(&stack(&temp, &[]), &[], &config(&["compose.yaml"]))
            .expect_err("a project without a candidate should fail");
        assert!(error.to_string().contains("no configured Compose file"));
    }
}
