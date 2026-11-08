use std::{ffi::OsString, io, path::PathBuf};

use super::{inspect, ip, list, ports, remove, stats};
use crate::{
    AppError, AppOutcome,
    output::{ColorPolicy, Output},
    process::{CapturePolicy, ProcessStatus, StdioPolicy, testing::RecordingExecutor},
};

fn cwd() -> PathBuf {
    PathBuf::from("normal host cwd/项目")
}

fn os(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn plain_output() -> (Vec<u8>, ColorPolicy) {
    (Vec::new(), ColorPolicy::disabled())
}

#[test]
fn list_builds_exact_capture_requests_for_all_and_ordered_filters() {
    for (all, filters, expected) in [
        (
            false,
            Vec::<String>::new(),
            vec!["container", "ls", "--format", list::FORMAT],
        ),
        (
            true,
            Vec::<String>::new(),
            vec!["container", "ls", "--all", "--format", list::FORMAT],
        ),
        (
            false,
            vec!["name=nginx".to_owned()],
            vec![
                "container",
                "ls",
                "--filter",
                "name=nginx",
                "--format",
                list::FORMAT,
            ],
        ),
        (
            true,
            vec!["name=foo".to_owned(), "status=running".to_owned()],
            vec![
                "container",
                "ls",
                "--all",
                "--filter",
                "name=foo",
                "--filter",
                "status=running",
                "--format",
                list::FORMAT,
            ],
        ),
    ] {
        let executor = RecordingExecutor::default();
        let (mut bytes, color) = plain_output();
        list::run(
            all,
            filters,
            &cwd(),
            &executor,
            &mut Output::new(&mut bytes, color),
        )
        .unwrap();

        assert!(executor.requests().is_empty());
        let requests = executor.capture_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].process.program, OsString::from("docker"));
        assert_eq!(requests[0].process.args, os(&expected));
        assert_eq!(requests[0].process.current_dir, cwd());
        assert_eq!(requests[0].process.stdio, StdioPolicy::Inherit);
        assert_eq!(requests[0].policy, CapturePolicy::StdoutWithInheritedStderr);
        assert!(
            !requests[0]
                .process
                .args
                .contains(&OsString::from("compose"))
        );
    }
}

#[test]
fn list_renders_machine_rows_with_placeholders_spaces_and_unicode() {
    let executor = RecordingExecutor::default();
    executor.push_capture(
        concat!(
            "f45a8a1b1234\tweb\tjellyfin-web-1\tUp 2 hours\t0.0.0.0:8096->8096/tcp\n",
            "1ae871441234\t\t服务\tUp 10 minutes\t\n",
        ),
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

    let rendered = String::from_utf8(bytes).unwrap();
    assert_eq!(
        rendered,
        concat!(
            "ID            SERVICE  NAME            STATUS         PORTS\n",
            "f45a8a1b1234  web      jellyfin-web-1  Up 2 hours     0.0.0.0:8096->8096/tcp\n",
            "1ae871441234  -        服务            Up 10 minutes  -\n",
        )
    );
    assert!(!rendered.contains('\t'));
}

#[test]
fn list_zero_rows_still_renders_header_and_colored_output_uses_shared_table_style() {
    let executor = RecordingExecutor::default();
    let mut plain = Vec::new();
    list::run(
        false,
        Vec::new(),
        &cwd(),
        &executor,
        &mut Output::plain(&mut plain),
    )
    .unwrap();
    assert_eq!(plain, b"ID  SERVICE  NAME  STATUS  PORTS\n");

    let executor = RecordingExecutor::default();
    executor.push_capture(b"abc\tweb\tname\tUp\t\n".to_vec(), 0);
    let mut colored = Vec::new();
    list::run(
        false,
        Vec::new(),
        &cwd(),
        &executor,
        &mut Output::new(&mut colored, ColorPolicy::enabled()),
    )
    .unwrap();
    let colored = String::from_utf8(colored).unwrap();
    assert!(colored.starts_with("\u{1b}[1m\u{1b}[34mID"));
    assert!(colored.contains("abc  web"));
    assert!(!colored.contains('\t'));
}

#[test]
fn list_rejects_malformed_and_invalid_utf8_capture() {
    for output in [b"one\ttwo\n".to_vec(), vec![0xff, b'\n']] {
        let executor = RecordingExecutor::default();
        executor.push_capture(output, 0);
        let error = list::run(
            false,
            Vec::new(),
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap_err();
        assert!(matches!(error, AppError::DockerOutput(_)));
        assert!(error.to_string().contains("dstack container list"));
    }
}

#[test]
fn list_nonzero_status_skips_parsing_and_spawn_failure_stays_distinct() {
    let executor = RecordingExecutor::default();
    executor.push_capture(vec![0xff], 17);
    let outcome = list::run(
        false,
        Vec::new(),
        &cwd(),
        &executor,
        &mut Output::plain(&mut Vec::new()),
    )
    .unwrap();
    assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(17)));

    let executor = RecordingExecutor::default();
    executor.push_capture_spawn_error(io::ErrorKind::NotFound, "docker missing");
    let error = list::run(
        false,
        Vec::new(),
        &cwd(),
        &executor,
        &mut Output::plain(&mut Vec::new()),
    )
    .unwrap_err();
    assert!(matches!(error, AppError::Process(_)));
    assert!(error.to_string().contains("docker missing"));
}

#[test]
fn ports_without_container_captures_and_renders_while_specific_modes_stream() {
    let executor = RecordingExecutor::default();
    executor.push_capture(
        b"jellyfin\t0.0.0.0:8096->8096/tcp\npostgres\t\n".to_vec(),
        0,
    );
    let mut bytes = Vec::new();
    ports::run(
        None,
        None,
        &cwd(),
        &executor,
        &executor,
        &mut Output::plain(&mut bytes),
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        concat!(
            "NAME      PORTS\n",
            "jellyfin  0.0.0.0:8096->8096/tcp\n",
            "postgres  -\n",
        )
    );
    assert_eq!(
        executor.capture_requests()[0].process.args,
        os(&["container", "ls", "--format", "{{.Names}}\t{{.Ports}}"])
    );

    for (private_port, expected) in [
        (None, vec!["container", "port", "nginx"]),
        (
            Some(OsString::from("8080/tcp")),
            vec!["container", "port", "nginx", "8080/tcp"],
        ),
    ] {
        let executor = RecordingExecutor::default();
        ports::run(
            Some(OsString::from("nginx")),
            private_port,
            &cwd(),
            &executor,
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap();
        assert_eq!(executor.requests()[0].args, os(&expected));
        assert_eq!(executor.requests()[0].stdio, StdioPolicy::Inherit);
        assert!(executor.capture_requests().is_empty());
    }
}

#[test]
fn ip_renders_one_many_and_no_networks_and_uses_inspect_format() {
    for (capture, expected) in [
        ("bridge\t172.17.0.2\n", "NETWORK  IP\nbridge   172.17.0.2\n"),
        (
            "bridge\t172.17.0.2\njellyfin_default\t172.19.0.3\n",
            concat!(
                "NETWORK           IP\n",
                "bridge            172.17.0.2\n",
                "jellyfin_default  172.19.0.3\n",
            ),
        ),
        ("", "NETWORK  IP\n"),
    ] {
        let executor = RecordingExecutor::default();
        executor.push_capture(capture.as_bytes().to_vec(), 0);
        let mut bytes = Vec::new();
        ip::run(
            OsString::from("nginx"),
            &cwd(),
            &executor,
            &mut Output::plain(&mut bytes),
        )
        .unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), expected);
        assert_eq!(
            executor.capture_requests()[0].process.args,
            [
                OsString::from("container"),
                OsString::from("inspect"),
                OsString::from("--format"),
                OsString::from(ip::FORMAT),
                OsString::from("nginx"),
            ]
        );
    }
}

#[test]
fn ip_propagates_nonzero_spawn_and_malformed_capture() {
    let executor = RecordingExecutor::default();
    executor.push_capture(vec![0xff], 19);
    assert_eq!(
        ip::run(
            OsString::from("nginx"),
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap(),
        AppOutcome::Child(ProcessStatus::from_code(19))
    );

    let executor = RecordingExecutor::default();
    executor.push_capture(b"malformed\n".to_vec(), 0);
    assert!(matches!(
        ip::run(
            OsString::from("nginx"),
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap_err(),
        AppError::DockerOutput(_)
    ));

    let executor = RecordingExecutor::default();
    executor.push_capture_spawn_error(io::ErrorKind::PermissionDenied, "inspect denied");
    assert!(matches!(
        ip::run(
            OsString::from("nginx"),
            &cwd(),
            &executor,
            &mut Output::plain(&mut Vec::new()),
        )
        .unwrap_err(),
        AppError::Process(_)
    ));
}

#[test]
fn stats_always_streams_exact_native_options_and_operands() {
    for (all, no_stream, containers, expected) in [
        (false, false, vec![], vec!["container", "stats"]),
        (
            false,
            false,
            vec!["nginx"],
            vec!["container", "stats", "nginx"],
        ),
        (
            false,
            false,
            vec!["nginx", "postgres"],
            vec!["container", "stats", "nginx", "postgres"],
        ),
        (true, false, vec![], vec!["container", "stats", "--all"]),
        (
            false,
            true,
            vec![],
            vec!["container", "stats", "--no-stream"],
        ),
        (
            true,
            true,
            vec!["nginx"],
            vec!["container", "stats", "--all", "--no-stream", "nginx"],
        ),
    ] {
        let executor = RecordingExecutor::default();
        stats::run(all, no_stream, os(&containers), &cwd(), &executor).unwrap();
        assert_eq!(executor.requests()[0].args, os(&expected));
        assert_eq!(executor.requests()[0].stdio, StdioPolicy::Inherit);
        assert!(executor.capture_requests().is_empty());
    }
}

#[test]
fn inspect_streams_one_or_many_containers_and_propagates_status() {
    for (containers, expected) in [
        (vec!["nginx"], vec!["container", "inspect", "nginx"]),
        (
            vec!["nginx", "postgres"],
            vec!["container", "inspect", "nginx", "postgres"],
        ),
    ] {
        let executor = RecordingExecutor::default();
        executor.push_status(23);
        let outcome = inspect::run(os(&containers), &cwd(), &executor).unwrap();
        assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(23)));
        assert_eq!(executor.requests()[0].args, os(&expected));
        assert_eq!(executor.requests()[0].stdio, StdioPolicy::Inherit);
        assert!(executor.capture_requests().is_empty());
    }
}

#[test]
fn remove_streams_exact_explicit_containers_without_force_stop_or_volumes() {
    for (containers, expected) in [
        (vec!["web"], vec!["container", "rm", "web"]),
        (vec!["web", "db"], vec!["container", "rm", "web", "db"]),
    ] {
        let executor = RecordingExecutor::default();
        executor.push_status(27);
        assert_eq!(
            remove::run(os(&containers), &cwd(), &executor).unwrap(),
            AppOutcome::Child(ProcessStatus::from_code(27))
        );
        let request = &executor.requests()[0];
        assert_eq!(request.args, os(&expected));
        assert_eq!(request.stdio, StdioPolicy::Inherit);
        assert!(!request.args.contains(&OsString::from("--force")));
        assert!(!request.args.contains(&OsString::from("--volumes")));
        assert!(!request.args.contains(&OsString::from("stop")));
        assert!(executor.capture_requests().is_empty());
    }

    let executor = RecordingExecutor::default();
    executor.push_spawn_error(io::ErrorKind::NotFound, "docker missing");
    assert!(matches!(
        remove::run(os(&["web"]), &cwd(), &executor).unwrap_err(),
        AppError::Process(_)
    ));
}
