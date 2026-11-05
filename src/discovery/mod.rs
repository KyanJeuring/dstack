use std::{
    collections::{BTreeMap, HashSet},
    fmt, fs, io,
    path::{Path, PathBuf},
};

use crate::config::DstackConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredStack {
    pub name: String,
    pub path: PathBuf,
}

/// Discovers direct-child Compose projects under the configured roots.
///
/// Direct child symlinks are followed once and canonicalized. DStack never
/// recursively walks a discovered project, so symlink cycles cannot recurse.
pub fn discover_stacks(
    config: &DstackConfig,
    home: &Path,
) -> Result<Vec<DiscoveredStack>, DiscoveryError> {
    let mut unique = HashSet::new();
    let mut stacks = Vec::new();

    for configured_root in config.expanded_discovery_paths(home) {
        let root = match fs::canonicalize(&configured_root) {
            Ok(root) => root,
            Err(source) if source.kind() == io::ErrorKind::NotFound => continue,
            Err(source) => {
                return Err(DiscoveryError::Io {
                    action: "resolve discovery root",
                    path: configured_root,
                    source,
                });
            }
        };
        let entries = fs::read_dir(&root).map_err(|source| DiscoveryError::Io {
            action: "read discovery root",
            path: root.clone(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| DiscoveryError::Io {
                action: "read discovery entry",
                path: root.clone(),
                source,
            })?;
            let entry_path = entry.path();
            if !entry_path.is_dir() {
                continue;
            }

            let project = fs::canonicalize(&entry_path).map_err(|source| DiscoveryError::Io {
                action: "resolve discovered project",
                path: entry_path.clone(),
                source,
            })?;
            if !has_compose_candidate(&project, &config.discovery.compose_files) {
                continue;
            }

            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| DiscoveryError::NonUtf8Name(entry_path.clone()))?;
            if unique.insert((name.clone(), project.clone())) {
                stacks.push(DiscoveredStack {
                    name,
                    path: project,
                });
            }
        }
    }

    stacks.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(stacks)
}

pub fn has_compose_candidate(project: &Path, candidates: &[PathBuf]) -> bool {
    candidates
        .iter()
        .any(|candidate| project.join(candidate).is_file())
}

pub fn group_by_name(stacks: &[DiscoveredStack]) -> BTreeMap<&str, Vec<&DiscoveredStack>> {
    let mut grouped = BTreeMap::new();
    for stack in stacks {
        grouped
            .entry(stack.name.as_str())
            .or_insert_with(Vec::new)
            .push(stack);
    }
    grouped
}

#[derive(Debug)]
pub enum DiscoveryError {
    NonUtf8Name(PathBuf),
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUtf8Name(path) => write!(
                formatter,
                "discovered project '{}' has a name that is not valid UTF-8 and cannot be targeted by name",
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

impl std::error::Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::NonUtf8Name(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::TempDir;

    use super::discover_stacks;
    use crate::config::{CONFIG_SCHEMA_VERSION, DiscoveryConfig, DstackConfig};

    fn config(paths: Vec<PathBuf>) -> DstackConfig {
        DstackConfig {
            version: CONFIG_SCHEMA_VERSION,
            discovery: DiscoveryConfig {
                paths,
                compose_files: vec![PathBuf::from("compose.yaml")],
            },
        }
    }

    fn project(root: &std::path::Path, name: &str, compose: bool) -> PathBuf {
        let path = root.join(name);
        fs::create_dir_all(&path).unwrap();
        if compose {
            fs::write(path.join("compose.yaml"), "services: {}\n").unwrap();
        }
        path
    }

    #[test]
    fn discovers_direct_children_across_roots_in_stable_order() {
        let temp = TempDir::new().unwrap();
        let first = temp.path().join("first");
        let second = temp.path().join("second with space");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        project(&first, "zeta", true);
        project(&second, "alpha λ", true);
        project(&first, "ignored", false);

        let stacks =
            discover_stacks(&config(vec![second.clone(), first.clone()]), temp.path()).unwrap();
        assert_eq!(
            stacks
                .iter()
                .map(|stack| stack.name.as_str())
                .collect::<Vec<_>>(),
            ["alpha λ", "zeta"]
        );
        assert!(stacks.iter().all(|stack| stack.path.is_absolute()));
    }

    #[test]
    fn missing_roots_and_nested_grandchildren_are_ignored() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(&root).unwrap();
        let wrapper = project(&root, "wrapper", false);
        project(&wrapper, "nested", true);

        let stacks = discover_stacks(
            &config(vec![temp.path().join("missing"), root]),
            temp.path(),
        )
        .unwrap();
        assert!(stacks.is_empty());
    }

    #[test]
    fn unreadable_or_non_directory_root_is_reported_instead_of_silently_swallowed() {
        let temp = TempDir::new().unwrap();
        let not_a_directory = temp.path().join("configured-root");
        fs::write(&not_a_directory, "not a directory").unwrap();

        let error = discover_stacks(&config(vec![not_a_directory]), temp.path())
            .expect_err("a configured root that cannot be read as a directory should fail");
        assert!(error.to_string().contains("read discovery root"));
    }

    #[test]
    fn overlapping_roots_deduplicate_the_same_named_physical_project() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(&root).unwrap();
        project(&root, "api", true);

        let stacks = discover_stacks(&config(vec![root.clone(), root]), temp.path()).unwrap();
        assert_eq!(stacks.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn direct_child_symlink_is_followed_once_without_recursive_traversal() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        let target = temp.path().join("target");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("compose.yaml"), "services: {}\n").unwrap();
        symlink(&target, root.join("linked")).unwrap();

        let stacks = discover_stacks(&config(vec![root]), temp.path()).unwrap();
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].name, "linked");
        assert_eq!(stacks[0].path, fs::canonicalize(target).unwrap());
    }
}
