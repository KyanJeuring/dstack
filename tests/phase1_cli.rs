use std::process::{Command, Output};

fn run_dstack(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dstack"))
        .args(args)
        .output()
        .expect("the dstack test binary should run")
}

#[test]
fn help_succeeds_and_is_generated() {
    let output = run_dstack(&["--help"]);

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage: dstack"));
    assert!(output.stderr.is_empty());
}

#[test]
fn version_succeeds_and_matches_package_version() {
    let output = run_dstack(&["--version"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("version output should be UTF-8"),
        format!("dstack {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn unknown_argument_fails_nonzero() {
    let output = run_dstack(&["--not-a-dstack-option"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected argument"));
}

#[test]
fn unsupported_bare_invocation_returns_application_error() {
    let output = run_dstack(&[]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("error output should be UTF-8"),
        "dstack: no DStack v2 commands are available in Phase 1; use --help or --version\n"
    );
}
