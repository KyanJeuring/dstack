use std::{ffi::OsString, path::PathBuf};

use super::{down, pull, rebuild, recreate, restart, start, stop, up, update};
use crate::{
    compose::{ComposeSelection, ComposeSelectionSource},
    context::ResolvedContext,
    outcome::AppOutcome,
    process::{ProcessExecutor, ProcessStatus, testing::RecordingExecutor},
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
                project.join("compose.yaml"),
                project.join("compose prod.yaml"),
            ],
        },
    }
}

fn os(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn assert_requests(executor: &RecordingExecutor, operations: &[Vec<&str>]) {
    let context = context();
    let requests = executor.requests();
    assert_eq!(requests.len(), operations.len());
    for (request, operation) in requests.iter().zip(operations) {
        let mut expected = os(&["compose", "--file"]);
        expected.push(context.compose.files[0].clone().into_os_string());
        expected.push(OsString::from("--file"));
        expected.push(context.compose.files[1].clone().into_os_string());
        expected.extend(os(operation));

        assert_eq!(request.program, OsString::from("docker"));
        assert_eq!(request.args, expected);
        assert_eq!(request.current_dir, context.stack.project);
    }
}

#[test]
fn up_has_typed_defaults_optional_orphans_and_service_operands() {
    for (services, remove_orphans, expected) in [
        (vec![], false, vec!["up", "-d", "--build"]),
        (vec!["php"], false, vec!["up", "-d", "--build", "php"]),
        (
            vec!["php", "nginx"],
            false,
            vec!["up", "-d", "--build", "php", "nginx"],
        ),
        (
            vec![],
            true,
            vec!["up", "-d", "--build", "--remove-orphans"],
        ),
    ] {
        let executor = RecordingExecutor::default();
        let outcome = up::run(&context(), os(&services), remove_orphans, &executor).unwrap();
        assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(0)));
        assert_requests(&executor, &[expected]);
    }
}

type ServiceHandler = fn(
    &ResolvedContext,
    Vec<OsString>,
    &dyn ProcessExecutor,
) -> Result<AppOutcome, crate::error::AppError>;

#[test]
fn start_stop_and_restart_append_services_without_changing_the_operation() {
    for (run, operation) in [
        (start::run as ServiceHandler, "start"),
        (stop::run as ServiceHandler, "stop"),
        (restart::run as ServiceHandler, "restart"),
    ] {
        for services in [vec![], vec!["php"], vec!["php", "nginx"]] {
            let executor = RecordingExecutor::default();
            run(&context(), os(&services), &executor).unwrap();
            let mut expected = vec![operation];
            expected.extend(services);
            assert_requests(&executor, &[expected]);
        }
    }
}

#[test]
fn down_and_recreate_use_explicit_volume_semantics() {
    for (volumes, expected) in [(false, vec!["down"]), (true, vec!["down", "--volumes"])] {
        let executor = RecordingExecutor::default();
        down::run(&context(), volumes, &executor).unwrap();
        assert_requests(&executor, std::slice::from_ref(&expected));

        let executor = RecordingExecutor::default();
        recreate::run(&context(), volumes, &executor).unwrap();
        assert_requests(&executor, &[expected, vec!["up", "-d", "--build"]]);
    }
}

#[test]
fn pull_and_update_apply_service_operands_to_each_relevant_step() {
    for services in [vec![], vec!["php"], vec!["php", "worker"]] {
        let executor = RecordingExecutor::default();
        pull::run(&context(), os(&services), &executor).unwrap();
        let mut pull_expected = vec!["pull"];
        pull_expected.extend(services.iter().copied());
        assert_requests(&executor, std::slice::from_ref(&pull_expected));

        let executor = RecordingExecutor::default();
        update::run(&context(), os(&services), &executor).unwrap();
        let mut up_expected = vec!["up", "-d"];
        up_expected.extend(services);
        assert_requests(&executor, &[pull_expected, up_expected]);
    }
}

#[test]
fn rebuild_builds_then_starts_with_services_and_optional_no_cache() {
    for (services, no_cache, build_expected, up_expected) in [
        (vec![], false, vec!["build"], vec!["up", "-d"]),
        (
            vec!["php"],
            false,
            vec!["build", "php"],
            vec!["up", "-d", "php"],
        ),
        (vec![], true, vec!["build", "--no-cache"], vec!["up", "-d"]),
        (
            vec!["php"],
            true,
            vec!["build", "--no-cache", "php"],
            vec!["up", "-d", "php"],
        ),
    ] {
        let executor = RecordingExecutor::default();
        rebuild::run(&context(), os(&services), no_cache, &executor).unwrap();
        assert_requests(&executor, &[build_expected, up_expected]);
    }
}

#[test]
fn multi_step_operations_stop_at_first_failure_and_return_first_status() {
    for operation in ["recreate", "update", "rebuild"] {
        let executor = RecordingExecutor::default();
        executor.push_status(17);
        let outcome = match operation {
            "recreate" => recreate::run(&context(), false, &executor),
            "update" => update::run(&context(), os(&["php"]), &executor),
            "rebuild" => rebuild::run(&context(), os(&["php"]), false, &executor),
            _ => unreachable!(),
        }
        .unwrap();

        assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(17)));
        assert_eq!(
            executor.requests().len(),
            1,
            "{operation} ran after failure"
        );
    }
}

#[test]
fn multi_step_operations_return_the_second_failure_status() {
    for operation in ["recreate", "update", "rebuild"] {
        let executor = RecordingExecutor::default();
        executor.push_status(0);
        executor.push_status(23);
        let outcome = match operation {
            "recreate" => recreate::run(&context(), false, &executor),
            "update" => update::run(&context(), Vec::new(), &executor),
            "rebuild" => rebuild::run(&context(), Vec::new(), false, &executor),
            _ => unreachable!(),
        }
        .unwrap();

        assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(23)));
        assert_eq!(executor.requests().len(), 2);
    }
}
