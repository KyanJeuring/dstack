use std::ffi::OsString;

use crate::{context::ResolvedContext, process::ProcessSpec};

/// Builds exact Docker Compose process requests from one resolved context.
#[derive(Debug, Clone, Copy)]
pub struct ComposeInvocation<'a> {
    context: &'a ResolvedContext,
}

impl<'a> ComposeInvocation<'a> {
    pub const fn new(context: &'a ResolvedContext) -> Self {
        Self { context }
    }

    pub fn process(
        self,
        compose_args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> ProcessSpec {
        let files = self
            .context
            .compose
            .files
            .iter()
            .flat_map(|file| [OsString::from("--file"), file.as_os_str().to_os_string()]);
        let args = std::iter::once(OsString::from("compose"))
            .chain(files)
            .chain(compose_args.into_iter().map(Into::into));

        ProcessSpec::new("docker", self.context.stack.project.clone()).with_args(args)
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use super::ComposeInvocation;
    use crate::{
        compose::{ComposeSelection, ComposeSelectionSource},
        context::ResolvedContext,
        process::StdioPolicy,
        resolution::{ResolvedStack, StackSource},
    };

    fn context(project: &str, files: &[&str]) -> ResolvedContext {
        ResolvedContext {
            stack: ResolvedStack {
                name: "stack".to_owned(),
                source: StackSource::Registered,
                project: PathBuf::from(project),
                profile_files: Vec::new(),
            },
            compose: ComposeSelection {
                source: ComposeSelectionSource::Explicit,
                files: files.iter().map(PathBuf::from).collect(),
            },
        }
    }

    #[test]
    fn builds_docker_compose_prefix_files_and_operation_as_distinct_args() {
        let context = context(
            "/srv/Example User/项目",
            &[
                "/srv/Example User/项目/compose file.yaml",
                "/srv/Example User/项目/-compose.prod.yaml",
            ],
        );
        let spec = ComposeInvocation::new(&context).process(["logs", "-f"]);

        assert_eq!(spec.program, OsString::from("docker"));
        assert_eq!(
            spec.args,
            [
                "compose",
                "--file",
                "/srv/Example User/项目/compose file.yaml",
                "--file",
                "/srv/Example User/项目/-compose.prod.yaml",
                "logs",
                "-f",
            ]
            .map(OsString::from)
        );
        assert_eq!(spec.current_dir, PathBuf::from("/srv/Example User/项目"));
        assert_eq!(spec.stdio, StdioPolicy::Inherit);
    }

    #[test]
    fn supports_raw_invocation_with_no_compose_command_args() {
        let context = context(
            "C:\\Users\\Example User\\project",
            &["C:\\Users\\Example User\\project\\compose.yaml"],
        );
        let spec = ComposeInvocation::new(&context).process(Vec::<OsString>::new());

        assert_eq!(
            spec.args,
            [
                "compose",
                "--file",
                "C:\\Users\\Example User\\project\\compose.yaml",
            ]
            .map(OsString::from)
        );
    }
}
