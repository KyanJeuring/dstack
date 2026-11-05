use std::{
    collections::HashMap,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use crate::registry::{Registry, RegistryError, RegistryStore, StackProfile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationIssue {
    pub line: usize,
    pub name: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub dry_run: bool,
    pub discovered: usize,
    pub added: usize,
    pub already_present: usize,
    pub skipped: Vec<MigrationIssue>,
    pub conflicts: Vec<MigrationIssue>,
    pub wrote: bool,
}

impl MigrationReport {
    pub fn has_issues(&self) -> bool {
        !self.skipped.is_empty() || !self.conflicts.is_empty()
    }
}

#[derive(Debug, Clone)]
struct LegacyEntry {
    line: usize,
    name: String,
    path: PathBuf,
}

/// Explicitly previews or imports the legacy v1 line-oriented registry.
///
/// The source is never modified. Valid non-conflicting entries may be added
/// while malformed or ambiguous entries are reported separately.
pub fn migrate_registry(
    store: &RegistryStore,
    source: &Path,
    dry_run: bool,
) -> Result<MigrationReport, MigrationError> {
    let contents =
        fs::read_to_string(source).map_err(|source_error| MigrationError::ReadLegacy {
            path: source.to_path_buf(),
            source: source_error,
        })?;
    let parsed = parse_legacy(&contents);

    if dry_run {
        let mut registry = store.load()?;
        Ok(plan_migration(
            &mut registry,
            parsed,
            source,
            store.path(),
            true,
        ))
    } else {
        store
            .mutate(|registry| {
                let mut report = plan_migration(registry, parsed, source, store.path(), false);
                let changed = report.added > 0;
                report.wrote = changed;
                Ok((report, changed))
            })
            .map_err(MigrationError::Registry)
    }
}

#[derive(Debug)]
struct ParsedLegacy {
    discovered: usize,
    entries: Vec<LegacyEntry>,
    skipped: Vec<MigrationIssue>,
    conflicts: Vec<MigrationIssue>,
}

fn parse_legacy(contents: &str) -> ParsedLegacy {
    let mut discovered = 0;
    let mut entries = Vec::new();
    let mut skipped = Vec::new();

    for (index, line) in contents.lines().enumerate() {
        let line_number = index + 1;
        if line.is_empty() {
            continue;
        }
        discovered += 1;

        if line.bytes().filter(|byte| *byte == b'=').count() != 1 {
            skipped.push(MigrationIssue {
                line: line_number,
                name: line.split_once('=').map(|(name, _)| name.to_owned()),
                reason:
                    "legacy line is ambiguous because it does not contain exactly one '=' delimiter"
                        .to_owned(),
            });
            continue;
        }

        let (name, path) = line
            .split_once('=')
            .expect("exactly one delimiter was counted above");
        if name.is_empty() {
            skipped.push(MigrationIssue {
                line: line_number,
                name: None,
                reason: "legacy profile name is empty".to_owned(),
            });
            continue;
        }
        if path.is_empty() {
            skipped.push(MigrationIssue {
                line: line_number,
                name: Some(name.to_owned()),
                reason: "legacy project path is empty".to_owned(),
            });
            continue;
        }

        entries.push(LegacyEntry {
            line: line_number,
            name: name.to_owned(),
            path: PathBuf::from(path),
        });
    }

    let mut name_counts = HashMap::new();
    for entry in &entries {
        *name_counts.entry(entry.name.clone()).or_insert(0usize) += 1;
    }

    let mut conflicts = Vec::new();
    entries.retain(|entry| {
        if name_counts[&entry.name] > 1 {
            conflicts.push(MigrationIssue {
                line: entry.line,
                name: Some(entry.name.clone()),
                reason: "legacy registry contains this name more than once; no entry was guessed"
                    .to_owned(),
            });
            false
        } else {
            true
        }
    });

    ParsedLegacy {
        discovered,
        entries,
        skipped,
        conflicts,
    }
}

fn plan_migration(
    registry: &mut Registry,
    parsed: ParsedLegacy,
    source: &Path,
    destination: &Path,
    dry_run: bool,
) -> MigrationReport {
    let mut report = MigrationReport {
        source: source.to_path_buf(),
        destination: destination.to_path_buf(),
        dry_run,
        discovered: parsed.discovered,
        added: 0,
        already_present: 0,
        skipped: parsed.skipped,
        conflicts: parsed.conflicts,
        wrote: false,
    };

    for entry in parsed.entries {
        if !entry.path.is_absolute() {
            report.skipped.push(MigrationIssue {
                line: entry.line,
                name: Some(entry.name),
                reason: "legacy project path is relative and cannot be migrated without guessing its original working directory"
                    .to_owned(),
            });
            continue;
        }
        let profile =
            match StackProfile::for_registration(entry.name.clone(), entry.path, Vec::new()) {
                Ok(profile) => profile,
                Err(error) => {
                    report.skipped.push(MigrationIssue {
                        line: entry.line,
                        name: Some(entry.name),
                        reason: error.to_string(),
                    });
                    continue;
                }
            };

        match registry.get(&profile.name) {
            Some(existing) if existing == &profile => {
                report.already_present += 1;
            }
            Some(_) => {
                report.conflicts.push(MigrationIssue {
                    line: entry.line,
                    name: Some(profile.name),
                    reason: "a different v2 profile already uses this name".to_owned(),
                });
            }
            None => {
                report.added += 1;
                registry
                    .insert(profile)
                    .expect("validated migration profile should remain insertable");
            }
        }
    }

    report
}

#[derive(Debug)]
pub enum MigrationError {
    ReadLegacy { path: PathBuf, source: io::Error },
    Registry(RegistryError),
}

impl fmt::Display for MigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadLegacy { path, source } if source.kind() == io::ErrorKind::NotFound => {
                write!(
                    formatter,
                    "legacy v1 registry '{}' does not exist",
                    path.display()
                )
            }
            Self::ReadLegacy { path, source } => write!(
                formatter,
                "could not read legacy v1 registry '{}': {source}",
                path.display()
            ),
            Self::Registry(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for MigrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadLegacy { source, .. } => Some(source),
            Self::Registry(source) => Some(source),
        }
    }
}

impl From<RegistryError> for MigrationError {
    fn from(error: RegistryError) -> Self {
        Self::Registry(error)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::TempDir;

    use crate::registry::{RegistryStore, StackProfile};

    use super::migrate_registry;

    fn setup() -> (TempDir, RegistryStore, PathBuf) {
        let temp = TempDir::new().expect("temporary directory should be created");
        let store = RegistryStore::new(
            temp.path().join("v2/registry.toml"),
            temp.path().join("v2/registry.lock"),
        );
        let legacy = temp.path().join("v1/registry");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        (temp, store, legacy)
    }

    fn project(temp: &TempDir, name: &str) -> PathBuf {
        let path = temp.path().join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn dry_run_reports_entries_without_writing() {
        let (temp, store, legacy) = setup();
        let path = project(&temp, "project");
        fs::write(&legacy, format!("alpha={}\n", path.display())).unwrap();

        let report = migrate_registry(&store, &legacy, true).unwrap();
        assert_eq!(report.discovered, 1);
        assert_eq!(report.added, 1);
        assert!(!report.wrote);
        assert!(!store.path().exists());
    }

    #[test]
    fn migration_imports_multiple_names_in_sorted_order_and_keeps_source() {
        let (temp, store, legacy) = setup();
        let shared = project(&temp, "shared path");
        let original = format!("prod={}\ndev={}\n", shared.display(), shared.display());
        fs::write(&legacy, &original).unwrap();

        let report = migrate_registry(&store, &legacy, false).unwrap();
        assert_eq!(report.added, 2);
        assert!(report.wrote);
        assert!(!report.has_issues());
        let names: Vec<_> = store
            .load()
            .unwrap()
            .stacks
            .into_iter()
            .map(|profile| profile.name)
            .collect();
        assert_eq!(names, ["dev", "prod"]);
        assert_eq!(fs::read_to_string(&legacy).unwrap(), original);
    }

    #[test]
    fn malformed_and_ambiguous_lines_are_skipped_without_guessing() {
        let (temp, store, legacy) = setup();
        let valid = project(&temp, "valid");
        fs::write(
            &legacy,
            format!(
                "valid={}\nmissing-delimiter\nname=path=with=equals\n=empty-name\nempty-path=\n",
                valid.display()
            ),
        )
        .unwrap();

        let report = migrate_registry(&store, &legacy, false).unwrap();
        assert_eq!(report.discovered, 5);
        assert_eq!(report.added, 1);
        assert_eq!(report.skipped.len(), 4);
        assert!(store.load().unwrap().get("valid").is_some());
    }

    #[test]
    fn duplicate_legacy_names_are_all_conflicts() {
        let (temp, store, legacy) = setup();
        let first = project(&temp, "first");
        let second = project(&temp, "second");
        fs::write(
            &legacy,
            format!("same={}\nsame={}\n", first.display(), second.display()),
        )
        .unwrap();

        let report = migrate_registry(&store, &legacy, false).unwrap();
        assert_eq!(report.conflicts.len(), 2);
        assert_eq!(report.added, 0);
        assert!(!report.wrote);
    }

    #[test]
    fn existing_identical_profile_makes_repeat_migration_a_noop() {
        let (temp, store, legacy) = setup();
        let path = project(&temp, "same");
        fs::write(&legacy, format!("same={}\n", path.display())).unwrap();

        let first = migrate_registry(&store, &legacy, false).unwrap();
        let bytes = fs::read(store.path()).unwrap();
        let second = migrate_registry(&store, &legacy, false).unwrap();

        assert_eq!(first.added, 1);
        assert_eq!(second.added, 0);
        assert_eq!(second.already_present, 1);
        assert!(!second.wrote);
        assert_eq!(fs::read(store.path()).unwrap(), bytes);
    }

    #[test]
    fn different_existing_profile_is_a_conflict_and_is_not_replaced() {
        let (temp, store, legacy) = setup();
        let existing_path = project(&temp, "existing");
        let legacy_path = project(&temp, "legacy");
        let existing = StackProfile::for_registration(
            "same".to_owned(),
            existing_path,
            vec![PathBuf::from("compose.prod.yaml")],
        )
        .unwrap();
        store.add(existing.clone()).unwrap();
        fs::write(&legacy, format!("same={}\n", legacy_path.display())).unwrap();

        let report = migrate_registry(&store, &legacy, false).unwrap();
        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(report.added, 0);
        assert!(!report.wrote);
        assert_eq!(store.load().unwrap().get("same"), Some(&existing));
    }

    #[test]
    fn stale_legacy_path_is_reported_and_not_imported() {
        let (temp, store, legacy) = setup();
        fs::write(
            &legacy,
            format!("stale={}\n", temp.path().join("missing").display()),
        )
        .unwrap();

        let report = migrate_registry(&store, &legacy, false).unwrap();
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.added, 0);
        assert!(!store.path().exists());
    }

    #[test]
    fn missing_legacy_registry_fails_clearly() {
        let (_temp, store, legacy) = setup();
        fs::remove_file(&legacy).ok();

        let error =
            migrate_registry(&store, &legacy, false).expect_err("missing source should fail");
        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn empty_legacy_registry_is_a_successful_noop() {
        let (_temp, store, legacy) = setup();
        fs::write(&legacy, "\n\n").unwrap();

        let report = migrate_registry(&store, &legacy, false).unwrap();
        assert_eq!(report.discovered, 0);
        assert_eq!(report.added, 0);
        assert!(!report.wrote);
        assert!(!report.has_issues());
        assert!(!store.path().exists());
    }

    #[test]
    fn valid_entries_are_imported_while_invalid_rows_remain_reported() {
        let (temp, store, legacy) = setup();
        let valid = project(&temp, "valid");
        fs::write(
            &legacy,
            format!("valid={}\nambiguous=path=with=equals\n", valid.display()),
        )
        .unwrap();

        let report = migrate_registry(&store, &legacy, false).unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(report.skipped.len(), 1);
        assert!(report.wrote);
        assert!(report.has_issues());
        assert!(store.load().unwrap().get("valid").is_some());
    }

    #[test]
    fn relative_legacy_path_is_rejected_instead_of_resolved_from_migration_cwd() {
        let (_temp, store, legacy) = setup();
        fs::write(&legacy, "relative=some/project\n").unwrap();

        let report = migrate_registry(&store, &legacy, false).unwrap();
        assert_eq!(report.added, 0);
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].reason.contains("relative"));
        assert!(!report.wrote);
    }
}
