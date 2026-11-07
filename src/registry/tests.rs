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
    let temp = TempDir::new().unwrap();
    let store = store(&temp);
    assert_eq!(store.load().unwrap(), Registry::default());
    assert!(!store.path().exists());
}

#[test]
fn first_add_creates_parent_and_versioned_registry() {
    let temp = TempDir::new().unwrap();
    let store = store(&temp);
    store.add(profile(&temp, "alpha", &[])).unwrap();
    let text = fs::read_to_string(store.path()).unwrap();
    assert!(text.contains(&format!("version = {REGISTRY_SCHEMA_VERSION}")));
    assert_eq!(store.load().unwrap().stacks.len(), 1);
}

#[test]
fn profile_round_trip_preserves_files_spaces_unicode_and_equals() {
    let temp = TempDir::new().unwrap();
    let expected = StackProfile::for_registration(
        "logs = λ".to_owned(),
        project(&temp, "project space=λ"),
        vec![
            PathBuf::from("compose prod.yaml"),
            PathBuf::from("config=dev.yaml"),
            PathBuf::from("配置.yaml"),
        ],
    )
    .unwrap();
    let mut registry = Registry::default();
    registry.insert(expected.clone()).unwrap();
    let text = registry.serialize().unwrap();
    let actual = Registry::parse(&text, Path::new("registry.toml")).unwrap();
    assert_eq!(actual.stacks, vec![expected]);
}

#[test]
fn punctuation_and_command_like_names_are_exact_data() {
    let temp = TempDir::new().unwrap();
    let mut registry = Registry::default();
    let shared_path = project(&temp, "shared-project");
    for name in ["registry", "logs", "prod.*[1]=blue"] {
        registry
            .insert(
                StackProfile::for_registration(name.to_owned(), shared_path.clone(), Vec::new())
                    .unwrap(),
            )
            .unwrap();
    }
    assert!(registry.get("prod.*[1]=blue").is_some());
    assert!(registry.get("prod-anything").is_none());
}

#[test]
fn empty_whitespace_and_control_character_names_are_rejected() {
    let temp = TempDir::new().unwrap();
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
    let temp = TempDir::new().unwrap();
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
                .unwrap(),
            )
            .unwrap();
    }
    let registry = store.load().unwrap();
    assert_eq!(registry.stacks.len(), 2);
    assert_eq!(registry.stacks[0].path, registry.stacks[1].path);
}

#[test]
fn duplicate_name_is_rejected_without_replacing_original() {
    let temp = TempDir::new().unwrap();
    let store = store(&temp);
    let original = profile(&temp, "same", &["compose.yaml"]);
    store.add(original.clone()).unwrap();
    let error = store
        .add(profile(&temp, "same", &["other.yaml"]))
        .unwrap_err();
    assert!(matches!(error, RegistryError::DuplicateName(name) if name == "same"));
    assert_eq!(store.load().unwrap().stacks, vec![original]);
}

#[test]
fn duplicate_compose_file_is_rejected() {
    let temp = TempDir::new().unwrap();
    let error = StackProfile::for_registration(
        "duplicate-files".to_owned(),
        project(&temp, "project"),
        vec![PathBuf::from("compose.yaml"), PathBuf::from("compose.yaml")],
    )
    .unwrap_err();
    assert!(matches!(error, RegistryError::DuplicateComposeFile(_)));
}

#[test]
fn names_are_serialized_in_stable_order() {
    let temp = TempDir::new().unwrap();
    let store = store(&temp);
    for name in ["zeta", "alpha", "middle"] {
        store.add(profile(&temp, name, &[])).unwrap();
    }
    let names: Vec<_> = store
        .load()
        .unwrap()
        .stacks
        .into_iter()
        .map(|profile| profile.name)
        .collect();
    assert_eq!(names, ["alpha", "middle", "zeta"]);
}

#[test]
fn remove_existing_profile_does_not_affect_shared_path_peer() {
    let temp = TempDir::new().unwrap();
    let path = project(&temp, "shared");
    let store = store(&temp);
    for name in ["dev", "prod"] {
        store
            .add(StackProfile::for_registration(name.to_owned(), path.clone(), Vec::new()).unwrap())
            .unwrap();
    }
    store.remove("dev").unwrap();
    let registry = store.load().unwrap();
    assert!(registry.get("dev").is_none());
    assert_eq!(
        registry.get("prod").unwrap().path,
        fs::canonicalize(path).unwrap()
    );
}

#[test]
fn remove_unknown_profile_fails_without_rewriting_registry() {
    let temp = TempDir::new().unwrap();
    let store = store(&temp);
    store.add(profile(&temp, "known", &[])).unwrap();
    let before = fs::read(store.path()).unwrap();
    let error = store.remove("unknown").unwrap_err();
    assert!(matches!(error, RegistryError::UnknownProfile(name) if name == "unknown"));
    assert_eq!(fs::read(store.path()).unwrap(), before);
}

#[test]
fn malformed_toml_is_rejected_and_not_replaced_by_mutation() {
    let temp = TempDir::new().unwrap();
    let store = store(&temp);
    fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    fs::write(store.path(), b"not = [valid").unwrap();
    let before = fs::read(store.path()).unwrap();
    assert!(matches!(
        store.load().unwrap_err(),
        RegistryError::MalformedRegistry { .. }
    ));
    assert!(store.add(profile(&temp, "new", &[])).is_err());
    assert_eq!(fs::read(store.path()).unwrap(), before);
}

#[test]
fn failed_serialization_does_not_destroy_previous_valid_registry() {
    let temp = TempDir::new().unwrap();
    let store = store(&temp);
    store.add(profile(&temp, "original", &[])).unwrap();
    let before = fs::read(store.path()).unwrap();
    let error = store
        .mutate(|registry| {
            registry.version = REGISTRY_SCHEMA_VERSION + 1;
            Ok(((), true))
        })
        .unwrap_err();
    assert!(matches!(
        error,
        RegistryError::UnsupportedSchemaVersion { .. }
    ));
    assert_eq!(fs::read(store.path()).unwrap(), before);
    assert!(store.load().unwrap().get("original").is_some());
}

#[test]
fn unsupported_future_schema_is_rejected() {
    let error =
        Registry::parse("version = 999\nstacks = []\n", Path::new("registry.toml")).unwrap_err();
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
    .unwrap_err();
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
        registry.unwrap_err(),
        RegistryError::InvalidStoredProfile { .. }
    ));
}

#[test]
fn held_lock_prevents_mutation_and_preserves_registry() {
    let temp = TempDir::new().unwrap();
    let store = store(&temp);
    store.add(profile(&temp, "original", &[])).unwrap();
    let before = fs::read(store.path()).unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(store.lock_path())
        .unwrap();
    lock.try_lock().unwrap();
    let error = store.add(profile(&temp, "blocked", &[])).unwrap_err();
    assert!(matches!(error, RegistryError::LockBusy(_)));
    assert_eq!(fs::read(store.path()).unwrap(), before);
    lock.unlock().unwrap();
    drop(lock);
    store.add(profile(&temp, "after", &[])).unwrap();
}

#[test]
fn registry_lock_can_be_reacquired_immediately_after_guard_drop() {
    let temp = TempDir::new().unwrap();
    let store = store(&temp);

    for _ in 0..100 {
        let lock = store.acquire_lock().unwrap();
        drop(lock);
    }
}

#[test]
fn readers_observe_complete_registry_while_mutation_lock_is_held() {
    let temp = TempDir::new().unwrap();
    let store = store(&temp);
    store.add(profile(&temp, "visible", &[])).unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(store.lock_path())
        .unwrap();
    lock.try_lock().unwrap();
    assert!(store.load().unwrap().get("visible").is_some());
}
