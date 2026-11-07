use std::ffi::{OsStr, OsString};

use clap::Args;

#[derive(Debug, Args)]
pub struct ComposeArgs {
    /// Raw Docker Compose arguments. Use `--` for a hard passthrough boundary.
    #[arg(value_name = "ARGS", num_args = 0.., allow_hyphen_values = true)]
    pub args: Vec<OsString>,
}

pub(super) fn normalize_invocation<I, T>(arguments: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let arguments = arguments.into_iter().map(Into::into).collect::<Vec<_>>();
    let Some(command_index) = find_compose_command(&arguments) else {
        return arguments;
    };

    let mut dstack_options = Vec::new();
    let mut raw = Vec::new();
    let mut index = command_index + 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        if argument == "--" {
            raw.extend(arguments[(index + 1)..].iter().cloned());
            break;
        }
        if is_owned_pair(argument) {
            dstack_options.push(argument.clone());
            index += 1;
            if let Some(value) = arguments.get(index) {
                dstack_options.push(value.clone());
                index += 1;
            }
            continue;
        }
        if is_owned_inline(argument) {
            dstack_options.push(argument.clone());
        } else {
            raw.push(argument.clone());
        }
        index += 1;
    }

    let mut normalized = arguments[..command_index].to_vec();
    normalized.extend(dstack_options);
    normalized.push(OsString::from("compose"));
    normalized.push(OsString::from("--"));
    normalized.extend(raw);
    normalized
}

fn find_compose_command(arguments: &[OsString]) -> Option<usize> {
    let mut index = 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        if argument == "compose" {
            return Some(index);
        }
        if is_owned_pair(argument) {
            index += 2;
        } else if is_owned_inline(argument) {
            index += 1;
        } else {
            return None;
        }
    }
    None
}

pub(super) fn is_owned_pair(argument: &OsStr) -> bool {
    matches!(argument.to_str(), Some("-s" | "--stack" | "--file"))
}

pub(super) fn is_owned_inline(argument: &OsStr) -> bool {
    argument.to_str().is_some_and(|argument| {
        argument.starts_with("--stack=")
            || argument.starts_with("--file=")
            || (argument.starts_with("-s") && argument.len() > 2)
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::normalize_invocation;

    #[test]
    fn moves_owned_options_before_compose_and_protects_raw_arguments() {
        assert_eq!(
            normalize_invocation([
                "dstack",
                "compose",
                "down",
                "--file",
                "compose.prod.yaml",
                "-s",
                "jellyfin",
                "-v",
            ]),
            [
                "dstack",
                "--file",
                "compose.prod.yaml",
                "-s",
                "jellyfin",
                "compose",
                "--",
                "down",
                "-v",
            ]
            .map(OsString::from)
        );
    }

    #[test]
    fn hard_boundary_keeps_every_following_argument_raw() {
        assert_eq!(
            normalize_invocation([
                "dstack",
                "-s",
                "jellyfin",
                "compose",
                "--",
                "--file",
                "special.yaml",
                "up",
            ]),
            [
                "dstack",
                "-s",
                "jellyfin",
                "compose",
                "--",
                "--file",
                "special.yaml",
                "up",
            ]
            .map(OsString::from)
        );
    }
}
