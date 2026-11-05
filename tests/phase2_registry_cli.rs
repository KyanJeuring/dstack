use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;
use unicode_width::UnicodeWidthStr;

fn run_dstack(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dstack"))
        .args(args)
        .env("DSTACK_CONFIG_DIR", root)
        .output()
        .expect("the dstack test binary should run")
}

fn project(temp: &TempDir, name: &str) -> PathBuf {
    let path = temp.path().join(name);
    fs::create_dir_all(&path).expect("project fixture should be created");
    path
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).expect("DStack text output should be UTF-8")
}

#[test]
fn registry_list_on_missing_store_bootstraps_an_empty_registry() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");

    let output = run_dstack(&state, &["registry", "list"]);

    assert!(output.status.success());
    assert_eq!(text(output.stdout), "NAME  PATH  FILES\n");
    assert!(output.stderr.is_empty());
    assert!(state.join("registry.toml").is_file());
    assert!(state.join("dstack.conf").is_file());
}

#[test]
fn registry_add_show_list_and_remove_round_trip() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let project = project(&temp, "project space=λ");
    let project_arg = project.to_str().unwrap();

    let add = run_dstack(
        &state,
        &[
            "registry",
            "add",
            "logs = λ",
            project_arg,
            "--file",
            "compose prod.yaml",
            "--file=-compose.dev.yaml",
        ],
    );
    assert!(add.status.success(), "{}", text(add.stderr));
    assert_eq!(text(add.stdout), "Registered stack profile 'logs = λ'.\n");

    let show = run_dstack(&state, &["registry", "show", "logs = λ"]);
    assert!(show.status.success());
    let show_stdout = text(show.stdout);
    assert!(show_stdout.contains("Name: logs = λ\n"));
    assert!(show_stdout.contains("compose prod.yaml"));
    assert!(show_stdout.contains("-compose.dev.yaml"));
    assert!(show.stderr.is_empty());

    let list = run_dstack(&state, &["registry", "list"]);
    assert!(list.status.success());
    let list_stdout = text(list.stdout);
    let stored_path = fs::canonicalize(&project).unwrap().display().to_string();
    let header_path_padding = " ".repeat(UnicodeWidthStr::width(stored_path.as_str()) - 2);
    assert_eq!(
        list_stdout,
        format!(
            "NAME      PATH{header_path_padding}FILES\nlogs = λ  {stored_path}  compose prod.yaml, -compose.dev.yaml\n"
        )
    );
    assert!(!list_stdout.contains('\t'));

    let remove = run_dstack(&state, &["registry", "remove", "logs = λ"]);
    assert!(remove.status.success());
    assert_eq!(text(remove.stdout), "Removed stack profile 'logs = λ'.\n");
    assert!(
        project.is_dir(),
        "registry removal must not delete the project"
    );

    let show_missing = run_dstack(&state, &["registry", "show", "logs = λ"]);
    assert_eq!(show_missing.status.code(), Some(1));
    assert!(show_missing.stdout.is_empty());
    assert!(text(show_missing.stderr).contains("is not registered"));
}

#[test]
fn duplicate_names_fail_but_duplicate_project_paths_are_valid() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let project = project(&temp, "shared");
    let path = project.to_str().unwrap();

    for name in ["prod", "dev"] {
        let output = run_dstack(&state, &["registry", "add", name, path]);
        assert!(output.status.success(), "{}", text(output.stderr));
    }

    let duplicate = run_dstack(
        &state,
        &["registry", "add", "prod", path, "--file", "other.yaml"],
    );
    assert_eq!(duplicate.status.code(), Some(1));
    assert!(duplicate.stdout.is_empty());
    assert!(text(duplicate.stderr).contains("already registered"));

    let list = text(run_dstack(&state, &["registry", "list"]).stdout);
    let dev = list
        .lines()
        .position(|line| line.starts_with("dev "))
        .unwrap();
    let prod = list
        .lines()
        .position(|line| line.starts_with("prod "))
        .unwrap();
    assert!(dev < prod, "registry list should be sorted by name");
    assert!(!list.contains("other.yaml"));
}

#[test]
fn invalid_add_inputs_do_not_modify_bootstrapped_registry() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let missing = temp.path().join("missing");

    let invalid_path = run_dstack(
        &state,
        &["registry", "add", "missing", missing.to_str().unwrap()],
    );
    assert_eq!(invalid_path.status.code(), Some(1));
    assert!(text(invalid_path.stderr).contains("not an existing directory"));
    let empty_registry = fs::read(state.join("registry.toml")).unwrap();

    let project = project(&temp, "valid");
    let duplicate_file = run_dstack(
        &state,
        &[
            "registry",
            "add",
            "duplicate-file",
            project.to_str().unwrap(),
            "--file",
            "compose.yaml",
            "--file",
            "compose.yaml",
        ],
    );
    assert_eq!(duplicate_file.status.code(), Some(1));
    assert!(text(duplicate_file.stderr).contains("provided more than once"));
    assert_eq!(
        fs::read(state.join("registry.toml")).unwrap(),
        empty_registry
    );

    let empty_file = run_dstack(
        &state,
        &[
            "registry",
            "add",
            "empty-file",
            project.to_str().unwrap(),
            "--file",
            "",
        ],
    );
    assert_eq!(empty_file.status.code(), Some(2));
    assert!(text(empty_file.stderr).contains("a value is required"));
    assert_eq!(
        fs::read(state.join("registry.toml")).unwrap(),
        empty_registry
    );
}

#[test]
fn malformed_and_future_registry_documents_fail_without_replacement() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    fs::create_dir_all(&state).unwrap();
    let registry = state.join("registry.toml");

    for content in ["", "not = [valid", "version = 999\nstacks = []\n"] {
        fs::write(&registry, content).unwrap();
        let before = fs::read(&registry).unwrap();
        let output = run_dstack(&state, &["registry", "list"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert_eq!(fs::read(&registry).unwrap(), before);
    }
}

#[test]
fn unknown_remove_fails_without_changing_the_document() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let project = project(&temp, "known");
    let add = run_dstack(
        &state,
        &["registry", "add", "known", project.to_str().unwrap()],
    );
    assert!(add.status.success());
    let before = fs::read(state.join("registry.toml")).unwrap();

    let remove = run_dstack(&state, &["registry", "remove", "unknown"]);
    assert_eq!(remove.status.code(), Some(1));
    assert!(remove.stdout.is_empty());
    assert!(text(remove.stderr).contains("is not registered"));
    assert_eq!(fs::read(state.join("registry.toml")).unwrap(), before);
}

#[test]
fn migration_dry_run_then_write_then_repeat_is_safe() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let project = project(&temp, "legacy project");
    let legacy = state.join("legacy/registry");
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    let source = format!("legacy={}\n", project.display());
    fs::write(&legacy, &source).unwrap();

    let dry_run = run_dstack(&state, &["migrate", "registry", "--dry-run"]);
    assert!(dry_run.status.success(), "{}", text(dry_run.stderr));
    let dry_stdout = text(dry_run.stdout);
    assert!(dry_stdout.contains("Mode: dry-run"));
    assert!(dry_stdout.contains("Added: 1"));
    assert!(dry_stdout.contains("Written: no"));
    assert!(state.join("registry.toml").is_file());
    assert!(state.join("dstack.conf").is_file());

    let write = run_dstack(&state, &["migrate", "registry"]);
    assert!(write.status.success(), "{}", text(write.stderr));
    assert!(text(write.stdout).contains("Written: yes"));
    assert_eq!(fs::read_to_string(&legacy).unwrap(), source);

    let repeat = run_dstack(&state, &["migrate", "registry"]);
    assert!(repeat.status.success(), "{}", text(repeat.stderr));
    let repeat_stdout = text(repeat.stdout);
    assert!(repeat_stdout.contains("Already present: 1"));
    assert!(repeat_stdout.contains("Written: no"));
}

#[test]
fn migration_reports_invalid_and_conflicting_entries_nonzero_without_overwrite() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let current = project(&temp, "current");
    let legacy_project = project(&temp, "legacy");

    let add = run_dstack(
        &state,
        &[
            "registry",
            "add",
            "conflict",
            current.to_str().unwrap(),
            "--file",
            "compose.prod.yaml",
        ],
    );
    assert!(add.status.success());

    let legacy = state.join("legacy/registry");
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    fs::write(
        &legacy,
        format!(
            "conflict={}\ninvalid=line=with=equals\nmissing={}\n",
            legacy_project.display(),
            temp.path().join("absent").display()
        ),
    )
    .unwrap();

    let output = run_dstack(&state, &["migrate", "registry"]);
    assert_eq!(output.status.code(), Some(1));
    let stdout = text(output.stdout);
    let stderr = text(output.stderr);
    assert!(stdout.contains("Skipped: 2"));
    assert!(stdout.contains("Conflicts: 1"));
    assert!(stdout.contains("Written: no"));
    assert!(stderr.contains("skipped"));
    assert!(stderr.contains("conflict"));

    let show = text(run_dstack(&state, &["registry", "show", "conflict"]).stdout);
    assert!(show.contains("compose.prod.yaml"));
    assert!(show.contains(&fs::canonicalize(current).unwrap().display().to_string()));
}

#[test]
fn missing_legacy_source_fails_after_safe_application_bootstrap() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");

    let output = run_dstack(&state, &["migrate", "registry"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(text(output.stderr).contains("does not exist"));
    assert!(state.join("registry.toml").is_file());
    assert!(state.join("dstack.conf").is_file());
}
