use std::{ffi::OsString, io, path::PathBuf};

use super::{inspect, list, remove};
use crate::{
    AppError, AppOutcome,
    output::Output,
    process::{ProcessStatus, StdioPolicy, testing::RecordingExecutor},
};

fn cwd() -> PathBuf {
    PathBuf::from("host cwd")
}

fn os(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn volume_list_captures_exact_filters_and_renders_table() {
    let executor = RecordingExecutor::default();
    executor.push_capture(b"local\tapp_data\nlocal\tcache\n".to_vec(), 0);
    let mut bytes = Vec::new();
    list::run(
        vec!["dangling=true".to_owned(), "label=team=infra".to_owned()],
        &cwd(),
        &executor,
        &mut Output::plain(&mut bytes),
    )
    .unwrap();

    assert_eq!(
        executor.capture_requests()[0].process.args,
        os(&[
            "volume",
            "ls",
            "--filter",
            "dangling=true",
            "--filter",
            "label=team=infra",
            "--format",
            list::FORMAT,
        ])
    );
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "DRIVER  NAME\nlocal   app_data\nlocal   cache\n"
    );
    assert!(executor.requests().is_empty());
}

#[test]
fn volume_inspect_and_remove_stream_only_explicit_names_without_force_or_prune() {
    for (names, expected) in [
        (vec!["app_data"], vec!["volume", "inspect", "app_data"]),
        (
            vec!["app_data", "cache"],
            vec!["volume", "inspect", "app_data", "cache"],
        ),
    ] {
        let executor = RecordingExecutor::default();
        executor.push_status(31);
        assert_eq!(
            inspect::run(
                false,
                os(&names),
                &cwd(),
                &executor,
                &executor,
                &mut Output::plain(&mut Vec::new()),
            )
            .unwrap(),
            AppOutcome::Child(ProcessStatus::from_code(31))
        );
        let request = &executor.requests()[0];
        assert_eq!(request.args, os(&expected));
        assert_eq!(request.current_dir, cwd());
        assert_eq!(request.stdio, StdioPolicy::Inherit);
        assert!(!request.args.contains(&OsString::from("--force")));
        assert!(!request.args.contains(&OsString::from("prune")));
        assert!(executor.capture_requests().is_empty());
    }
    for (names, expected) in [
        (vec!["app_data"], vec!["volume", "rm", "app_data"]),
        (
            vec!["app_data", "cache"],
            vec!["volume", "rm", "app_data", "cache"],
        ),
    ] {
        let executor = RecordingExecutor::default();
        executor.push_status(31);
        assert_eq!(
            remove::run(os(&names), &cwd(), &executor).unwrap(),
            AppOutcome::Child(ProcessStatus::from_code(31))
        );
        assert_eq!(executor.requests()[0].args, os(&expected));
        assert!(executor.capture_requests().is_empty());
    }
}

#[test]
fn volume_list_reports_parse_nonzero_and_spawn_failures_correctly() {
    let executor = RecordingExecutor::default();
    executor.push_capture(vec![0xff], 0);
    assert!(matches!(
        list::run(
            Vec::new(),
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap_err(),
        AppError::DockerOutput(_)
    ));

    let executor = RecordingExecutor::default();
    executor.push_capture(b"malformed\n".to_vec(), 18);
    assert_eq!(
        list::run(
            Vec::new(),
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap(),
        AppOutcome::Child(ProcessStatus::from_code(18))
    );

    let executor = RecordingExecutor::default();
    executor.push_capture_spawn_error(io::ErrorKind::PermissionDenied, "volume denied");
    assert!(matches!(
        list::run(
            Vec::new(),
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap_err(),
        AppError::Process(_)
    ));
}
