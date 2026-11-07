use std::{ffi::OsString, io, path::PathBuf};

use super::{inspect, list};
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
fn network_list_captures_exact_filters_and_renders_table() {
    let executor = RecordingExecutor::default();
    executor.push_capture(
        b"a1b2c3\tbridge\tbridge\tlocal\nd4e5f6\tapp_default\tbridge\tlocal\n".to_vec(),
        0,
    );
    let mut bytes = Vec::new();
    list::run(
        vec!["driver=bridge".to_owned(), "scope=local".to_owned()],
        &cwd(),
        &executor,
        &mut Output::plain(&mut bytes),
    )
    .unwrap();

    assert_eq!(
        executor.capture_requests()[0].process.args,
        os(&[
            "network",
            "ls",
            "--filter",
            "driver=bridge",
            "--filter",
            "scope=local",
            "--format",
            list::FORMAT,
        ])
    );
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        concat!(
            "NETWORK ID  NAME         DRIVER  SCOPE\n",
            "a1b2c3      bridge       bridge  local\n",
            "d4e5f6      app_default  bridge  local\n",
        )
    );
    assert!(executor.requests().is_empty());
}

#[test]
fn network_inspect_streams_one_or_many_names_and_propagates_status() {
    for (names, expected) in [
        (vec!["bridge"], vec!["network", "inspect", "bridge"]),
        (
            vec!["bridge", "app_default"],
            vec!["network", "inspect", "bridge", "app_default"],
        ),
    ] {
        let executor = RecordingExecutor::default();
        executor.push_status(42);
        assert_eq!(
            inspect::run(os(&names), &cwd(), &executor).unwrap(),
            AppOutcome::Child(ProcessStatus::from_code(42))
        );
        let request = &executor.requests()[0];
        assert_eq!(request.args, os(&expected));
        assert_eq!(request.stdio, StdioPolicy::Inherit);
        assert!(executor.capture_requests().is_empty());
    }
}

#[test]
fn network_list_reports_malformed_nonzero_and_spawn_results() {
    let executor = RecordingExecutor::default();
    executor.push_capture(b"wrong\tfields\n".to_vec(), 0);
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
    executor.push_capture(vec![0xff], 21);
    assert_eq!(
        list::run(
            Vec::new(),
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap(),
        AppOutcome::Child(ProcessStatus::from_code(21))
    );

    let executor = RecordingExecutor::default();
    executor.push_capture_spawn_error(io::ErrorKind::ConnectionRefused, "daemon unavailable");
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
