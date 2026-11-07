use crate::docker::{DockerOutputError, parse_rows};

use super::model::ImageRow;

pub(crate) fn images(output: &[u8]) -> Result<Vec<ImageRow>, DockerOutputError> {
    parse_rows("dstack image list", output, 4).map(|rows| {
        rows.into_iter()
            .map(|row| ImageRow {
                repository: placeholder(&row[0]),
                tag: placeholder(&row[1]),
                id: row[2].clone(),
                size: row[3].clone(),
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
