use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use super::model::{ComposeContainerState, ContainerRuntimeState};

const SERVICE_LABEL: &str = "com.docker.compose.service";
const ONE_OFF_LABEL: &str = "com.docker.compose.oneoff";

pub(crate) fn service_names(output: &[u8]) -> Option<Vec<String>> {
    let output = std::str::from_utf8(output).ok()?;
    unique_nonempty_lines(output, |value| !value.chars().any(char::is_whitespace))
}

pub(crate) fn container_ids(output: &[u8]) -> Option<Vec<String>> {
    let output = std::str::from_utf8(output).ok()?;
    unique_nonempty_lines(output, |value| {
        (12..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn unique_nonempty_lines(output: &str, validate: impl Fn(&str) -> bool) -> Option<Vec<String>> {
    let mut seen = HashSet::new();
    let mut values = Vec::new();
    for value in output.lines() {
        if value.is_empty() || !validate(value) || !seen.insert(value.to_owned()) {
            return None;
        }
        values.push(value.to_owned());
    }
    Some(values)
}

pub(crate) fn inspected_containers(
    output: &[u8],
    expected_ids: &[String],
) -> Option<Vec<ComposeContainerState>> {
    let inspected = serde_json::from_slice::<Vec<InspectedContainer>>(output).ok()?;
    if inspected.len() != expected_ids.len() {
        return None;
    }

    let mut unmatched = expected_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut containers = Vec::new();
    for inspected in inspected {
        if !unmatched.remove(inspected.id.as_str()) {
            return None;
        }
        let labels = inspected.config.labels?;
        let service = labels.get(SERVICE_LABEL)?.to_owned();
        if service.is_empty() {
            return None;
        }
        match labels.get(ONE_OFF_LABEL)?.as_str() {
            one_off if one_off.eq_ignore_ascii_case("true") => continue,
            one_off if one_off.eq_ignore_ascii_case("false") => {}
            _ => return None,
        }
        let runtime = classify_runtime(&inspected.state);
        containers.push(ComposeContainerState {
            service,
            container_id: inspected.id,
            runtime,
        });
    }
    if unmatched.is_empty() {
        Some(containers)
    } else {
        None
    }
}

fn classify_runtime(state: &InspectedState) -> ContainerRuntimeState {
    if state.paused || state.restarting {
        return ContainerRuntimeState::Other;
    }
    if state.running && state.status == "running" {
        return ContainerRuntimeState::Running;
    }
    if !state.running && matches!(state.status.as_str(), "created" | "exited" | "dead") {
        return ContainerRuntimeState::Stopped;
    }
    ContainerRuntimeState::Other
}

#[derive(Debug, Deserialize)]
struct InspectedContainer {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Config")]
    config: InspectedConfig,
    #[serde(rename = "State")]
    state: InspectedState,
}

#[derive(Debug, Deserialize)]
struct InspectedConfig {
    #[serde(rename = "Labels")]
    labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct InspectedState {
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "Running")]
    running: bool,
    #[serde(rename = "Paused")]
    paused: bool,
    #[serde(rename = "Restarting")]
    restarting: bool,
}

#[cfg(test)]
mod tests {
    use super::{container_ids, inspected_containers, service_names};
    use crate::compose::state::model::ContainerRuntimeState;

    #[test]
    fn parses_unique_machine_lines_and_rejects_unsupported_values() {
        assert_eq!(
            service_names(b"web\r\ndb\n"),
            Some(vec!["web".to_owned(), "db".to_owned()])
        );
        assert!(service_names(b"web\nweb\n").is_none());
        assert!(service_names(b"web service\n").is_none());
        assert_eq!(
            container_ids(
                b"aaaaaaaaaaaa\nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n"
            )
            .unwrap()
            .len(),
            2
        );
        assert!(container_ids(b"not-an-id\n").is_none());
    }

    #[test]
    fn inspect_json_keeps_replicas_filters_one_offs_and_ignores_health() {
        let ids = ["aaaaaaaaaaaa", "bbbbbbbbbbbb", "cccccccccccc"].map(str::to_owned);
        let json = inspect_json(&[
            container(&ids[0], "web", "False", "running", true, false, false),
            container(&ids[1], "web", "false", "exited", false, false, false),
            container(&ids[2], "web", "True", "exited", false, false, false),
        ]);
        let states = inspected_containers(json.as_bytes(), &ids).unwrap();
        assert_eq!(states.len(), 2);
        assert_eq!(states[0].runtime, ContainerRuntimeState::Running);
        assert_eq!(states[1].runtime, ContainerRuntimeState::Stopped);
    }

    #[test]
    fn paused_restarting_and_inconsistent_states_are_other() {
        let ids = ["aaaaaaaaaaaa", "bbbbbbbbbbbb", "cccccccccccc"].map(str::to_owned);
        let json = inspect_json(&[
            container(&ids[0], "web", "False", "running", true, true, false),
            container(&ids[1], "web", "False", "restarting", true, false, true),
            container(&ids[2], "web", "False", "running", false, false, false),
        ]);
        let states = inspected_containers(json.as_bytes(), &ids).unwrap();
        assert!(
            states
                .iter()
                .all(|state| state.runtime == ContainerRuntimeState::Other)
        );
    }

    fn inspect_json(containers: &[String]) -> String {
        format!("[{}]", containers.join(","))
    }

    fn container(
        id: &str,
        service: &str,
        one_off: &str,
        status: &str,
        running: bool,
        paused: bool,
        restarting: bool,
    ) -> String {
        format!(
            r#"{{"Id":"{id}","Config":{{"Labels":{{"com.docker.compose.service":"{service}","com.docker.compose.oneoff":"{one_off}"}}}},"State":{{"Status":"{status}","Running":{running},"Paused":{paused},"Restarting":{restarting},"Health":{{"Status":"unhealthy"}}}}}}"#
        )
    }
}
