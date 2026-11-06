use std::process::ExitCode;

use crate::process::ProcessStatus;

/// Successful application completion, optionally carrying a child status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppOutcome {
    Success,
    Child(ProcessStatus),
}

impl AppOutcome {
    pub fn exit_code(self) -> ExitCode {
        match self {
            Self::Success => ExitCode::SUCCESS,
            Self::Child(status) => status
                .code()
                .filter(|code| *code != 0)
                .and_then(|code| u8::try_from(code).ok())
                .map_or_else(
                    || {
                        if status.success() {
                            ExitCode::SUCCESS
                        } else {
                            ExitCode::FAILURE
                        }
                    },
                    ExitCode::from,
                ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppOutcome;
    use crate::process::ProcessStatus;

    #[test]
    fn preserves_representable_child_codes_and_maps_signal_like_status_to_failure() {
        assert_eq!(
            AppOutcome::Success.exit_code(),
            std::process::ExitCode::SUCCESS
        );
        assert_eq!(
            AppOutcome::Child(ProcessStatus::from_code(17)).exit_code(),
            std::process::ExitCode::from(17)
        );
        assert_eq!(
            AppOutcome::Child(ProcessStatus::without_code()).exit_code(),
            std::process::ExitCode::FAILURE
        );
    }
}
