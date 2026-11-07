use crate::docker::{DockerOutputError, parse_rows};

use super::model::VolumeRow;

pub(crate) fn volumes(output: &[u8]) -> Result<Vec<VolumeRow>, DockerOutputError> {
    parse_rows("dstack volume list", output, 2).map(|rows| {
        rows.into_iter()
            .map(|row| VolumeRow {
                driver: row[0].clone(),
                name: row[1].clone(),
            })
            .collect()
    })
}
