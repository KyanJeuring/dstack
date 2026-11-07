use std::{fs, path::PathBuf};

use tempfile::TempDir;

use super::{
    SESSION_SCHEMA_VERSION, SessionEnvironment, SessionError, SessionOrigin, SessionState,
    resolve_active_session,
};

fn sample_state(temp: &TempDir) -> SessionState {
    let project = temp.path().join("project with spaces λ");
    fs::create_dir_all(&project).unwrap();
    let first = project.join("compose.yaml");
    let second = project.join("compose prod.yaml");
    fs::write(&first, "services: {}\n").unwrap();
    fs::write(&second, "services: {}\n").unwrap();
    SessionState {
        version: SESSION_SCHEMA_VERSION,
        name: "jellyfin 東京".to_owned(),
        project: fs::canonicalize(project).unwrap(),
        files: vec![first, second],
        origin: SessionOrigin::Registered,
    }
}

#[test]
fn valid_state_round_trips_as_one_line_json_with_file_order() {
    let temp = TempDir::new().unwrap();
    let state = sample_state(&temp);
    let payload = state.serialize().unwrap();
    assert!(!payload.contains('\n'));
    assert!(payload.starts_with("{\"version\":1,"));
    assert_eq!(SessionState::parse(&payload).unwrap(), state);
}

#[test]
fn future_malformed_unknown_and_missing_fields_are_rejected() {
    for (payload, expected) in [
        (
            r#"{"version":2,"name":"x","project":"/tmp","files":["/tmp/a"],"origin":"registered"}"#,
            "unsupported schema version 2",
        ),
        ("not-json", "invalid"),
        (
            r#"{"version":1,"name":"x","project":"/tmp","files":["/tmp/a"],"origin":"registered","extra":true}"#,
            "unknown field",
        ),
        (
            r#"{"version":1,"name":"x","project":"/tmp","origin":"registered"}"#,
            "missing field",
        ),
    ] {
        let error = SessionState::parse(payload).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn invalid_path_and_file_representations_are_rejected() {
    let temp = TempDir::new().unwrap();
    let mut state = sample_state(&temp);
    state.project = PathBuf::from("relative");
    assert!(
        state
            .serialize()
            .unwrap_err()
            .to_string()
            .contains("absolute")
    );

    let mut state = sample_state(&temp);
    state.files.clear();
    assert!(
        state
            .serialize()
            .unwrap_err()
            .to_string()
            .contains("at least one")
    );

    let mut state = sample_state(&temp);
    state.files.push(state.files[0].clone());
    assert!(
        state
            .serialize()
            .unwrap_err()
            .to_string()
            .contains("more than once")
    );
}

#[test]
fn stale_project_and_compose_file_are_distinct_errors() {
    let temp = TempDir::new().unwrap();
    let state = sample_state(&temp);
    fs::remove_file(&state.files[0]).unwrap();
    assert!(matches!(
        state.validate_usable(),
        Err(SessionError::StaleComposeFile(_))
    ));

    let temp = TempDir::new().unwrap();
    let state = sample_state(&temp);
    fs::remove_dir_all(&state.project).unwrap();
    assert!(matches!(
        state.validate_usable(),
        Err(SessionError::StaleProject(_))
    ));
}

#[cfg(unix)]
#[test]
fn non_utf8_session_paths_fail_without_lossy_conversion() {
    use std::os::unix::ffi::OsStringExt;

    let temp = TempDir::new().unwrap();
    let mut state = sample_state(&temp);
    state.files[0] = PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 0xff]));
    assert!(matches!(
        state.serialize(),
        Err(SessionError::NonUtf8Path { .. })
    ));
}

#[cfg(unix)]
#[test]
fn non_utf8_environment_payload_is_a_typed_session_error() {
    use std::os::unix::ffi::OsStringExt;

    let environment =
        SessionEnvironment::with_value(std::ffi::OsString::from_vec(vec![b'{', 0xff, b'}']));
    assert!(matches!(
        resolve_active_session(&environment),
        Err(SessionError::NonUtf8Environment)
    ));
}
