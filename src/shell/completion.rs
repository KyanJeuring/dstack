use clap::{Command, CommandFactory};
use clap_complete::aot::{self, Shell};

use crate::Cli;

use super::ShellKind;

const BINARY_NAME: &str = "dstack";

pub fn completion_script(shell: ShellKind) -> String {
    let mut command = completion_command();
    let mut output = Vec::new();
    aot::generate(generator(shell), &mut command, BINARY_NAME, &mut output);
    let script = String::from_utf8(output).expect("clap_complete generated non-UTF-8 shell source");
    match shell {
        ShellKind::Powershell => powershell_without_using_directives(script),
        ShellKind::Bash | ShellKind::Zsh | ShellKind::Fish => script,
    }
}

fn powershell_without_using_directives(script: String) -> String {
    script
        .replace("using namespace System.Management.Automation\n", "")
        .replace(
            "using namespace System.Management.Automation.Language\n",
            "",
        )
        .replace(
            "[StringConstantExpressionAst]",
            "[System.Management.Automation.Language.StringConstantExpressionAst]",
        )
        .replace(
            "[StringConstantType]",
            "[System.Management.Automation.Language.StringConstantType]",
        )
        .replace(
            "[CompletionResult]",
            "[System.Management.Automation.CompletionResult]",
        )
        .replace(
            "[CompletionResultType]",
            "[System.Management.Automation.CompletionResultType]",
        )
}

fn generator(shell: ShellKind) -> Shell {
    match shell {
        ShellKind::Bash => Shell::Bash,
        ShellKind::Zsh => Shell::Zsh,
        ShellKind::Fish => Shell::Fish,
        ShellKind::Powershell => Shell::PowerShell,
    }
}

fn completion_command() -> Command {
    let mut command = Cli::command();
    command.build();
    visible_command(&command)
}

fn visible_command(source: &Command) -> Command {
    let mut command = Command::new(source.get_name().to_owned())
        .disable_help_flag(true)
        .disable_help_subcommand(true)
        .disable_version_flag(true)
        .args(
            source
                .get_arguments()
                .filter(|argument| !argument.is_hide_set())
                .cloned(),
        )
        .subcommands(
            source
                .get_subcommands()
                .filter(|subcommand| !subcommand.is_hide_set() && subcommand.get_name() != "help")
                .map(visible_command),
        );

    if let Some(bin_name) = source.get_bin_name() {
        command = command.bin_name(bin_name.to_owned());
    }
    if let Some(about) = source.get_about() {
        command = command.about(about.clone());
    }
    if let Some(long_about) = source.get_long_about() {
        command = command.long_about(long_about.clone());
    }
    if let Some(version) = source.get_version() {
        command = command.version(version.to_owned());
    }
    if let Some(long_version) = source.get_long_version() {
        command = command.long_version(long_version.to_owned());
    }
    command.visible_aliases(source.get_visible_aliases().map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHELLS: [ShellKind; 4] = [
        ShellKind::Bash,
        ShellKind::Zsh,
        ShellKind::Fish,
        ShellKind::Powershell,
    ];

    #[test]
    fn all_supported_scripts_are_deterministic_and_target_dstack() {
        for shell in SHELLS {
            let first = completion_script(shell);
            let second = completion_script(shell);
            assert!(!first.is_empty(), "{shell} completion was empty");
            assert_eq!(first, second, "{shell} completion was not deterministic");
            assert!(first.contains("dstack"), "{shell} did not target dstack");
            assert!(
                !first.contains("dstack-compose"),
                "{shell} targeted the Cargo package name"
            );
        }
    }

    #[test]
    fn scripts_include_the_current_public_surface_and_exclude_protocol_commands() {
        for shell in SHELLS {
            let script = completion_script(shell);
            for public in [
                "container",
                "stop",
                "prune",
                "system",
                "df",
                "shell",
                "install",
                "status",
                "uninstall",
            ] {
                assert!(
                    script.contains(public),
                    "{shell} completion omitted {public}"
                );
            }
            for hidden in ["__init", "__activate", "__clear", "__completions"] {
                assert!(
                    !script.contains(hidden),
                    "{shell} completion exposed {hidden}"
                );
            }
        }
    }

    #[test]
    fn visible_projection_is_derived_from_the_clap_tree() {
        let command = completion_command();
        let root = command
            .get_subcommands()
            .map(Command::get_name)
            .collect::<std::collections::BTreeSet<_>>();
        let expected_root = [
            "compose",
            "config",
            "container",
            "context",
            "down",
            "exec",
            "image",
            "list",
            "logs",
            "migrate",
            "network",
            "pull",
            "rebuild",
            "recreate",
            "registry",
            "restart",
            "run",
            "services",
            "shell",
            "start",
            "stop",
            "system",
            "unset",
            "up",
            "update",
            "use",
            "volume",
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(root, expected_root);

        let container = command.find_subcommand("container").unwrap();
        let container_names = container
            .get_subcommands()
            .map(Command::get_name)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            container_names,
            [
                "inspect", "ip", "list", "ports", "prune", "remove", "stats", "stop"
            ]
            .into_iter()
            .collect()
        );

        let system = command.find_subcommand("system").unwrap();
        assert_eq!(
            system
                .get_subcommands()
                .map(Command::get_name)
                .collect::<Vec<_>>(),
            ["df", "prune"]
        );

        let shell = command.find_subcommand("shell").unwrap();
        let names = shell
            .get_subcommands()
            .map(Command::get_name)
            .collect::<Vec<_>>();
        assert_eq!(names, ["install", "status", "uninstall"]);
    }

    #[test]
    fn powershell_completion_can_follow_the_session_wrapper_in_one_payload() {
        let script = completion_script(ShellKind::Powershell);
        assert!(!script.contains("using namespace"));
        assert!(script.contains("[System.Management.Automation.CompletionResult]"));
        assert!(
            script.contains("[System.Management.Automation.Language.StringConstantExpressionAst]")
        );
    }

    #[test]
    fn fish_completion_is_static_source_for_the_shell_visible_command() {
        let script = completion_script(ShellKind::Fish);
        assert!(script.contains("complete -c dstack"));
        assert!(script.contains("__fish_dstack_needs_command"));
        assert!(!script.contains("command dstack"));
    }
}
