use crate::error::AppError;

pub(super) fn merge_stack_options(
    root: Option<String>,
    command: Option<String>,
) -> Result<Option<String>, AppError> {
    match (root, command) {
        (Some(_), Some(_)) => Err(AppError::InvalidOptions(
            "--stack/-s may be supplied only once".to_owned(),
        )),
        (Some(value), None) | (None, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}
