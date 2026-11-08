use std::{collections::HashSet, ffi::OsString};

use crate::{
    compose::ComposeInvocation,
    context::ResolvedContext,
    docker::DockerInvocation,
    process::{CapturedProcessSpec, ProcessOutputExecutor},
};

use super::{
    model::{ComposeContainerState, ContainerRuntimeState, DesiredState, DesiredStateAssessment},
    parse,
};

pub(crate) struct ComposeStateInspector<'a> {
    executor: &'a dyn ProcessOutputExecutor,
}

impl<'a> ComposeStateInspector<'a> {
    pub(crate) const fn new(executor: &'a dyn ProcessOutputExecutor) -> Self {
        Self { executor }
    }

    pub(crate) fn assess(
        &self,
        context: &ResolvedContext,
        requested_services: &[OsString],
        desired: DesiredState,
    ) -> DesiredStateAssessment {
        self.try_assess(context, requested_services, desired)
            .unwrap_or(DesiredStateAssessment::NeedsCommand)
    }

    fn try_assess(
        &self,
        context: &ResolvedContext,
        requested_services: &[OsString],
        desired: DesiredState,
    ) -> Option<DesiredStateAssessment> {
        let configured = self.configured_services(context)?;
        if configured.is_empty() {
            return None;
        }
        let configured_set = configured
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let explicit = !requested_services.is_empty();
        let targets = if explicit {
            let mut targets = Vec::with_capacity(requested_services.len());
            for requested in requested_services {
                let requested = requested.to_str()?;
                if !configured_set.contains(requested) {
                    return None;
                }
                targets.push(requested.to_owned());
            }
            targets
        } else {
            configured
        };

        let ids = self.container_ids(context, &targets)?;
        if ids.is_empty() {
            return Some(DesiredStateAssessment::NeedsCommand);
        }
        let containers = self.container_states(context, &ids)?;
        assess_containers(&containers, &targets, desired, explicit)
    }

    fn configured_services(&self, context: &ResolvedContext) -> Option<Vec<String>> {
        let spec = CapturedProcessSpec::silent(
            ComposeInvocation::new(context).process(["config", "--services"]),
        );
        let output = self.capture(&spec)?;
        parse::service_names(&output)
    }

    fn container_ids(&self, context: &ResolvedContext, targets: &[String]) -> Option<Vec<String>> {
        let args = [
            ["ps", "--all", "--quiet", "--no-trunc"]
                .into_iter()
                .map(OsString::from)
                .collect::<Vec<_>>(),
            targets.iter().map(OsString::from).collect(),
        ]
        .concat();
        let spec = CapturedProcessSpec::silent(ComposeInvocation::new(context).process(args));
        let output = self.capture(&spec)?;
        parse::container_ids(&output)
    }

    fn container_states(
        &self,
        context: &ResolvedContext,
        ids: &[String],
    ) -> Option<Vec<ComposeContainerState>> {
        let args = [
            vec![OsString::from("container"), OsString::from("inspect")],
            ids.iter().map(OsString::from).collect(),
        ]
        .concat();
        let spec = CapturedProcessSpec::silent(
            DockerInvocation::new(&context.stack.project).process(args),
        );
        let output = self.capture(&spec)?;
        parse::inspected_containers(&output, ids)
    }

    fn capture(&self, spec: &CapturedProcessSpec) -> Option<Vec<u8>> {
        let captured = self.executor.capture(spec).ok()?;
        captured.status.success().then_some(captured.stdout)
    }
}

fn assess_containers(
    containers: &[ComposeContainerState],
    targets: &[String],
    desired: DesiredState,
    explicit: bool,
) -> Option<DesiredStateAssessment> {
    let unique_container_count = containers
        .iter()
        .map(|container| container.container_id.as_str())
        .collect::<HashSet<_>>()
        .len();
    if unique_container_count != containers.len() {
        return None;
    }
    let target_set = targets.iter().map(String::as_str).collect::<HashSet<_>>();
    if containers
        .iter()
        .any(|container| !target_set.contains(container.service.as_str()))
    {
        return None;
    }

    let complete = match desired {
        DesiredState::Running => targets.iter().all(|target| {
            containers
                .iter()
                .any(|container| container.service == *target)
        }),
        DesiredState::Stopped if explicit => targets.iter().all(|target| {
            containers
                .iter()
                .any(|container| container.service == *target)
        }),
        DesiredState::Stopped => !containers.is_empty(),
    };
    if !complete {
        return Some(DesiredStateAssessment::NeedsCommand);
    }

    let satisfied = containers.iter().all(|container| match desired {
        DesiredState::Running => container.runtime == ContainerRuntimeState::Running,
        DesiredState::Stopped => container.runtime == ContainerRuntimeState::Stopped,
    });
    Some(if satisfied {
        DesiredStateAssessment::AlreadySatisfied
    } else {
        DesiredStateAssessment::NeedsCommand
    })
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, io, path::PathBuf};

    use super::ComposeStateInspector;
    use crate::{
        compose::{
            ComposeSelection, ComposeSelectionSource,
            state::{DesiredState, DesiredStateAssessment},
        },
        context::ResolvedContext,
        process::{CapturePolicy, testing::RecordingExecutor},
        resolution::{ResolvedStack, StackSource},
    };

    const WEB_1: &str = "aaaaaaaaaaaa";
    const WEB_2: &str = "bbbbbbbbbbbb";
    const DB_1: &str = "cccccccccccc";
    const REDIS_1: &str = "dddddddddddd";

    #[derive(Clone, Copy)]
    struct Container<'a> {
        id: &'a str,
        service: &'a str,
        status: &'a str,
        running: bool,
        paused: bool,
        restarting: bool,
        one_off: bool,
    }

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

    fn running(id: &'static str, service: &'static str) -> Container<'static> {
        Container {
            id,
            service,
            status: "running",
            running: true,
            paused: false,
            restarting: false,
            one_off: false,
        }
    }

    fn stopped(id: &'static str, service: &'static str) -> Container<'static> {
        Container {
            id,
            service,
            status: "exited",
            running: false,
            paused: false,
            restarting: false,
            one_off: false,
        }
    }

    fn queue_fixture(
        executor: &RecordingExecutor,
        configured: &[&str],
        containers: &[Container<'_>],
    ) {
        executor.push_capture(format!("{}\n", configured.join("\n")), 0);
        if containers.is_empty() {
            executor.push_capture(Vec::new(), 0);
            return;
        }
        executor.push_capture(
            format!(
                "{}\n",
                containers
                    .iter()
                    .map(|container| container.id)
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            0,
        );
        let objects = containers
            .iter()
            .map(|container| {
                format!(
                    r#"{{"Id":"{}","Config":{{"Labels":{{"com.docker.compose.service":"{}","com.docker.compose.oneoff":"{}"}}}},"State":{{"Status":"{}","Running":{},"Paused":{},"Restarting":{},"Health":{{"Status":"unhealthy"}}}}}}"#,
                    container.id,
                    container.service,
                    if container.one_off { "True" } else { "False" },
                    container.status,
                    container.running,
                    container.paused,
                    container.restarting,
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        executor.push_capture(format!("[{objects}]"), 0);
    }

    fn assess(
        executor: &RecordingExecutor,
        services: &[&str],
        desired: DesiredState,
    ) -> DesiredStateAssessment {
        ComposeStateInspector::new(executor).assess(
            &context(),
            &services.iter().map(OsString::from).collect::<Vec<_>>(),
            desired,
        )
    }

    #[test]
    fn running_requires_complete_target_coverage_and_every_replica() {
        let executor = RecordingExecutor::default();
        queue_fixture(
            &executor,
            &["web", "db", "redis"],
            &[
                running(WEB_1, "web"),
                running(DB_1, "db"),
                running(REDIS_1, "redis"),
            ],
        );
        assert_eq!(
            assess(&executor, &[], DesiredState::Running),
            DesiredStateAssessment::AlreadySatisfied
        );

        let executor = RecordingExecutor::default();
        queue_fixture(
            &executor,
            &["web", "db", "redis"],
            &[running(WEB_1, "web"), running(REDIS_1, "redis")],
        );
        assert_eq!(
            assess(&executor, &["web", "redis"], DesiredState::Running),
            DesiredStateAssessment::AlreadySatisfied
        );

        let executor = RecordingExecutor::default();
        queue_fixture(
            &executor,
            &["web"],
            &[running(WEB_1, "web"), stopped(WEB_2, "web")],
        );
        assert_eq!(
            assess(&executor, &["web"], DesiredState::Running),
            DesiredStateAssessment::NeedsCommand
        );

        let executor = RecordingExecutor::default();
        queue_fixture(
            &executor,
            &["web"],
            &[running(WEB_1, "web"), running(WEB_2, "web")],
        );
        assert_eq!(
            assess(&executor, &["web"], DesiredState::Running),
            DesiredStateAssessment::AlreadySatisfied
        );
    }

    #[test]
    fn running_missing_unknown_and_one_off_targets_need_the_real_command() {
        let executor = RecordingExecutor::default();
        queue_fixture(&executor, &["web", "db"], &[running(WEB_1, "web")]);
        assert_eq!(
            assess(&executor, &[], DesiredState::Running),
            DesiredStateAssessment::NeedsCommand
        );

        let executor = RecordingExecutor::default();
        queue_fixture(&executor, &["web", "db"], &[]);
        assert_eq!(
            assess(&executor, &["db"], DesiredState::Running),
            DesiredStateAssessment::NeedsCommand
        );

        let executor = RecordingExecutor::default();
        executor.push_capture(b"web\n".to_vec(), 0);
        assert_eq!(
            assess(&executor, &["does-not-exist"], DesiredState::Running),
            DesiredStateAssessment::NeedsCommand
        );
        assert_eq!(executor.capture_requests().len(), 1);

        let executor = RecordingExecutor::default();
        let mut one_off = running(WEB_1, "web");
        one_off.one_off = true;
        queue_fixture(&executor, &["web"], &[one_off]);
        assert_eq!(
            assess(&executor, &["web"], DesiredState::Running),
            DesiredStateAssessment::NeedsCommand
        );
    }

    #[test]
    fn stopped_requires_existing_targets_and_every_replica_to_be_definitively_stopped() {
        let executor = RecordingExecutor::default();
        queue_fixture(
            &executor,
            &["web", "db"],
            &[stopped(WEB_1, "web"), stopped(DB_1, "db")],
        );
        assert_eq!(
            assess(&executor, &[], DesiredState::Stopped),
            DesiredStateAssessment::AlreadySatisfied
        );

        let executor = RecordingExecutor::default();
        queue_fixture(&executor, &["web", "db"], &[]);
        assert_eq!(
            assess(&executor, &[], DesiredState::Stopped),
            DesiredStateAssessment::NeedsCommand
        );

        let executor = RecordingExecutor::default();
        queue_fixture(
            &executor,
            &["web", "redis", "db"],
            &[stopped(WEB_1, "web"), stopped(REDIS_1, "redis")],
        );
        assert_eq!(
            assess(&executor, &["web", "redis"], DesiredState::Stopped),
            DesiredStateAssessment::AlreadySatisfied
        );

        let executor = RecordingExecutor::default();
        queue_fixture(
            &executor,
            &["web"],
            &[stopped(WEB_1, "web"), running(WEB_2, "web")],
        );
        assert_eq!(
            assess(&executor, &["web"], DesiredState::Stopped),
            DesiredStateAssessment::NeedsCommand
        );
    }

    #[test]
    fn paused_restarting_and_unknown_runtime_states_are_never_already_stopped() {
        for container in [
            Container {
                paused: true,
                ..running(WEB_1, "web")
            },
            Container {
                status: "restarting",
                restarting: true,
                ..running(WEB_1, "web")
            },
            Container {
                status: "removing",
                running: false,
                ..stopped(WEB_1, "web")
            },
        ] {
            let executor = RecordingExecutor::default();
            queue_fixture(&executor, &["web"], &[container]);
            assert_eq!(
                assess(&executor, &["web"], DesiredState::Stopped),
                DesiredStateAssessment::NeedsCommand
            );
        }
    }

    #[test]
    fn probes_use_exact_resolved_files_project_and_silent_capture() {
        let executor = RecordingExecutor::default();
        queue_fixture(
            &executor,
            &["web", "redis", "db"],
            &[running(WEB_1, "web"), running(REDIS_1, "redis")],
        );
        assert_eq!(
            assess(&executor, &["web", "redis"], DesiredState::Running),
            DesiredStateAssessment::AlreadySatisfied
        );

        let context = context();
        let requests = executor.capture_requests();
        assert_eq!(requests.len(), 3);
        let prefix = [
            vec![OsString::from("compose"), OsString::from("--file")],
            vec![
                context.compose.files[0].clone().into_os_string(),
                OsString::from("--file"),
                context.compose.files[1].clone().into_os_string(),
            ],
        ]
        .concat();
        assert_eq!(
            requests[0].process.args,
            [
                prefix.clone(),
                ["config", "--services"].map(OsString::from).to_vec()
            ]
            .concat()
        );
        assert_eq!(
            requests[1].process.args,
            [
                prefix,
                ["ps", "--all", "--quiet", "--no-trunc", "web", "redis"]
                    .map(OsString::from)
                    .to_vec(),
            ]
            .concat()
        );
        assert_eq!(
            requests[2].process.args,
            ["container", "inspect", WEB_1, REDIS_1].map(OsString::from)
        );
        assert!(
            requests
                .iter()
                .all(|request| request.process.current_dir == context.stack.project)
        );
        assert!(
            requests
                .iter()
                .all(|request| request.policy == CapturePolicy::StdoutWithNullStderr)
        );
        assert!(executor.requests().is_empty());
    }

    #[test]
    fn every_probe_failure_or_malformed_output_is_inconclusive() {
        let executor = RecordingExecutor::default();
        executor.push_capture(b"probe noise".to_vec(), 17);
        assert_eq!(
            assess(&executor, &[], DesiredState::Running),
            DesiredStateAssessment::NeedsCommand
        );

        let executor = RecordingExecutor::default();
        executor.push_capture(b"web\n".to_vec(), 0);
        executor.push_capture(b"daemon noise".to_vec(), 18);
        assert_eq!(
            assess(&executor, &[], DesiredState::Stopped),
            DesiredStateAssessment::NeedsCommand
        );

        let executor = RecordingExecutor::default();
        executor.push_capture(b"web\n".to_vec(), 0);
        executor.push_capture(format!("{WEB_1}\n"), 0);
        executor.push_capture(b"unsupported json".to_vec(), 0);
        assert_eq!(
            assess(&executor, &[], DesiredState::Running),
            DesiredStateAssessment::NeedsCommand
        );

        let executor = RecordingExecutor::default();
        executor.push_capture_spawn_error(io::ErrorKind::NotFound, "docker missing");
        assert_eq!(
            assess(&executor, &[], DesiredState::Running),
            DesiredStateAssessment::NeedsCommand
        );
    }
}
