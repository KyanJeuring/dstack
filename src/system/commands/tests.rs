use std::{ffi::OsString, io, path::PathBuf};

use super::{df, prune};
use crate::{
    AppError, AppOutcome,
    process::{ProcessStatus, StdioPolicy, testing::RecordingExecutor},
};

fn cwd() -> PathBuf {
    PathBuf::from("host cwd")
}

fn os(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn df_streams_exact_default_and_verbose_argv() {
    for (verbose, expected) in [
        (false, vec!["system", "df"]),
        (true, vec!["system", "df", "--verbose"]),
    ] {
        let executor = RecordingExecutor::default();
        executor.push_status(23);
        assert_eq!(
            df::run(verbose, &cwd(), &executor).unwrap(),
            AppOutcome::Child(ProcessStatus::from_code(23))
        );
        let requests = executor.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].args, os(&expected));
        assert_eq!(requests[0].stdio, StdioPolicy::Inherit);
        assert!(executor.capture_requests().is_empty());
    }
}

#[test]
fn prune_streams_deterministic_independent_options_and_ordered_filters() {
    let cases = [
        (false, false, vec![], false, vec!["system", "prune"]),
        (true, false, vec![], false, vec!["system", "prune", "--all"]),
        (
            false,
            true,
            vec![],
            false,
            vec!["system", "prune", "--volumes"],
        ),
        (
            false,
            false,
            vec![],
            true,
            vec!["system", "prune", "--force"],
        ),
        (
            true,
            true,
            vec!["label=a", "until=24h", "label=a"],
            true,
            vec![
                "system",
                "prune",
                "--all",
                "--volumes",
                "--filter",
                "label=a",
                "--filter",
                "until=24h",
                "--filter",
                "label=a",
                "--force",
            ],
        ),
    ];
    for (all, volumes, filters, force, expected) in cases {
        let executor = RecordingExecutor::default();
        assert_eq!(
            prune::run(
                all,
                volumes,
                filters.into_iter().map(str::to_owned).collect(),
                force,
                &cwd(),
                &executor,
            )
            .unwrap(),
            AppOutcome::Child(ProcessStatus::from_code(0))
        );
        let requests = executor.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].args, os(&expected));
        assert_eq!(requests[0].stdio, StdioPolicy::Inherit);
        assert!(executor.capture_requests().is_empty());
    }
}

#[test]
fn system_commands_preserve_child_status_and_typed_spawn_errors() {
    let executor = RecordingExecutor::default();
    executor.push_status(47);
    assert_eq!(
        df::run(false, &cwd(), &executor).unwrap(),
        AppOutcome::Child(ProcessStatus::from_code(47))
    );

    let executor = RecordingExecutor::default();
    executor.push_spawn_error(io::ErrorKind::NotFound, "docker missing");
    assert!(matches!(
        prune::run(false, false, Vec::new(), false, &cwd(), &executor).unwrap_err(),
        AppError::Process(_)
    ));
}
