use crate::docker::{DockerOutputError, parse_rows};

use super::model::{ContainerRow, NetworkAddressRow, PortRow};

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

fn placeholder(value: &str) -> String {
    if value.is_empty() {
        "-".to_owned()
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{containers, network_addresses, ports};

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
}
