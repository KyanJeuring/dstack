use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioPolicy {
    Inherit,
}

/// A process request expressed without shell parsing or quoting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub current_dir: PathBuf,
    pub stdio: StdioPolicy,
}

impl ProcessSpec {
    pub fn new(program: impl Into<OsString>, current_dir: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            current_dir: current_dir.into(),
            stdio: StdioPolicy::Inherit,
        }
    }

    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<OsString>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }
}

/// Portable projection of a native child status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessStatus {
    code: Option<i32>,
}

impl ProcessStatus {
    pub const fn from_code(code: i32) -> Self {
        Self { code: Some(code) }
    }

    pub const fn without_code() -> Self {
        Self { code: None }
    }

    pub const fn success(self) -> bool {
        matches!(self.code, Some(0))
    }

    pub const fn code(self) -> Option<i32> {
        self.code
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use super::{ProcessSpec, ProcessStatus, StdioPolicy};

    #[test]
    fn process_spec_preserves_native_program_args_cwd_and_stdio() {
        let spec = ProcessSpec::new("docker", PathBuf::from("project with space/项目"))
            .with_args(["compose", "--file", "compose file.yaml", "logs", "-f"]);

        assert_eq!(spec.program, OsString::from("docker"));
        assert_eq!(
            spec.args,
            ["compose", "--file", "compose file.yaml", "logs", "-f"].map(OsString::from)
        );
        assert_eq!(spec.current_dir, PathBuf::from("project with space/项目"));
        assert_eq!(spec.stdio, StdioPolicy::Inherit);
    }

    #[test]
    fn process_status_distinguishes_zero_nonzero_and_missing_codes() {
        assert!(ProcessStatus::from_code(0).success());
        assert!(!ProcessStatus::from_code(17).success());
        assert_eq!(ProcessStatus::from_code(17).code(), Some(17));
        assert_eq!(ProcessStatus::without_code().code(), None);
    }
}
