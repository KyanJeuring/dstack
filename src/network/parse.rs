use crate::docker::{DockerOutputError, parse_rows};

use super::model::NetworkRow;

pub(crate) fn networks(output: &[u8]) -> Result<Vec<NetworkRow>, DockerOutputError> {
    parse_rows("dstack network list", output, 4).map(|rows| {
        rows.into_iter()
            .map(|row| NetworkRow {
                id: row[0].clone(),
                name: row[1].clone(),
                driver: row[2].clone(),
                scope: row[3].clone(),
            })
            .collect()
    })
}
