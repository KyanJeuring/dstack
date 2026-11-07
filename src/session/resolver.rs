use crate::resolution::{ResolvedStack, StackSource};

use super::{SessionEnvironment, SessionError, SessionState};

pub fn resolve_active_session(
    environment: &SessionEnvironment,
) -> Result<Option<ResolvedStack>, SessionError> {
    let Some(value) = environment.value() else {
        return Ok(None);
    };
    let payload = value.to_str().ok_or(SessionError::NonUtf8Environment)?;
    let state = SessionState::parse(payload)?;
    state.validate_usable()?;
    Ok(Some(ResolvedStack {
        name: state.name,
        source: StackSource::Session,
        project: state.project,
        profile_files: state.files,
    }))
}
