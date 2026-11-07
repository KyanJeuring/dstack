use std::{ffi::OsString, io, path::PathBuf};

use super::list;
use crate::{
    AppError, AppOutcome,
    output::Output,
    process::{ProcessStatus, testing::RecordingExecutor},
};

fn cwd() -> PathBuf {
    PathBuf::from("host cwd")
}

fn os(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn image_list_builds_exact_all_and_ordered_filter_requests() {
    for (all, filters, expected) in [
        (
            false,
            Vec::<String>::new(),
            vec!["image", "ls", "--format", list::FORMAT],
        ),
        (
            true,
            Vec::<String>::new(),
            vec!["image", "ls", "--all", "--format", list::FORMAT],
        ),
        (
            false,
            vec!["dangling=true".to_owned(), "reference=nginx*".to_owned()],
            vec![
                "image",
                "ls",
                "--filter",
                "dangling=true",
                "--filter",
                "reference=nginx*",
                "--format",
                list::FORMAT,
            ],
        ),
    ] {
        let executor = RecordingExecutor::default();
        list::run(
            all,
            filters,
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap();
        let request = &executor.capture_requests()[0].process;
        assert_eq!(request.program, OsString::from("docker"));
        assert_eq!(request.args, os(&expected));
        assert_eq!(request.current_dir, cwd());
        assert!(!request.args.contains(&OsString::from("compose")));
        assert!(executor.requests().is_empty());
    }
}

#[test]
fn image_list_renders_machine_rows_in_docker_order() {
    let executor = RecordingExecutor::default();
    executor.push_capture(
        b"nginx\tlatest\t1e5f3c5b981a\t192MB\npostgres\t17\t381d7bcef901\t430MB\n".to_vec(),
        0,
    );
    let mut bytes = Vec::new();
    list::run(
        false,
        Vec::new(),
        &cwd(),
        &executor,
        &mut Output::plain(&mut bytes),
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        concat!(
            "REPOSITORY  TAG     IMAGE ID      SIZE\n",
            "nginx       latest  1e5f3c5b981a  192MB\n",
            "postgres    17      381d7bcef901  430MB\n",
        )
    );
}

#[test]
fn image_list_handles_malformed_nonzero_and_spawn_results_without_panicking() {
    let executor = RecordingExecutor::default();
    executor.push_capture(b"bad\trow\n".to_vec(), 0);
    assert!(matches!(
        list::run(
            false,
            Vec::new(),
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap_err(),
        AppError::DockerOutput(_)
    ));

    let executor = RecordingExecutor::default();
    executor.push_capture(vec![0xff], 29);
    assert_eq!(
        list::run(
            false,
            Vec::new(),
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap(),
        AppOutcome::Child(ProcessStatus::from_code(29))
    );

    let executor = RecordingExecutor::default();
    executor.push_capture_spawn_error(io::ErrorKind::NotFound, "docker missing");
    assert!(matches!(
        list::run(
            false,
            Vec::new(),
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap_err(),
        AppError::Process(_)
    ));
}
