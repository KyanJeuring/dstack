use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use crate::docker::{DockerOutputError, parse_rows};

use super::model::{ContainerNetworkAddressRow, ContainerRow, NetworkAddressRow, PortRow};

pub(crate) fn containers(output: &[u8]) -> Result<Vec<ContainerRow>, DockerOutputError> {
    parse_rows("dstack container list", output, 5).map(|rows| {
        rows.into_iter()
            .map(|row| ContainerRow {
                id: row[0].clone(),
                service: placeholder(&row[1]),
                name: row[2].clone(),
                status: row[3].clone(),
                ports: placeholder(&row[4]),
            })
            .collect()
    })
}

pub(crate) fn ports(output: &[u8]) -> Result<Vec<PortRow>, DockerOutputError> {
    parse_rows("dstack container ports", output, 2).map(|rows| {
        rows.into_iter()
            .map(|row| PortRow {
                name: row[0].clone(),
                ports: placeholder(&row[1]),
            })
            .collect()
    })
}

pub(crate) fn network_addresses(
    output: &[u8],
) -> Result<Vec<NetworkAddressRow>, DockerOutputError> {
    parse_rows("dstack container ip", output, 2).map(|rows| {
        rows.into_iter()
            .map(|row| NetworkAddressRow {
                network: row[0].clone(),
                ip: placeholder(&row[1]),
            })
            .collect()
    })
}

pub(crate) fn all_network_addresses(
    objects: Vec<Value>,
    expected_ids: &[String],
) -> Result<Vec<ContainerNetworkAddressRow>, DockerOutputError> {
    const COMMAND: &str = "dstack container ip --all";

    if objects.len() != expected_ids.len() {
        return Err(unexpected_shape(
            COMMAND,
            "container inspect returned a different object count than enumeration",
        ));
    }
    let mut unmatched = expected_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut rows = Vec::new();
    for object in objects {
        let container = serde_json::from_value::<InspectedContainer>(object).map_err(|source| {
            DockerOutputError::InvalidJson {
                command: COMMAND,
                source,
            }
        })?;
        if container.id.is_empty() || !unmatched.remove(container.id.as_str()) {
            return Err(unexpected_shape(
                COMMAND,
                "container inspect returned a missing, duplicate, or unexpected container ID",
            ));
        }
        let name = container.name.strip_prefix('/').unwrap_or(&container.name);
        if name.is_empty() {
            return Err(unexpected_shape(
                COMMAND,
                "container inspect returned an empty container name",
            ));
        }
        if container.network_settings.networks.is_empty() {
            rows.push(ContainerNetworkAddressRow {
                container: name.to_owned(),
                network: "-".to_owned(),
                ip: "-".to_owned(),
            });
            continue;
        }
        for (network, endpoint) in container.network_settings.networks {
            if network.is_empty() {
                return Err(unexpected_shape(
                    COMMAND,
                    "container inspect returned an empty network name",
                ));
            }
            rows.push(ContainerNetworkAddressRow {
                container: name.to_owned(),
                network,
                ip: placeholder(&endpoint.ip_address),
            });
        }
    }
    if !unmatched.is_empty() {
        return Err(unexpected_shape(
            COMMAND,
            "container inspect omitted one or more enumerated container IDs",
        ));
    }
    rows.sort_by(|left, right| {
        left.container
            .cmp(&right.container)
            .then_with(|| left.network.cmp(&right.network))
    });
    Ok(rows)
}

fn unexpected_shape(command: &'static str, reason: &str) -> DockerOutputError {
    DockerOutputError::UnexpectedJsonShape {
        command,
        reason: reason.to_owned(),
    }
}

#[derive(Debug, Deserialize)]
struct InspectedContainer {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "NetworkSettings")]
    network_settings: InspectedNetworkSettings,
}

#[derive(Debug, Deserialize)]
struct InspectedNetworkSettings {
    #[serde(rename = "Networks")]
    networks: HashMap<String, InspectedNetworkEndpoint>,
}

#[derive(Debug, Deserialize)]
struct InspectedNetworkEndpoint {
    #[serde(rename = "IPAddress")]
    ip_address: String,
}

fn placeholder(value: &str) -> String {
    if value.is_empty() {
        "-".to_owned()
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{all_network_addresses, containers, network_addresses, ports};

    #[test]
    fn container_rows_preserve_spaces_unicode_and_fill_visual_gaps() {
        let rows = containers(
            "abc123\tweb\tjellyfin-web-1\tUp 2 hours\t0.0.0.0:8096->8096/tcp\n\
             def456\t\t服务\tExited (0) 3 days ago\t\n"
                .as_bytes(),
        )
        .unwrap();

        assert_eq!(rows[0].status, "Up 2 hours");
        assert_eq!(rows[1].service, "-");
        assert_eq!(rows[1].ports, "-");
        assert_eq!(rows[1].name, "服务");
    }

    #[test]
    fn port_and_network_rows_have_deterministic_empty_values() {
        assert_eq!(ports(b"nginx\t\n").unwrap()[0].ports, "-");
        let networks = network_addresses(b"bridge\t172.17.0.2\napp_default\t\n").unwrap();
        assert_eq!(networks[0].ip, "172.17.0.2");
        assert_eq!(networks[1].ip, "-");
    }

    #[test]
    fn all_network_rows_normalize_names_fill_gaps_and_sort_deterministically() {
        let objects = vec![
            json!({
                "Id": "redis-id",
                "Name": "/redis",
                "NetworkSettings": {"Networks": {
                    "monitoring": {"IPAddress": "172.22.0.5"},
                    "backend": {"IPAddress": "172.21.0.3"}
                }}
            }),
            json!({
                "Id": "isolated-id",
                "Name": "/isolated",
                "NetworkSettings": {"Networks": {}}
            }),
            json!({
                "Id": "db-id",
                "Name": "/db",
                "NetworkSettings": {"Networks": {
                    "backend": {"IPAddress": ""}
                }}
            }),
        ];
        let ids = ["redis-id", "isolated-id", "db-id"].map(str::to_owned);
        let rows = all_network_addresses(objects, &ids).unwrap();
        assert_eq!(
            rows.iter()
                .map(|row| (&*row.container, &*row.network, &*row.ip))
                .collect::<Vec<_>>(),
            [
                ("db", "backend", "-"),
                ("isolated", "-", "-"),
                ("redis", "backend", "172.21.0.3"),
                ("redis", "monitoring", "172.22.0.5"),
            ]
        );
    }

    #[test]
    fn all_network_rows_reject_missing_invalid_and_mismatched_shapes() {
        let ids = ["expected".to_owned()];
        for object in [
            json!({}),
            json!({"Id":"expected","Name":"/name","NetworkSettings":{}}),
            json!({"Id":"other","Name":"/name","NetworkSettings":{"Networks":{}}}),
            json!({"Id":"expected","Name":"/","NetworkSettings":{"Networks":{}}}),
            json!({"Id":"expected","Name":"/name","NetworkSettings":{"Networks":{"": {"IPAddress":"1.2.3.4"}}}}),
        ] {
            assert!(all_network_addresses(vec![object], &ids).is_err());
        }
        assert!(all_network_addresses(Vec::new(), &ids).is_err());
    }
}
