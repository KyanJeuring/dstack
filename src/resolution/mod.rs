use std::{
    collections::HashSet,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use crate::{
    config::DstackConfig,
    discovery::{DiscoveredStack, group_by_name, has_compose_candidate},
    registry::{Registry, StackProfile},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackSource {
    Registered,
    Discovered,
    CurrentDirectory,
}

impl fmt::Display for StackSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Registered => "registered",
            Self::Discovered => "discovered",
            Self::CurrentDirectory => "cwd",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStack {
    pub name: String,
    pub source: StackSource,
    pub project: PathBuf,
    pub profile_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveStack {
    pub name: String,
    pub source: StackSource,
    pub project: PathBuf,
    pub ambiguous: bool,
}

pub fn effective_stacks(
    registry: &Registry,
    discovered: &[DiscoveredStack],
) -> Vec<EffectiveStack> {
    let registered_names: HashSet<_> = registry
        .stacks
        .iter()
        .map(|profile| profile.name.as_str())
        .collect();
    let grouped = group_by_name(discovered);
    let mut effective = Vec::new();

    for profile in &registry.stacks {
        effective.push(EffectiveStack {
            name: profile.name.clone(),
            source: StackSource::Registered,
            project: profile.path.clone(),
            ambiguous: false,
        });
    }
    for (name, candidates) in grouped {
        if registered_names.contains(name) {
            continue;
        }
        let ambiguous = candidates.len() > 1;
        for candidate in candidates {
            effective.push(EffectiveStack {
                name: candidate.name.clone(),
                source: StackSource::Discovered,
                project: candidate.path.clone(),
                ambiguous,
            });
        }
    }

    effective.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| source_order(left.source).cmp(&source_order(right.source)))
            .then_with(|| left.project.cmp(&right.project))
    });
    effective
}

fn source_order(source: StackSource) -> u8 {
    match source {
        StackSource::Registered => 0,
        StackSource::Discovered => 1,
        StackSource::CurrentDirectory => 2,
    }
}

/// Resolves Phase 3 context: explicit name, then exact current directory.
///
/// A future session layer is intentionally inserted between those branches.
pub fn resolve_stack(
    explicit_name: Option<&str>,
    current_dir: &Path,
    registry: &Registry,
    discovered: &[DiscoveredStack],
    config: &DstackConfig,
) -> Result<ResolvedStack, ResolutionError> {
    if let Some(name) = explicit_name {
        return resolve_named(name, registry, discovered);
    }

    // Future active-session resolution belongs here, before cwd fallback.
    resolve_current_directory(current_dir, config)
}

fn resolve_named(
    name: &str,
    registry: &Registry,
    discovered: &[DiscoveredStack],
) -> Result<ResolvedStack, ResolutionError> {
    if let Some(profile) = registry.get(name) {
        return registered_stack(profile);
    }

    let matches: Vec<_> = discovered
        .iter()
        .filter(|candidate| candidate.name == name)
        .collect();
    match matches.as_slice() {
        [] => Err(ResolutionError::UnknownStack(name.to_owned())),
        [candidate] => Ok(ResolvedStack {
            name: candidate.name.clone(),
            source: StackSource::Discovered,
            project: candidate.path.clone(),
            profile_files: Vec::new(),
        }),
        candidates => Err(ResolutionError::AmbiguousDiscovered {
            name: name.to_owned(),
            paths: candidates
                .iter()
                .map(|candidate| candidate.path.clone())
                .collect(),
        }),
    }
}

fn registered_stack(profile: &StackProfile) -> Result<ResolvedStack, ResolutionError> {
    if !profile.path.is_dir() {
        return Err(ResolutionError::InvalidRegisteredProject {
            name: profile.name.clone(),
            path: profile.path.clone(),
        });
    }
    let project = fs::canonicalize(&profile.path).map_err(|source| ResolutionError::Io {
        action: "resolve registered project",
        path: profile.path.clone(),
        source,
    })?;
    Ok(ResolvedStack {
        name: profile.name.clone(),
        source: StackSource::Registered,
        project,
        profile_files: profile.files.clone(),
    })
}

fn resolve_current_directory(
    current_dir: &Path,
    config: &DstackConfig,
) -> Result<ResolvedStack, ResolutionError> {
    let project = fs::canonicalize(current_dir).map_err(|source| ResolutionError::Io {
        action: "resolve current directory",
        path: current_dir.to_path_buf(),
        source,
    })?;
    if !has_compose_candidate(&project, &config.discovery.compose_files) {
        return Err(ResolutionError::NoContext);
    }
    let name = project
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ResolutionError::InvalidCurrentDirectoryName(project.clone()))?
        .to_owned();
    Ok(ResolvedStack {
        name,
        source: StackSource::CurrentDirectory,
        project,
        profile_files: Vec::new(),
    })
}

#[derive(Debug)]
pub enum ResolutionError {
    UnknownStack(String),
    AmbiguousDiscovered {
        name: String,
        paths: Vec<PathBuf>,
    },
    InvalidRegisteredProject {
        name: String,
        path: PathBuf,
    },
    InvalidCurrentDirectoryName(PathBuf),
    NoContext,
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownStack(name) => write!(
                formatter,
                "stack '{name}' was not found; run 'dstack list' to see available stacks"
            ),
            Self::AmbiguousDiscovered { name, paths } => {
                write!(
                    formatter,
                    "discovered stack name '{name}' is ambiguous across {} projects",
                    paths.len()
                )?;
                for path in paths {
                    write!(formatter, "\n  - {}", path.display())?;
                }
                formatter.write_str(
                    "\nregister distinct profile names with 'dstack registry add NAME PATH'",
                )
            }
            Self::InvalidRegisteredProject { name, path } => write!(
                formatter,
                "registered stack profile '{name}' points to missing or invalid directory '{}'",
                path.display()
            ),
            Self::InvalidCurrentDirectoryName(path) => write!(
                formatter,
                "current Compose project '{}' has a name that is not valid UTF-8",
                path.display()
            ),
            Self::NoContext => formatter.write_str(
                "no DStack context found in the exact current directory; use '--stack <name>' or '-s <name>' and run 'dstack list' to inspect available stacks",
            ),
            Self::Io {
                action,
                path,
                source,
            } => write!(formatter, "could not {action} '{}': {source}", path.display()),
        }
    }
}

impl std::error::Error for ResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::TempDir;

    use super::{StackSource, effective_stacks, resolve_stack};
    use crate::{
        config::{CONFIG_SCHEMA_VERSION, DiscoveryConfig, DstackConfig},
        discovery::DiscoveredStack,
        registry::{Registry, StackProfile},
    };

    fn config() -> DstackConfig {
        DstackConfig {
            version: CONFIG_SCHEMA_VERSION,
            discovery: DiscoveryConfig {
                paths: Vec::new(),
                compose_files: vec![PathBuf::from("compose.yaml")],
            },
        }
    }

    fn project(temp: &TempDir, name: &str, compose: bool) -> PathBuf {
        let path = temp.path().join(name);
        fs::create_dir_all(&path).unwrap();
        if compose {
            fs::write(path.join("compose.yaml"), "services: {}\n").unwrap();
        }
        fs::canonicalize(path).unwrap()
    }

    #[test]
    fn registered_name_is_authoritative_and_explicit_beats_cwd() {
        let temp = TempDir::new().unwrap();
        let registered_path = project(&temp, "registered", true);
        let discovered_path = project(&temp, "discovered", true);
        let cwd = project(&temp, "cwd", true);
        let mut registry = Registry::default();
        registry
            .insert(
                StackProfile::for_registration(
                    "same".to_owned(),
                    registered_path.clone(),
                    Vec::new(),
                )
                .unwrap(),
            )
            .unwrap();
        let discovered = vec![DiscoveredStack {
            name: "same".to_owned(),
            path: discovered_path,
        }];

        let resolved =
            resolve_stack(Some("same"), &cwd, &registry, &discovered, &config()).unwrap();
        assert_eq!(resolved.source, StackSource::Registered);
        assert_eq!(resolved.project, registered_path);
        assert_eq!(effective_stacks(&registry, &discovered).len(), 1);
    }

    #[test]
    fn ambiguous_discovered_name_never_chooses_by_search_order() {
        let temp = TempDir::new().unwrap();
        let discovered = vec![
            DiscoveredStack {
                name: "api".to_owned(),
                path: project(&temp, "one", true),
            },
            DiscoveredStack {
                name: "api".to_owned(),
                path: project(&temp, "two", true),
            },
        ];
        let error = resolve_stack(
            Some("api"),
            temp.path(),
            &Registry::default(),
            &discovered,
            &config(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("ambiguous"));
        assert!(
            effective_stacks(&Registry::default(), &discovered)
                .iter()
                .all(|stack| stack.ambiguous)
        );
    }

    #[test]
    fn cwd_is_exact_and_never_walks_to_parent() {
        let temp = TempDir::new().unwrap();
        let project = project(&temp, "project", true);
        let child = project.join("child");
        fs::create_dir_all(&child).unwrap();

        let resolved = resolve_stack(None, &project, &Registry::default(), &[], &config()).unwrap();
        assert_eq!(resolved.source, StackSource::CurrentDirectory);
        assert!(resolve_stack(None, &child, &Registry::default(), &[], &config()).is_err());
    }
}
