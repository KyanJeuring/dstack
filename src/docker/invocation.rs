use std::{ffi::OsString, path::Path};

use crate::process::{CapturedProcessSpec, ProcessSpec};

/// Builds exact direct-Docker process requests without Compose context.
#[derive(Debug, Clone, Copy)]
pub struct DockerInvocation<'a> {
    current_dir: &'a Path,
}

impl<'a> DockerInvocation<'a> {
    pub const fn new(current_dir: &'a Path) -> Self {
        Self { current_dir }
    }

    pub fn process(
        self,
        docker_args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> ProcessSpec {
        ProcessSpec::new("docker", self.current_dir).with_args(docker_args)
    }

    pub fn capture(
        self,
        docker_args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> CapturedProcessSpec {
        CapturedProcessSpec::stdout(self.process(docker_args))
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use super::DockerInvocation;
    use crate::process::{CapturePolicy, StdioPolicy};

    #[test]
    fn direct_docker_invocation_has_no_compose_prefix_or_project_override() {
        let cwd = PathBuf::from("normal cwd/项目");
        let spec = DockerInvocation::new(&cwd).process(["container", "stats", "nginx"]);

        assert_eq!(spec.program, OsString::from("docker"));
        assert_eq!(
            spec.args,
            ["container", "stats", "nginx"].map(OsString::from)
        );
        assert_eq!(spec.current_dir, cwd);
        assert_eq!(spec.stdio, StdioPolicy::Inherit);
        assert!(!spec.args.contains(&OsString::from("compose")));
    }

    #[test]
    fn captured_docker_invocation_records_the_capture_policy() {
        let cwd = PathBuf::from("normal cwd");
        let spec = DockerInvocation::new(&cwd).capture(["image", "ls"]);

        assert_eq!(spec.process.current_dir, cwd);
        assert_eq!(spec.policy, CapturePolicy::StdoutWithInheritedStderr);
    }
}
