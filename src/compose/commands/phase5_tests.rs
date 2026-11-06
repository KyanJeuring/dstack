use std::{ffi::OsString, io, path::PathBuf};

use super::{exec, logs, run, services};
use crate::{
    AppError,
    compose::{ComposeSelection, ComposeSelectionSource, shell::SUPPORTED_SHELLS},
    context::ResolvedContext,
    outcome::AppOutcome,
    process::{ProcessSpec, ProcessStatus, StdioPolicy, testing::RecordingExecutor},
    resolution::{ResolvedStack, StackSource},
};

fn context() -> ResolvedContext {
    let project = PathBuf::from("/srv/Example User/项目");
    ResolvedContext {
        stack: ResolvedStack {
            name: "example".to_owned(),
            source: StackSource::Registered,
            project: project.clone(),
            profile_files: Vec::new(),
        },
        compose: ComposeSelection {
            source: ComposeSelectionSource::Explicit,
            files: vec![
                project.join("compose file.yaml"),
                project.join("compose.prod.yaml"),
            ],
        },
    }
}

fn os(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn expected_spec(operation: &[&str], stdio: StdioPolicy) -> ProcessSpec {
    let context = context();
    let mut args = os(&["compose", "--file"]);
    args.push(context.compose.files[0].clone().into_os_string());
    args.push(OsString::from("--file"));
    args.push(context.compose.files[1].clone().into_os_string());
    args.extend(os(operation));
    ProcessSpec::new("docker", context.stack.project)
        .with_args(args)
        .with_stdio(stdio)
}

fn assert_single_request(executor: &RecordingExecutor, operation: &[&str]) {
    assert_eq!(
        executor.requests(),
        [expected_spec(operation, StdioPolicy::Inherit)]
    );
}

#[test]
fn services_uses_compose_config_services_with_inherited_stdio() {
    let executor = RecordingExecutor::default();

    let outcome = services::run(&context(), &executor).unwrap();

    assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(0)));
    assert_single_request(&executor, &["config", "--services"]);
    assert!(!executor.requests()[0].args.contains(&OsString::from("ps")));
}

#[test]
fn logs_has_deterministic_defaults_flags_and_ordered_service_operands() {
    for (service_values, follow, tail, expected) in [
        (vec![], false, 100, vec!["logs", "--tail", "100"]),
        (
            vec!["php"],
            false,
            100,
            vec!["logs", "--tail", "100", "php"],
        ),
        (
            vec!["php", "nginx"],
            false,
            100,
            vec!["logs", "--tail", "100", "php", "nginx"],
        ),
        (vec![], false, 50, vec!["logs", "--tail", "50"]),
        (vec![], false, 0, vec!["logs", "--tail", "0"]),
        (vec![], true, 100, vec!["logs", "--follow", "--tail", "100"]),
        (
            vec!["php"],
            true,
            100,
            vec!["logs", "--follow", "--tail", "100", "php"],
        ),
        (
            vec!["php", "nginx"],
            true,
            25,
            vec!["logs", "--follow", "--tail", "25", "php", "nginx"],
        ),
        (
            vec!["php", "worker", "php"],
            false,
            100,
            vec!["logs", "--tail", "100", "php", "worker", "php"],
        ),
    ] {
        let executor = RecordingExecutor::default();
        let outcome = logs::run(&context(), os(&service_values), follow, tail, &executor).unwrap();

        assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(0)));
        assert_single_request(&executor, &expected);
    }
}

#[test]
fn explicit_exec_preserves_exact_command_argv_and_never_probes() {
    for (service, command, expected) in [
        ("php", vec!["bash"], vec!["exec", "php", "bash"]),
        (
            "php",
            vec!["bash", "-lc", "echo hello"],
            vec!["exec", "php", "bash", "-lc", "echo hello"],
        ),
        (
            "app",
            vec!["sh", "-c", "echo a && echo b"],
            vec!["exec", "app", "sh", "-c", "echo a && echo b"],
        ),
        (
            "app",
            vec!["command", "--file", "foo", "-s", "bar"],
            vec!["exec", "app", "command", "--file", "foo", "-s", "bar"],
        ),
        ("app", vec!["--help"], vec!["exec", "app", "--help"]),
        (
            "服务",
            vec!["命令", "argument with spaces", "-选项", ""],
            vec!["exec", "服务", "命令", "argument with spaces", "-选项", ""],
        ),
    ] {
        let executor = RecordingExecutor::default();
        let outcome =
            exec::run(&context(), OsString::from(service), os(&command), &executor).unwrap();

        assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(0)));
        assert_single_request(&executor, &expected);
    }
}

#[test]
fn shell_probe_uses_documented_order_quiet_stdio_and_stops_after_success() {
    assert_eq!(SUPPORTED_SHELLS, ["bash", "sh", "zsh", "ash"]);

    for (failed, selected) in [(0, "bash"), (1, "sh"), (2, "zsh"), (3, "ash")] {
        let executor = RecordingExecutor::default();
        for _ in 0..failed {
            executor.push_status(127);
        }
        executor.push_status(0);
        executor.push_status(23);

        let outcome = exec::run(&context(), OsString::from("php"), Vec::new(), &executor)
            .expect("a supported shell should be selected");

        assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(23)));
        let requests = executor.requests();
        assert_eq!(requests.len(), failed + 2);
        for (index, request) in requests[..=failed].iter().enumerate() {
            assert_eq!(
                request,
                &expected_spec(
                    &["exec", "-T", "php", SUPPORTED_SHELLS[index], "-c", "exit 0",],
                    StdioPolicy::Null,
                )
            );
        }
        assert_eq!(
            requests.last(),
            Some(&expected_spec(
                &["exec", "php", selected],
                StdioPolicy::Inherit,
            ))
        );
        assert!(
            !requests
                .last()
                .unwrap()
                .args
                .contains(&OsString::from("-T"))
        );
    }
}

#[test]
fn all_failed_shell_probes_report_attempts_without_final_exec() {
    let executor = RecordingExecutor::default();
    for code in [1, 2, 3, 4] {
        executor.push_status(code);
    }

    let error = exec::run(&context(), OsString::from("php"), Vec::new(), &executor).unwrap_err();

    assert!(matches!(
        error,
        AppError::InteractiveShellUnavailable { .. }
    ));
    assert_eq!(executor.requests().len(), SUPPORTED_SHELLS.len());
    let message = error.to_string();
    assert!(message.contains("service 'php'"));
    assert!(message.contains("tried bash, sh, zsh, ash"));
    assert!(message.contains("Ensure the service is running"));
}

#[test]
fn shell_probe_spawn_failure_stops_immediately_as_a_process_error() {
    let executor = RecordingExecutor::default();
    executor.push_spawn_error(io::ErrorKind::NotFound, "docker is missing");

    let error = exec::run(&context(), OsString::from("php"), Vec::new(), &executor).unwrap_err();

    assert!(matches!(error, AppError::Process(_)));
    assert_eq!(executor.requests().len(), 1);
    assert!(
        error
            .to_string()
            .contains("could not start process 'docker'")
    );
}

#[test]
fn run_always_uses_rm_and_preserves_optional_command_argv() {
    for (service, command, expected) in [
        ("worker", vec![], vec!["run", "--rm", "worker"]),
        (
            "worker",
            vec!["bash"],
            vec!["run", "--rm", "worker", "bash"],
        ),
        (
            "worker",
            vec!["ls", "-la", "/data"],
            vec!["run", "--rm", "worker", "ls", "-la", "/data"],
        ),
        (
            "worker",
            vec!["sh", "-c", "echo hello"],
            vec!["run", "--rm", "worker", "sh", "-c", "echo hello"],
        ),
        (
            "worker",
            vec!["command", "--file", "foo", "-s", "bar"],
            vec![
                "run", "--rm", "worker", "command", "--file", "foo", "-s", "bar",
            ],
        ),
        (
            "worker",
            vec!["--help"],
            vec!["run", "--rm", "worker", "--help"],
        ),
        (
            "服务",
            vec!["命令", "argument with spaces", ""],
            vec!["run", "--rm", "服务", "命令", "argument with spaces", ""],
        ),
    ] {
        let executor = RecordingExecutor::default();
        let outcome =
            run::run(&context(), OsString::from(service), os(&command), &executor).unwrap();

        assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(0)));
        assert_single_request(&executor, &expected);
    }
}

#[test]
fn phase5_commands_propagate_numeric_child_statuses() {
    let executor = RecordingExecutor::default();
    executor.push_status(17);
    assert_eq!(
        services::run(&context(), &executor).unwrap(),
        AppOutcome::Child(ProcessStatus::from_code(17))
    );

    let executor = RecordingExecutor::default();
    executor.push_status(12);
    assert_eq!(
        logs::run(&context(), Vec::new(), false, 100, &executor).unwrap(),
        AppOutcome::Child(ProcessStatus::from_code(12))
    );

    let executor = RecordingExecutor::default();
    executor.push_status(42);
    assert_eq!(
        exec::run(&context(), OsString::from("app"), os(&["false"]), &executor,).unwrap(),
        AppOutcome::Child(ProcessStatus::from_code(42))
    );

    let executor = RecordingExecutor::default();
    executor.push_status(9);
    assert_eq!(
        run::run(&context(), OsString::from("worker"), Vec::new(), &executor,).unwrap(),
        AppOutcome::Child(ProcessStatus::from_code(9))
    );
}

#[test]
fn phase5_direct_invocations_report_spawn_errors() {
    for operation in ["services", "logs", "exec", "run"] {
        let executor = RecordingExecutor::default();
        executor.push_spawn_error(io::ErrorKind::PermissionDenied, "docker permission denied");
        let result = match operation {
            "services" => services::run(&context(), &executor),
            "logs" => logs::run(&context(), Vec::new(), false, 100, &executor),
            "exec" => exec::run(&context(), OsString::from("app"), os(&["true"]), &executor),
            "run" => run::run(&context(), OsString::from("worker"), Vec::new(), &executor),
            _ => unreachable!(),
        };

        let error = result.unwrap_err();
        assert!(matches!(error, AppError::Process(_)), "{operation}");
        assert!(error.to_string().contains("docker permission denied"));
        assert_eq!(executor.requests().len(), 1);
    }
}
