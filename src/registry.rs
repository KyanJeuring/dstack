use std::{
    collections::HashSet,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};

pub const REGISTRY_SCHEMA_VERSION: u32 = 1;

/// A persistent named stack profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackProfile {
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub files: Vec<PathBuf>,
}

impl StackProfile {
    /// Validates registration input and stores a stable physical project path.
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

/// Versioned on-disk registry document.
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

/// Filesystem-backed registry with atomic mutation and advisory locking.
#[derive(Debug, Clone)]
pub struct RegistryStore {
    registry_path: PathBuf,
    lock_path: PathBuf,
}

impl RegistryStore {
    pub fn new(registry_path: PathBuf, lock_path: PathBuf) -> Self {
        Self {
            registry_path,
            lock_path,
        }
    }

    pub fn path(&self) -> &Path {
        &self.registry_path
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    pub fn load(&self) -> Result<Registry, RegistryError> {
        match fs::read_to_string(&self.registry_path) {
            Ok(contents) => Registry::parse(&contents, &self.registry_path),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(Registry::default()),
            Err(source) => Err(RegistryError::Io {
                action: "read registry",
                path: self.registry_path.clone(),
                source,
            }),
        }
    }

    pub fn add(&self, profile: StackProfile) -> Result<(), RegistryError> {
        self.mutate(|registry| {
            registry.insert(profile)?;
            Ok(((), true))
        })
    }

    pub fn remove(&self, name: &str) -> Result<StackProfile, RegistryError> {
        self.mutate(|registry| {
            let removed = registry.remove(name)?;
            Ok((removed, true))
        })
    }

    pub(crate) fn mutate<T>(
        &self,
        operation: impl FnOnce(&mut Registry) -> Result<(T, bool), RegistryError>,
    ) -> Result<T, RegistryError> {
        let _lock = self.acquire_lock()?;
        let mut registry = self.load()?;
        let (result, changed) = operation(&mut registry)?;
        if changed {
            self.save(&registry)?;
        }
        Ok(result)
    }

    fn acquire_lock(&self) -> Result<File, RegistryError> {
        let parent = self.parent_directory()?;
        fs::create_dir_all(parent).map_err(|source| RegistryError::Io {
            action: "create registry directory",
            path: parent.to_path_buf(),
            source,
        })?;

        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.lock_path)
            .map_err(|source| RegistryError::Io {
                action: "open registry lock",
                path: self.lock_path.clone(),
                source,
            })?;

        match lock.try_lock() {
            Ok(()) => Ok(lock),
            Err(std::fs::TryLockError::WouldBlock) => {
                Err(RegistryError::LockBusy(self.lock_path.clone()))
            }
            Err(std::fs::TryLockError::Error(source)) => Err(RegistryError::Io {
                action: "lock registry",
                path: self.lock_path.clone(),
                source,
            }),
        }
    }

    fn save(&self, registry: &Registry) -> Result<(), RegistryError> {
        let contents = registry.serialize()?;
        let parent = self.parent_directory()?;
        fs::create_dir_all(parent).map_err(|source| RegistryError::Io {
            action: "create registry directory",
            path: parent.to_path_buf(),
            source,
        })?;

        let mut file =
            AtomicWriteFile::open(&self.registry_path).map_err(|source| RegistryError::Io {
                action: "create registry staging file",
                path: self.registry_path.clone(),
                source,
            })?;
        file.write_all(contents.as_bytes())
            .map_err(|source| RegistryError::Io {
                action: "write registry staging file",
                path: self.registry_path.clone(),
                source,
            })?;
        file.commit().map_err(|source| RegistryError::Io {
            action: "replace registry atomically",
            path: self.registry_path.clone(),
            source,
        })
    }

    fn parent_directory(&self) -> Result<&Path, RegistryError> {
        self.registry_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| RegistryError::MissingParent(self.registry_path.clone()))
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

#[derive(Debug)]
pub enum RegistryError {
    InvalidName(String),
    InvalidProjectDirectory(PathBuf),
    InvalidComposeFile(String),
    DuplicateComposeFile(PathBuf),
    DuplicateName(String),
    UnknownProfile(String),
    UnsupportedSchemaVersion {
        found: u32,
        supported: u32,
    },
    MalformedRegistry {
        path: PathBuf,
        source: toml::de::Error,
    },
    Serialize(toml::ser::Error),
    InvalidStoredProfile {
        name: String,
        reason: String,
    },
    NonUtf8Path {
        label: &'static str,
        path: PathBuf,
    },
    InvalidPathText {
        label: &'static str,
        path: PathBuf,
    },
    LockBusy(PathBuf),
    MissingParent(PathBuf),
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(reason) => write!(formatter, "invalid stack profile name: {reason}"),
            Self::InvalidProjectDirectory(path) => write!(
                formatter,
                "project path '{}' is not an existing directory",
                path.display()
            ),
            Self::InvalidComposeFile(reason) => {
                write!(formatter, "invalid Compose-file value: {reason}")
            }
            Self::DuplicateComposeFile(path) => write!(
                formatter,
                "Compose-file value '{}' was provided more than once",
                path.display()
            ),
            Self::DuplicateName(name) => {
                write!(formatter, "stack profile '{name}' is already registered")
            }
            Self::UnknownProfile(name) => {
                write!(formatter, "stack profile '{name}' is not registered")
            }
            Self::UnsupportedSchemaVersion { found, supported } => write!(
                formatter,
                "registry schema version {found} is not supported (this DStack supports version {supported})"
            ),
            Self::MalformedRegistry { path, source } => write!(
                formatter,
                "could not parse registry '{}': {source}",
                path.display()
            ),
            Self::Serialize(source) => write!(formatter, "could not serialize registry: {source}"),
            Self::InvalidStoredProfile { name, reason } => {
                write!(formatter, "invalid stored profile '{name}': {reason}")
            }
            Self::NonUtf8Path { label, path } => write!(
                formatter,
                "{label} '{}' cannot be represented safely in the TOML registry because it is not valid UTF-8",
                path.display()
            ),
            Self::InvalidPathText { label, path } => write!(
                formatter,
                "{label} '{}' contains unsupported control characters",
                path.display()
            ),
            Self::LockBusy(path) => write!(
                formatter,
                "registry is locked by another DStack process ('{}')",
                path.display()
            ),
            Self::MissingParent(path) => write!(
                formatter,
                "registry path '{}' has no usable parent directory",
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

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MalformedRegistry { source, .. } => Some(source),
            Self::Serialize(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, OpenOptions},
        path::{Path, PathBuf},
    };

    use tempfile::TempDir;

    use super::{REGISTRY_SCHEMA_VERSION, Registry, RegistryError, RegistryStore, StackProfile};

    fn store(temp: &TempDir) -> RegistryStore {
        RegistryStore::new(
            temp.path().join("state/registry.toml"),
            temp.path().join("state/registry.lock"),
        )
    }

    fn project(temp: &TempDir, name: &str) -> PathBuf {
        let path = temp.path().join(name);
        fs::create_dir_all(&path).expect("project fixture should be created");
        path
    }

    fn profile(temp: &TempDir, name: &str, files: &[&str]) -> StackProfile {
        StackProfile::for_registration(
            name.to_owned(),
            project(temp, &format!("project-{name}")),
            files.iter().map(PathBuf::from).collect(),
        )
        .expect("profile fixture should be valid")
    }

    #[test]
    fn missing_registry_loads_as_empty_without_creating_a_file() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let store = store(&temp);

        assert_eq!(
            store.load().expect("missing registry should load"),
            Registry::default()
        );
        assert!(!store.path().exists());
    }

    #[test]
    fn first_add_creates_parent_and_versioned_registry() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let store = store(&temp);
        store
            .add(profile(&temp, "alpha", &[]))
            .expect("profile should be added");

        let text = fs::read_to_string(store.path()).expect("registry should be readable");
        assert!(text.contains(&format!("version = {REGISTRY_SCHEMA_VERSION}")));
        assert_eq!(store.load().expect("registry should load").stacks.len(), 1);
    }

    #[test]
    fn profile_round_trip_preserves_files_spaces_unicode_and_equals() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let project_path = project(&temp, "project space=λ");
        let expected = StackProfile::for_registration(
            "logs = λ".to_owned(),
            project_path,
            vec![
                PathBuf::from("compose prod.yaml"),
                PathBuf::from("config=dev.yaml"),
                PathBuf::from("配置.yaml"),
            ],
        )
        .expect("profile should be valid");
        let mut registry = Registry::default();
        registry
            .insert(expected.clone())
            .expect("profile should be inserted");

        let text = registry.serialize().expect("registry should serialize");
        let actual =
            Registry::parse(&text, Path::new("registry.toml")).expect("registry should parse");
        assert_eq!(actual.stacks, vec![expected]);
    }

    #[test]
    fn punctuation_and_command_like_names_are_exact_data() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let mut registry = Registry::default();
        let shared_path = project(&temp, "shared-project");
        for name in ["registry", "logs", "prod.*[1]=blue"] {
            registry
                .insert(
                    StackProfile::for_registration(
                        name.to_owned(),
                        shared_path.clone(),
                        Vec::new(),
                    )
                    .expect("punctuation-bearing name should be valid"),
                )
                .expect("punctuation-bearing name should be valid");
        }

        assert!(registry.get("prod.*[1]=blue").is_some());
        assert!(registry.get("prod-anything").is_none());
    }

    #[test]
    fn empty_whitespace_and_control_character_names_are_rejected() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let path = project(&temp, "project");
        for name in ["", "   ", "line\nbreak", "tab\tname"] {
            assert!(matches!(
                StackProfile::for_registration(name.to_owned(), path.clone(), Vec::new()),
                Err(RegistryError::InvalidName(_))
            ));
        }
    }

    #[test]
    fn multiple_names_can_share_one_project_path() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let path = project(&temp, "shared");
        let store = store(&temp);
        for (name, file) in [("dev", "compose.dev.yaml"), ("prod", "compose.prod.yaml")] {
            store
                .add(
                    StackProfile::for_registration(
                        name.to_owned(),
                        path.clone(),
                        vec![PathBuf::from(file)],
                    )
                    .expect("profile should be valid"),
                )
                .expect("shared-path profile should be added");
        }

        let registry = store.load().expect("registry should load");
        assert_eq!(registry.stacks.len(), 2);
        assert_eq!(registry.stacks[0].path, registry.stacks[1].path);
    }

    #[test]
    fn duplicate_name_is_rejected_without_replacing_original() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let store = store(&temp);
        let original = profile(&temp, "same", &["compose.yaml"]);
        store.add(original.clone()).expect("first add should work");

        let error = store
            .add(profile(&temp, "same", &["other.yaml"]))
            .expect_err("duplicate name should fail");
        assert!(matches!(error, RegistryError::DuplicateName(name) if name == "same"));
        assert_eq!(
            store.load().expect("registry should remain valid").stacks,
            vec![original]
        );
    }

    #[test]
    fn duplicate_compose_file_is_rejected() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let error = StackProfile::for_registration(
            "duplicate-files".to_owned(),
            project(&temp, "project"),
            vec![PathBuf::from("compose.yaml"), PathBuf::from("compose.yaml")],
        )
        .expect_err("duplicate file should fail");
        assert!(matches!(error, RegistryError::DuplicateComposeFile(_)));
    }

    #[test]
    fn names_are_serialized_in_stable_order() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let store = store(&temp);
        for name in ["zeta", "alpha", "middle"] {
            store
                .add(profile(&temp, name, &[]))
                .expect("profile should be added");
        }

        let names: Vec<_> = store
            .load()
            .expect("registry should load")
            .stacks
            .into_iter()
            .map(|profile| profile.name)
            .collect();
        assert_eq!(names, ["alpha", "middle", "zeta"]);
    }

    #[test]
    fn remove_existing_profile_does_not_affect_shared_path_peer() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let path = project(&temp, "shared");
        let store = store(&temp);
        for name in ["dev", "prod"] {
            store
                .add(
                    StackProfile::for_registration(name.to_owned(), path.clone(), Vec::new())
                        .expect("profile should be valid"),
                )
                .expect("profile should be added");
        }

        store.remove("dev").expect("profile should be removed");
        let registry = store.load().expect("registry should load");
        assert!(registry.get("dev").is_none());
        assert_eq!(
            registry.get("prod").expect("peer should remain").path,
            fs::canonicalize(path).unwrap()
        );
    }

    #[test]
    fn remove_unknown_profile_fails_without_rewriting_registry() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let store = store(&temp);
        store.add(profile(&temp, "known", &[])).unwrap();
        let before = fs::read(store.path()).unwrap();

        let error = store
            .remove("unknown")
            .expect_err("unknown remove should fail");
        assert!(matches!(error, RegistryError::UnknownProfile(name) if name == "unknown"));
        assert_eq!(fs::read(store.path()).unwrap(), before);
    }

    #[test]
    fn malformed_toml_is_rejected_and_not_replaced_by_mutation() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let store = store(&temp);
        fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        fs::write(store.path(), b"not = [valid").unwrap();
        let before = fs::read(store.path()).unwrap();

        assert!(matches!(
            store.load().expect_err("malformed registry should fail"),
            RegistryError::MalformedRegistry { .. }
        ));
        assert!(store.add(profile(&temp, "new", &[])).is_err());
        assert_eq!(fs::read(store.path()).unwrap(), before);
    }

    #[test]
    fn failed_serialization_does_not_destroy_previous_valid_registry() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let store = store(&temp);
        store.add(profile(&temp, "original", &[])).unwrap();
        let before = fs::read(store.path()).unwrap();

        let error = store
            .mutate(|registry| {
                registry.version = REGISTRY_SCHEMA_VERSION + 1;
                Ok(((), true))
            })
            .expect_err("invalid staged document should not commit");

        assert!(matches!(
            error,
            RegistryError::UnsupportedSchemaVersion { .. }
        ));
        assert_eq!(fs::read(store.path()).unwrap(), before);
        assert!(store.load().unwrap().get("original").is_some());
    }

    #[test]
    fn unsupported_future_schema_is_rejected() {
        let error = Registry::parse("version = 999\nstacks = []\n", Path::new("registry.toml"))
            .expect_err("future schema should fail");
        assert!(matches!(
            error,
            RegistryError::UnsupportedSchemaVersion {
                found: 999,
                supported: REGISTRY_SCHEMA_VERSION
            }
        ));
    }

    #[test]
    fn unknown_schema_fields_are_rejected_instead_of_ignored() {
        let error = Registry::parse(
            "version = 1\nfuture_setting = true\nstacks = []\n",
            Path::new("registry.toml"),
        )
        .expect_err("unknown fields should fail closed");
        assert!(matches!(error, RegistryError::MalformedRegistry { .. }));
    }

    #[test]
    fn windows_style_paths_round_trip_as_data() {
        let text = r#"version = 1

[[stacks]]
name = "windows"
path = 'C:\Users\Kyan\Compose Project'
files = ['compose.prod.yaml']
"#;
        let registry = Registry::parse(text, Path::new("registry.toml"));

        #[cfg(windows)]
        assert!(registry.is_ok());
        #[cfg(not(windows))]
        assert!(matches!(
            registry.expect_err("Windows path is not absolute on this host"),
            RegistryError::InvalidStoredProfile { .. }
        ));
    }

    #[test]
    fn held_lock_prevents_mutation_and_preserves_registry() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let store = store(&temp);
        store.add(profile(&temp, "original", &[])).unwrap();
        let before = fs::read(store.path()).unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(store.lock_path())
            .unwrap();
        lock.try_lock().expect("test should acquire lock");

        let error = store
            .add(profile(&temp, "blocked", &[]))
            .expect_err("held lock should block mutation");
        assert!(matches!(error, RegistryError::LockBusy(_)));
        assert_eq!(fs::read(store.path()).unwrap(), before);
        drop(lock);

        store
            .add(profile(&temp, "after", &[]))
            .expect("released lock should allow mutation");
    }

    #[test]
    fn readers_observe_complete_registry_while_mutation_lock_is_held() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let store = store(&temp);
        store.add(profile(&temp, "visible", &[])).unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(store.lock_path())
            .unwrap();
        lock.try_lock().expect("test should acquire lock");

        let registry = store
            .load()
            .expect("atomic registry remains readable during mutation lock");
        assert!(registry.get("visible").is_some());
    }
}
