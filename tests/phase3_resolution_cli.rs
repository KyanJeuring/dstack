use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use dstack_compose::config::{CONFIG_SCHEMA_VERSION, DiscoveryConfig, DstackConfig};
use tempfile::TempDir;

fn run_dstack(root: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dstack"))
        .args(args)
        .current_dir(cwd)
        .env("DSTACK_CONFIG_DIR", root)
        .output()
        .expect("the dstack test binary should run")
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).expect("DStack text output should be UTF-8")
}

fn write_config(root: &Path, discovery_roots: Vec<PathBuf>, candidates: &[&str]) {
    fs::create_dir_all(root).unwrap();
    let config = DstackConfig {
        version: CONFIG_SCHEMA_VERSION,
        discovery: DiscoveryConfig {
            paths: discovery_roots,
            compose_files: candidates.iter().map(PathBuf::from).collect(),
        },
    };
    fs::write(
        root.join("dstack.conf"),
        toml::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
}

fn project(parent: &Path, name: &str, files: &[&str]) -> PathBuf {
    let path = parent.join(name);
    fs::create_dir_all(&path).unwrap();
    for file in files {
        fs::write(path.join(file), "services: {}\n").unwrap();
    }
    fs::canonicalize(path).unwrap()
}

#[test]
fn help_and_version_remain_side_effect_free_but_config_path_bootstraps() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state with space λ");

    for args in [&["--help"][..], &["--version"][..]] {
        let output = run_dstack(&state, temp.path(), args);
        assert!(output.status.success());
        assert!(!state.exists());
    }

    let path = run_dstack(&state, temp.path(), &["config", "path"]);
    assert!(path.status.success(), "{}", text(path.stderr));
    assert_eq!(
        text(path.stdout),
        format!("{}\n", state.join("dstack.conf").display())
    );
    assert!(state.join("dstack.conf").is_file());
    assert!(state.join("registry.toml").is_file());
    let registry = fs::read_to_string(state.join("registry.toml")).unwrap();
    assert!(registry.contains("version = 1"));
    assert!(registry.contains("stacks = []"));

    let show = run_dstack(&state, temp.path(), &["config", "show"]);
    assert!(show.status.success());
    let stdout = text(show.stdout);
    assert!(stdout.contains("Version: 1"));
    assert!(stdout.contains("~/projects"));
    assert!(stdout.contains("docker-compose.yml"));
}

#[test]
fn malformed_or_future_config_is_never_replaced() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    fs::create_dir_all(&state).unwrap();

    for contents in [
        "not = [valid",
        "version = 999\n[discovery]\npaths=[]\ncompose_files=['compose.yaml']\n",
        "version = 1\n[discovry]\npaths=[]\ncompose_files=['compose.yaml']\n",
    ] {
        fs::write(state.join("dstack.conf"), contents).unwrap();
        let before = fs::read(state.join("dstack.conf")).unwrap();
        let output = run_dstack(&state, temp.path(), &["config", "show"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert_eq!(fs::read(state.join("dstack.conf")).unwrap(), before);
    }
}

#[test]
fn effective_list_combines_sources_shadows_registered_names_and_marks_ambiguity() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let first = temp.path().join("first root");
    let second = temp.path().join("second root");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    let registered_project = project(temp.path(), "registered project", &["compose.yaml"]);
    project(&first, "alpha", &["compose.yaml"]);
    project(&first, "same", &["compose.yaml"]);
    project(&second, "same", &["compose.yaml"]);
    project(&second, "shadowed", &["compose.yaml"]);
    project(&second, "ignored", &[]);
    write_config(
        &state,
        vec![second.clone(), temp.path().join("missing"), first.clone()],
        &["compose.yaml"],
    );

    let add = run_dstack(
        &state,
        temp.path(),
        &[
            "registry",
            "add",
            "shadowed",
            registered_project.to_str().unwrap(),
        ],
    );
    assert!(add.status.success(), "{}", text(add.stderr));

    let output = run_dstack(&state, temp.path(), &["list"]);
    assert!(output.status.success(), "{}", text(output.stderr));
    let stdout = text(output.stdout);
    assert!(stdout.starts_with("NAME\tSOURCE\tPATH\n"));
    assert!(stdout.contains("alpha\tdiscovered\t"));
    assert_eq!(stdout.matches("same\tdiscovered (ambiguous)\t").count(), 2);
    assert_eq!(stdout.matches("shadowed\t").count(), 1);
    assert!(stdout.contains("shadowed\tregistered\t"));
    assert!(!stdout.contains("ignored\t"));

    let ambiguous = run_dstack(&state, temp.path(), &["-s", "same", "context", "show"]);
    assert_eq!(ambiguous.status.code(), Some(1));
    assert!(text(ambiguous.stderr).contains("ambiguous"));
}

#[test]
fn explicit_registered_and_discovered_contexts_resolve_without_positional_guessing() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let root = temp.path().join("projects");
    fs::create_dir_all(&root).unwrap();
    let registered = project(temp.path(), "registered", &["compose.prod.yaml"]);
    let discovered = project(&root, "discovered", &["compose.yaml"]);
    write_config(&state, vec![root], &["compose.yaml"]);
    assert!(
        run_dstack(
            &state,
            temp.path(),
            &[
                "registry",
                "add",
                "prod",
                registered.to_str().unwrap(),
                "--file",
                "compose.prod.yaml",
            ],
        )
        .status
        .success()
    );

    let registered_context = run_dstack(&state, &discovered, &["context", "show", "-s", "prod"]);
    assert!(registered_context.status.success());
    let stdout = text(registered_context.stdout);
    assert!(stdout.contains("Stack: prod\n"));
    assert!(stdout.contains("Source: registered\n"));
    assert!(stdout.contains(&registered.join("compose.prod.yaml").display().to_string()));
    assert!(!stdout.contains(&discovered.display().to_string()));

    let discovered_context = run_dstack(
        &state,
        temp.path(),
        &["--stack", "discovered", "context", "show"],
    );
    assert!(discovered_context.status.success());
    let stdout = text(discovered_context.stdout);
    assert!(stdout.contains("Source: discovered\n"));
    assert!(stdout.contains(&discovered.display().to_string()));

    let positional = run_dstack(&state, temp.path(), &["discovered", "context", "show"]);
    assert_eq!(positional.status.code(), Some(2));
}

#[test]
fn cwd_resolution_is_exact_and_never_searches_parents() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let project = project(temp.path(), "cwd project", &["compose.yaml"]);
    let child = project.join("child");
    fs::create_dir_all(&child).unwrap();
    write_config(&state, Vec::new(), &["compose.yaml"]);

    let cwd = run_dstack(&state, &project, &["context", "show"]);
    assert!(cwd.status.success(), "{}", text(cwd.stderr));
    let stdout = text(cwd.stdout);
    assert!(stdout.contains("Source: cwd\n"));
    assert!(stdout.contains(&project.join("compose.yaml").display().to_string()));

    let child_result = run_dstack(&state, &child, &["context", "show"]);
    assert_eq!(child_result.status.code(), Some(1));
    let stderr = text(child_result.stderr);
    assert!(stderr.contains("exact current directory"));
    assert!(stderr.contains("--stack <name>"));
    assert!(stderr.contains("dstack list"));
}

#[test]
fn unknown_explicit_stack_fails_even_when_cwd_is_a_valid_project() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let cwd = project(temp.path(), "cwd", &["compose.yaml"]);
    write_config(&state, Vec::new(), &["compose.yaml"]);

    let output = run_dstack(&state, &cwd, &["context", "show", "-s", "missing"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = text(output.stderr);
    assert!(stderr.contains("stack 'missing' was not found"));
    assert!(!stderr.contains("Source: cwd"));
}

#[test]
fn explicit_files_replace_profile_defaults_and_preserve_hostile_path_data() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let project = project(
        temp.path(),
        "project",
        &[
            "compose.default.yaml",
            "compose explicit one.yaml",
            "-compose-two.yaml",
        ],
    );
    write_config(&state, Vec::new(), &["compose.default.yaml"]);
    assert!(
        run_dstack(
            &state,
            temp.path(),
            &[
                "registry",
                "add",
                "profile",
                project.to_str().unwrap(),
                "--file",
                "compose.default.yaml",
            ],
        )
        .status
        .success()
    );

    let context = run_dstack(
        &state,
        temp.path(),
        &[
            "--file",
            "compose explicit one.yaml",
            "context",
            "show",
            "-s",
            "profile",
            "--file=-compose-two.yaml",
        ],
    );
    assert!(context.status.success(), "{}", text(context.stderr));
    let stdout = text(context.stdout);
    let first = stdout.find("compose explicit one.yaml").unwrap();
    let second = stdout.find("-compose-two.yaml").unwrap();
    assert!(first < second);
    assert!(!stdout.contains("compose.default.yaml"));
}

#[test]
fn missing_explicit_or_registered_compose_files_fail_without_fallback() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let project = project(temp.path(), "project", &["compose.yaml"]);
    write_config(&state, Vec::new(), &["compose.yaml"]);
    assert!(
        run_dstack(
            &state,
            temp.path(),
            &[
                "registry",
                "add",
                "broken",
                project.to_str().unwrap(),
                "--file",
                "missing.yaml",
            ],
        )
        .status
        .success()
    );

    for extra in [Vec::<&str>::new(), vec!["--file", "also-missing.yaml"]] {
        let mut args = vec!["-s", "broken", "context", "show"];
        args.extend(extra);
        let output = run_dstack(&state, temp.path(), &args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(text(output.stderr).contains("does not exist"));
    }
}

#[test]
fn automatic_compose_selection_uses_configured_priority() {
    let temp = TempDir::new().unwrap();
    let state = temp.path().join("state");
    let project = project(
        temp.path(),
        "project",
        &["compose.yaml", "docker-compose.yml"],
    );
    write_config(&state, Vec::new(), &["docker-compose.yml", "compose.yaml"]);

    let output = run_dstack(&state, &project, &["context", "show"]);
    assert!(output.status.success());
    let stdout = text(output.stdout);
    assert!(stdout.contains(&project.join("docker-compose.yml").display().to_string()));
    assert!(!stdout.contains(&project.join("compose.yaml").display().to_string()));
}
