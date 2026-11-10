use std::{ffi::OsString, io::Write, path::Path};

use serde_json::Value;

use crate::{
    error::AppError,
    outcome::AppOutcome,
    output::Output,
    process::{ProcessOutputExecutor, ProcessStatus},
};

use super::{DockerInvocation, DockerOutputError};
use super::{TARGET_BATCH_SIZE, parse_identifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InspectResource {
    Container,
    Image,
    Volume,
    Network,
}

impl InspectResource {
    fn enumeration_args(self) -> Vec<OsString> {
        match self {
            Self::Container => os(&["container", "ls", "--all", "--quiet", "--no-trunc"]),
            Self::Image => os(&["image", "ls", "--all", "--quiet", "--no-trunc"]),
            Self::Volume => os(&["volume", "ls", "--quiet"]),
            Self::Network => os(&["network", "ls", "--quiet", "--no-trunc"]),
        }
    }

    fn inspect_prefix(self) -> Vec<OsString> {
        match self {
            Self::Container => os(&["container", "inspect"]),
            Self::Image => os(&["image", "inspect"]),
            Self::Volume => os(&["volume", "inspect"]),
            Self::Network => os(&["network", "inspect"]),
        }
    }

    const fn command(self) -> &'static str {
        match self {
            Self::Container => "dstack container inspect --all",
            Self::Image => "dstack image inspect --all",
            Self::Volume => "dstack volume inspect --all",
            Self::Network => "dstack network inspect --all",
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct InspectAllOutput {
    pub(crate) identifiers: Vec<String>,
    pub(crate) objects: Vec<Value>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum InspectAllResult {
    Complete(InspectAllOutput),
    Child(ProcessStatus),
}

pub(crate) fn inspect_all(
    resource: InspectResource,
    current_dir: &Path,
    executor: &dyn ProcessOutputExecutor,
) -> Result<InspectAllResult, AppError> {
    let invocation = DockerInvocation::new(current_dir);
    let enumeration = executor.capture(&invocation.capture(resource.enumeration_args()))?;
    if !enumeration.status.success() {
        return Ok(InspectAllResult::Child(enumeration.status));
    }

    let identifiers = parse_identifiers(resource.command(), &enumeration.stdout)?;
    let mut objects = Vec::new();
    for batch in identifiers.chunks(TARGET_BATCH_SIZE) {
        let args = resource
            .inspect_prefix()
            .into_iter()
            .chain(batch.iter().map(OsString::from));
        let inspected = executor.capture(&invocation.capture(args))?;
        if !inspected.status.success() {
            return Ok(InspectAllResult::Child(inspected.status));
        }
        let mut batch =
            serde_json::from_slice::<Vec<Value>>(&inspected.stdout).map_err(|source| {
                DockerOutputError::InvalidJson {
                    command: resource.command(),
                    source,
                }
            })?;
        objects.append(&mut batch);
    }

    Ok(InspectAllResult::Complete(InspectAllOutput {
        identifiers,
        objects,
    }))
}

pub(crate) fn write_inspect_json(
    result: InspectAllResult,
    output: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    match result {
        InspectAllResult::Child(status) => Ok(AppOutcome::Child(status)),
        InspectAllResult::Complete(inspected) => {
            let json = serde_json::to_vec_pretty(&inspected.objects)
                .map_err(|source| std::io::Error::other(source.to_string()))?;
            output.write_all(&json)?;
            output.write_all(b"\n")?;
            Ok(AppOutcome::Success)
        }
    }
}

fn os(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, io, path::PathBuf};

    use serde_json::{Value, json};

    use super::{InspectAllResult, InspectResource, inspect_all, write_inspect_json};
    use crate::{
        AppError, AppOutcome,
        docker::TARGET_BATCH_SIZE,
        output::{ColorPolicy, Output},
        process::{CapturePolicy, ProcessStatus, testing::RecordingExecutor},
    };

    fn cwd() -> PathBuf {
        PathBuf::from("host cwd/项目")
    }

    #[test]
    fn resource_enumeration_uses_exact_machine_readable_commands() {
        for (resource, expected) in [
            (
                InspectResource::Container,
                vec!["container", "ls", "--all", "--quiet", "--no-trunc"],
            ),
            (
                InspectResource::Image,
                vec!["image", "ls", "--all", "--quiet", "--no-trunc"],
            ),
            (InspectResource::Volume, vec!["volume", "ls", "--quiet"]),
            (
                InspectResource::Network,
                vec!["network", "ls", "--quiet", "--no-trunc"],
            ),
        ] {
            let executor = RecordingExecutor::default();
            let result = inspect_all(resource, &cwd(), &executor).unwrap();
            assert!(matches!(result, InspectAllResult::Complete(_)));
            let request = &executor.capture_requests()[0];
            assert_eq!(
                request.process.args,
                expected.into_iter().map(OsString::from).collect::<Vec<_>>()
            );
            assert_eq!(request.process.current_dir, cwd());
            assert_eq!(request.policy, CapturePolicy::StdoutWithInheritedStderr);
        }
    }

    #[test]
    fn duplicate_image_and_generic_identifiers_are_inspected_once_in_first_seen_order() {
        let executor = RecordingExecutor::default();
        executor.push_capture(
            b"sha256:aaa\nsha256:bbb\nsha256:aaa\nsha256:ccc\nsha256:bbb\n".to_vec(),
            0,
        );
        executor.push_capture(br#"[{"Id":"aaa"},{"Id":"bbb"},{"Id":"ccc"}]"#.to_vec(), 0);
        let InspectAllResult::Complete(result) =
            inspect_all(InspectResource::Image, &cwd(), &executor).unwrap()
        else {
            panic!("inspection should complete");
        };
        assert_eq!(
            result.identifiers,
            ["sha256:aaa", "sha256:bbb", "sha256:ccc"]
        );
        assert_eq!(
            result.objects,
            [
                json!({"Id":"aaa"}),
                json!({"Id":"bbb"}),
                json!({"Id":"ccc"})
            ]
        );
        assert_eq!(
            executor.capture_requests()[1].process.args,
            ["image", "inspect", "sha256:aaa", "sha256:bbb", "sha256:ccc"].map(OsString::from)
        );
    }

    #[test]
    fn every_resource_inspects_each_enumerated_target_structurally_in_order() {
        for (resource, identifiers, prefix) in [
            (
                InspectResource::Container,
                vec!["container-a", "container-b"],
                vec!["container", "inspect"],
            ),
            (
                InspectResource::Volume,
                vec!["app.data-1", "cache_name"],
                vec!["volume", "inspect"],
            ),
            (
                InspectResource::Network,
                vec!["bridge-id", "host-id", "none-id"],
                vec!["network", "inspect"],
            ),
        ] {
            let executor = RecordingExecutor::default();
            executor.push_capture(format!("{}\n", identifiers.join("\n")), 0);
            executor.push_capture(
                serde_json::to_vec(
                    &identifiers
                        .iter()
                        .map(|identifier| json!({"target": identifier}))
                        .collect::<Vec<_>>(),
                )
                .unwrap(),
                0,
            );
            let InspectAllResult::Complete(result) =
                inspect_all(resource, &cwd(), &executor).unwrap()
            else {
                panic!("inspection should complete");
            };
            assert_eq!(result.identifiers, identifiers);
            assert_eq!(
                executor.capture_requests()[1].process.args,
                prefix
                    .into_iter()
                    .chain(identifiers)
                    .map(OsString::from)
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn inspection_is_bounded_and_aggregates_batches_in_target_order() {
        let identifiers = (0..TARGET_BATCH_SIZE + 3)
            .map(|index| format!("id{index:03}"))
            .collect::<Vec<_>>();
        let executor = RecordingExecutor::default();
        executor.push_capture(format!("{}\n", identifiers.join("\n")), 0);
        for batch in identifiers.chunks(TARGET_BATCH_SIZE) {
            executor.push_capture(
                serde_json::to_vec(
                    &batch
                        .iter()
                        .map(|identifier| json!({"Id": identifier}))
                        .collect::<Vec<_>>(),
                )
                .unwrap(),
                0,
            );
        }

        let InspectAllResult::Complete(result) =
            inspect_all(InspectResource::Container, &cwd(), &executor).unwrap()
        else {
            panic!("inspection should complete");
        };
        assert_eq!(result.identifiers, identifiers);
        assert_eq!(result.objects.len(), TARGET_BATCH_SIZE + 3);
        let requests = executor.capture_requests();
        assert_eq!(requests.len(), 3);
        for request in &requests[1..] {
            assert!(request.process.args.len() - 2 <= TARGET_BATCH_SIZE);
        }
        assert_eq!(
            result
                .objects
                .iter()
                .map(|object| object["Id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            identifiers.iter().map(String::as_str).collect::<Vec<_>>()
        );
        let expected = Value::Array(result.objects.clone());
        let mut bytes = Vec::new();
        write_inspect_json(
            InspectAllResult::Complete(result),
            &mut Output::plain(&mut bytes),
        )
        .unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), expected);
    }

    #[test]
    fn empty_resources_render_one_empty_json_array_without_inspect() {
        for resource in [
            InspectResource::Container,
            InspectResource::Image,
            InspectResource::Volume,
            InspectResource::Network,
        ] {
            let executor = RecordingExecutor::default();
            let result = inspect_all(resource, &cwd(), &executor).unwrap();
            let mut bytes = Vec::new();
            assert_eq!(
                write_inspect_json(result, &mut Output::plain(&mut bytes)).unwrap(),
                AppOutcome::Success
            );
            assert_eq!(bytes, b"[]\n");
            assert_eq!(executor.capture_requests().len(), 1);
        }
    }

    #[test]
    fn enumeration_failure_propagates_for_every_resource_without_inspect_or_output() {
        for resource in [
            InspectResource::Container,
            InspectResource::Image,
            InspectResource::Volume,
            InspectResource::Network,
        ] {
            let executor = RecordingExecutor::default();
            executor.push_capture(b"ignored".to_vec(), 28);
            let result = inspect_all(resource, &cwd(), &executor).unwrap();
            let mut bytes = Vec::new();
            assert_eq!(
                write_inspect_json(result, &mut Output::plain(&mut bytes)).unwrap(),
                AppOutcome::Child(ProcessStatus::from_code(28))
            );
            assert!(bytes.is_empty());
            assert_eq!(executor.capture_requests().len(), 1);
        }
    }

    #[test]
    fn inspect_failure_stops_remaining_batches_and_emits_no_partial_json() {
        let identifiers = (0..(TARGET_BATCH_SIZE * 2) + 1)
            .map(|index| format!("id{index}"))
            .collect::<Vec<_>>();
        let executor = RecordingExecutor::default();
        executor.push_capture(format!("{}\n", identifiers.join("\n")), 0);
        executor.push_capture(br#"[{"Id":"partial"}]"#.to_vec(), 0);
        executor.push_capture(b"daemon failure".to_vec(), 41);
        let result = inspect_all(InspectResource::Container, &cwd(), &executor).unwrap();
        let mut bytes = Vec::new();
        assert_eq!(
            write_inspect_json(result, &mut Output::plain(&mut bytes)).unwrap(),
            AppOutcome::Child(ProcessStatus::from_code(41))
        );
        assert!(bytes.is_empty());
        assert_eq!(executor.capture_requests().len(), 3);
    }

    #[test]
    fn malformed_inspect_json_and_spawn_errors_are_typed_and_emit_nothing() {
        let executor = RecordingExecutor::default();
        executor.push_capture(b"id\n".to_vec(), 0);
        executor.push_capture(b"not json".to_vec(), 0);
        let error = inspect_all(InspectResource::Container, &cwd(), &executor).unwrap_err();
        assert!(matches!(error, AppError::DockerOutput(_)));

        let executor = RecordingExecutor::default();
        executor.push_capture_spawn_error(io::ErrorKind::NotFound, "docker missing");
        assert!(matches!(
            inspect_all(InspectResource::Container, &cwd(), &executor).unwrap_err(),
            AppError::Process(_)
        ));

        let executor = RecordingExecutor::default();
        executor.push_capture(b"id\n".to_vec(), 0);
        executor.push_capture_spawn_error(io::ErrorKind::PermissionDenied, "inspect denied");
        assert!(matches!(
            inspect_all(InspectResource::Container, &cwd(), &executor).unwrap_err(),
            AppError::Process(_)
        ));
    }

    #[test]
    fn aggregate_output_is_one_semantic_pretty_json_array_with_newline() {
        let executor = RecordingExecutor::default();
        executor.push_capture(b"a\nb\nc\n".to_vec(), 0);
        executor.push_capture(br#"[{"Id":"a"},{"Id":"b"},{"Id":"c"}]"#.to_vec(), 0);
        let result = inspect_all(InspectResource::Network, &cwd(), &executor).unwrap();
        let mut bytes = Vec::new();
        write_inspect_json(result, &mut Output::new(&mut bytes, ColorPolicy::enabled())).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            json!([{"Id":"a"},{"Id":"b"},{"Id":"c"}])
        );
        assert_eq!(bytes.last(), Some(&b'\n'));
        assert!(!bytes.contains(&b'\x1b'));
    }
}
