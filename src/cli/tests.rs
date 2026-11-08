use std::{ffi::OsString, path::PathBuf};

use clap::CommandFactory;

use super::{
    Cli, Command, ContainerCommand, ContextCommand, ImageCommand, MigrationCommand, NetworkCommand,
    RegistryCommand, ShellCommand, VolumeCommand,
};

#[test]
fn parses_bare_invocation_for_application_diagnostic() {
    let cli = Cli::try_parse_from(["dstack"]).unwrap();
    assert!(cli.command.is_none());
}

#[test]
fn parses_repeated_registry_files() {
    let cli = Cli::try_parse_from([
        "dstack",
        "registry",
        "add",
        "jellyfin-prod",
        "/srv/jellyfin",
        "--file",
        "compose.yaml",
        "--file=compose.prod.yaml",
    ])
    .unwrap();
    let Some(Command::Registry(registry)) = cli.command else {
        panic!("registry command should be selected");
    };
    let RegistryCommand::Add { name, path, files } = registry.command else {
        panic!("registry add should be selected");
    };
    assert_eq!(name, "jellyfin-prod");
    assert_eq!(path, PathBuf::from("/srv/jellyfin"));
    assert_eq!(
        [cli.files, files].concat(),
        [
            PathBuf::from("compose.yaml"),
            PathBuf::from("compose.prod.yaml")
        ]
    );
}

#[test]
fn parses_migration_dry_run() {
    let cli = Cli::try_parse_from(["dstack", "migrate", "registry", "--dry-run"]).unwrap();
    let Some(Command::Migrate(migration)) = cli.command else {
        panic!("migration command should be selected");
    };
    assert!(matches!(
        migration.command,
        MigrationCommand::Registry { dry_run: true }
    ));
}

#[test]
fn global_target_and_files_parse_around_nested_context_command() {
    for argv in [
        vec![
            "dstack",
            "-s",
            "jellyfin",
            "context",
            "show",
            "--file",
            "compose.prod.yaml",
        ],
        vec![
            "dstack",
            "context",
            "show",
            "--file",
            "compose.prod.yaml",
            "-s",
            "jellyfin",
        ],
        vec![
            "dstack",
            "--file",
            "compose.prod.yaml",
            "context",
            "show",
            "-s",
            "jellyfin",
        ],
    ] {
        let cli = Cli::try_parse_from(argv).unwrap();
        let root_stack = cli.stack;
        let root_files = cli.files;
        let Some(Command::Context(context)) = cli.command else {
            panic!("context command should parse");
        };
        let ContextCommand::Show(options) = context.command;
        assert_eq!(
            root_stack.as_deref().or(options.stack.as_deref()),
            Some("jellyfin")
        );
        assert_eq!(
            [root_files, options.files].concat(),
            [PathBuf::from("compose.prod.yaml")]
        );
    }
}

#[test]
fn repeated_files_across_root_and_leaf_keep_argv_order() {
    let cli = Cli::try_parse_from([
        "dstack",
        "--file",
        "compose.yaml",
        "context",
        "show",
        "--file",
        "compose.prod.yaml",
    ])
    .unwrap();
    let root_files = cli.files;
    let Some(Command::Context(context)) = cli.command else {
        panic!("context command should parse");
    };
    let ContextCommand::Show(options) = context.command;
    assert_eq!(
        [root_files, options.files].concat(),
        [
            PathBuf::from("compose.yaml"),
            PathBuf::from("compose.prod.yaml")
        ]
    );
}

#[test]
fn raw_compose_preserves_args_and_recognizes_globals_before_boundary() {
    let cli = Cli::try_parse_from([
        "dstack",
        "compose",
        "down",
        "--file",
        "compose.prod.yaml",
        "-s",
        "jellyfin",
        "-v",
    ])
    .unwrap();

    assert_eq!(cli.stack.as_deref(), Some("jellyfin"));
    assert_eq!(cli.files, [PathBuf::from("compose.prod.yaml")]);
    let Some(Command::Compose(compose)) = cli.command else {
        panic!("compose command should parse");
    };
    assert_eq!(compose.args, ["down", "-v"].map(std::ffi::OsString::from));
}

#[test]
fn hard_boundary_makes_every_following_value_raw() {
    let cli = Cli::try_parse_from([
        "dstack",
        "-s",
        "jellyfin",
        "compose",
        "--",
        "down",
        "--file",
        "special.yaml",
        "-s",
        "raw-stack",
    ])
    .unwrap();

    assert_eq!(cli.stack.as_deref(), Some("jellyfin"));
    assert!(cli.files.is_empty());
    let Some(Command::Compose(compose)) = cli.command else {
        panic!("compose command should parse");
    };
    assert_eq!(
        compose.args,
        ["down", "--file", "special.yaml", "-s", "raw-stack"].map(std::ffi::OsString::from)
    );
}

#[test]
fn compose_short_f_is_always_raw() {
    let cli = Cli::try_parse_from(["dstack", "compose", "logs", "-f"]).unwrap();
    let Some(Command::Compose(compose)) = cli.command else {
        panic!("compose command should parse");
    };
    assert_eq!(compose.args, ["logs", "-f"].map(std::ffi::OsString::from));
}

#[test]
fn typed_operations_accept_target_options_before_or_after_the_command() {
    for argv in [
        vec![
            "dstack",
            "-s",
            "jellyfin",
            "--file",
            "compose.yaml",
            "up",
            "php",
            "nginx",
        ],
        vec![
            "dstack",
            "up",
            "php",
            "nginx",
            "--file",
            "compose.yaml",
            "-s",
            "jellyfin",
        ],
    ] {
        let cli = Cli::try_parse_from(argv).unwrap();
        let root_stack = cli.stack;
        let root_files = cli.files;
        let Some(Command::Up(arguments)) = cli.command else {
            panic!("up command should parse");
        };
        assert_eq!(
            root_stack.as_deref().or(arguments.target.stack.as_deref()),
            Some("jellyfin")
        );
        assert_eq!(
            [root_files, arguments.target.files].concat(),
            [PathBuf::from("compose.yaml")]
        );
        assert_eq!(
            arguments.services,
            ["php", "nginx"].map(std::ffi::OsString::from)
        );
    }
}

#[test]
fn service_operands_are_never_guessed_as_stack_or_command_names() {
    let cli = Cli::try_parse_from([
        "dstack",
        "-s",
        "registered",
        "restart",
        "down",
        "restart",
        "down",
    ])
    .unwrap();
    let Some(Command::Restart(arguments)) = cli.command else {
        panic!("restart command should parse");
    };
    assert_eq!(
        arguments.services,
        ["down", "restart", "down"].map(std::ffi::OsString::from)
    );
    assert!(Cli::try_parse_from(["dstack", "registered", "up"]).is_err());
    assert!(Cli::try_parse_from(["dstack", "down", "php"]).is_err());
}

#[test]
fn services_and_logs_accept_target_options_around_the_command() {
    for argv in [
        vec![
            "dstack",
            "-s",
            "jellyfin",
            "--file",
            "compose.prod.yaml",
            "services",
        ],
        vec![
            "dstack",
            "services",
            "--file",
            "compose.prod.yaml",
            "-s",
            "jellyfin",
        ],
    ] {
        let cli = Cli::try_parse_from(argv).unwrap();
        let root_stack = cli.stack;
        let root_files = cli.files;
        let Some(Command::Services(arguments)) = cli.command else {
            panic!("services command should parse");
        };
        assert_eq!(
            root_stack.as_deref().or(arguments.target.stack.as_deref()),
            Some("jellyfin")
        );
        assert_eq!(
            [root_files, arguments.target.files].concat(),
            [PathBuf::from("compose.prod.yaml")]
        );
    }

    let cli = Cli::try_parse_from([
        "dstack", "logs", "php", "worker", "php", "--tail", "25", "-f", "-s", "prod",
    ])
    .unwrap();
    let Some(Command::Logs(arguments)) = cli.command else {
        panic!("logs command should parse");
    };
    assert!(arguments.follow);
    assert_eq!(arguments.tail, 25);
    assert_eq!(arguments.target.stack.as_deref(), Some("prod"));
    assert_eq!(
        arguments.services,
        ["php", "worker", "php"].map(std::ffi::OsString::from)
    );

    for flag in ["-f", "--follow"] {
        let Some(Command::Logs(arguments)) = Cli::try_parse_from(["dstack", "logs", flag, "php"])
            .unwrap()
            .command
        else {
            panic!("logs command should parse");
        };
        assert!(arguments.follow);
        assert_eq!(arguments.tail, 100);
        assert_eq!(arguments.services, [std::ffi::OsString::from("php")]);
    }
}

#[test]
fn logs_tail_is_typed_and_rejects_negative_or_nonnumeric_values() {
    let negative = Cli::try_parse_from(["dstack", "logs", "--tail", "-1"]).unwrap_err();
    assert!(negative.to_string().contains("unexpected argument '-1'"));
    let nonnumeric = Cli::try_parse_from(["dstack", "logs", "--tail", "nope"]).unwrap_err();
    assert!(nonnumeric.to_string().contains("invalid value 'nope'"));

    let Some(Command::Logs(arguments)) = Cli::try_parse_from(["dstack", "logs", "--tail", "0"])
        .unwrap()
        .command
    else {
        panic!("logs command should parse");
    };
    assert_eq!(arguments.tail, 0);
}

#[test]
fn exec_and_run_require_a_hard_boundary_for_explicit_commands() {
    for (command, service) in [("exec", "php"), ("run", "worker")] {
        let error = Cli::try_parse_from(["dstack", command, service, "bash"]).unwrap_err();
        let rendered = error.to_string();
        assert!(rendered.contains("unexpected argument 'bash'"));
        assert!(rendered.contains(&format!("dstack {command} [OPTIONS] SERVICE -- COMMAND...")));
    }
}

#[test]
fn exec_and_run_preserve_every_argument_after_the_hard_boundary() {
    let exec = Cli::try_parse_from([
        "dstack", "exec", "app", "--", "command", "--file", "foo", "-s", "bar",
    ])
    .unwrap();
    let Some(Command::Exec(arguments)) = exec.command else {
        panic!("exec command should parse");
    };
    assert_eq!(arguments.service, std::ffi::OsString::from("app"));
    assert_eq!(
        arguments.command,
        ["command", "--file", "foo", "-s", "bar"].map(std::ffi::OsString::from)
    );

    let run = Cli::try_parse_from(["dstack", "run", "worker", "--", "--help"]).unwrap();
    let Some(Command::Run(arguments)) = run.command else {
        panic!("run command should parse");
    };
    assert_eq!(arguments.service, std::ffi::OsString::from("worker"));
    assert_eq!(arguments.command, [std::ffi::OsString::from("--help")]);
}

#[test]
fn generated_help_is_valid() {
    Cli::command().debug_assert();
}

#[test]
fn generated_version_uses_package_metadata() {
    assert_eq!(
        Cli::command().get_version(),
        Some(env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn container_namespace_parses_short_and_long_list_options_in_order() {
    for argv in [
        vec![
            "dstack",
            "container",
            "list",
            "--all",
            "--filter",
            "name=nginx",
            "--filter",
            "label=hello world",
        ],
        vec![
            "dstack",
            "container",
            "list",
            "-a",
            "-f",
            "name=nginx",
            "-f",
            "label=hello world",
        ],
    ] {
        let cli = Cli::try_parse_from(argv).unwrap();
        let Some(Command::Container(container)) = cli.command else {
            panic!("container command should parse");
        };
        let ContainerCommand::List(arguments) = container.command else {
            panic!("container list should parse");
        };
        assert!(arguments.all);
        assert_eq!(arguments.filters, ["name=nginx", "label=hello world"]);
    }

    assert!(Cli::try_parse_from(["dstack", "container", "list", "--filter", ""]).is_err());
}

#[test]
fn container_ports_ip_stats_and_inspect_are_typed() {
    for (argv, container, private_port) in [
        (vec!["dstack", "container", "ports"], None, None),
        (
            vec!["dstack", "container", "ports", "nginx"],
            Some("nginx"),
            None,
        ),
        (
            vec!["dstack", "container", "ports", "nginx", "8080/tcp"],
            Some("nginx"),
            Some("8080/tcp"),
        ),
    ] {
        let Some(Command::Container(container_command)) =
            Cli::try_parse_from(argv).unwrap().command
        else {
            panic!("container command should parse");
        };
        let ContainerCommand::Ports(arguments) = container_command.command else {
            panic!("ports command should parse");
        };
        assert_eq!(
            arguments.container.as_deref(),
            container.map(std::ffi::OsStr::new)
        );
        assert_eq!(
            arguments.private_port.as_deref(),
            private_port.map(std::ffi::OsStr::new)
        );
    }

    let Some(Command::Container(container)) = Cli::try_parse_from([
        "dstack",
        "container",
        "stats",
        "--all",
        "--no-stream",
        "nginx",
        "postgres",
    ])
    .unwrap()
    .command
    else {
        panic!("container stats should parse");
    };
    let ContainerCommand::Stats(arguments) = container.command else {
        panic!("container stats should parse");
    };
    assert!(arguments.all);
    assert!(arguments.no_stream);
    assert_eq!(
        arguments.containers,
        ["nginx", "postgres"].map(std::ffi::OsString::from)
    );

    assert!(Cli::try_parse_from(["dstack", "container", "ip"]).is_err());
    assert!(Cli::try_parse_from(["dstack", "container", "inspect"]).is_err());
}

#[test]
fn targeted_host_lifecycle_commands_require_and_preserve_operands() {
    for (argv, expected) in [
        (vec!["dstack", "container", "remove", "one"], vec!["one"]),
        (
            vec!["dstack", "container", "remove", "one", "two"],
            vec!["one", "two"],
        ),
    ] {
        let Some(Command::Container(arguments)) = Cli::try_parse_from(argv).unwrap().command else {
            panic!("container command should parse");
        };
        let ContainerCommand::Remove(arguments) = arguments.command else {
            panic!("container remove should parse");
        };
        assert_eq!(
            arguments.containers,
            expected.into_iter().map(OsString::from).collect::<Vec<_>>()
        );
    }

    for (operation, operands) in [
        ("inspect", vec!["one"]),
        ("inspect", vec!["one", "two"]),
        ("remove", vec!["one"]),
        ("remove", vec!["one", "two"]),
    ] {
        let argv = [vec!["dstack", "image", operation], operands.clone()].concat();
        let Some(Command::Image(arguments)) = Cli::try_parse_from(argv).unwrap().command else {
            panic!("image command should parse");
        };
        let images = match arguments.command {
            ImageCommand::Inspect(arguments) => arguments.images,
            ImageCommand::Remove(arguments) => arguments.images,
            ImageCommand::List(_) => panic!("targeted image command should parse"),
        };
        assert_eq!(
            images,
            operands.into_iter().map(OsString::from).collect::<Vec<_>>()
        );
    }

    for operands in [vec!["one"], vec!["one", "two"]] {
        let argv = [vec!["dstack", "network", "remove"], operands.clone()].concat();
        let Some(Command::Network(arguments)) = Cli::try_parse_from(argv).unwrap().command else {
            panic!("network command should parse");
        };
        let NetworkCommand::Remove(arguments) = arguments.command else {
            panic!("network remove should parse");
        };
        assert_eq!(
            arguments.names,
            operands.into_iter().map(OsString::from).collect::<Vec<_>>()
        );
    }

    for argv in [
        ["dstack", "container", "remove"].as_slice(),
        ["dstack", "image", "inspect"].as_slice(),
        ["dstack", "image", "remove"].as_slice(),
        ["dstack", "network", "remove"].as_slice(),
    ] {
        assert!(Cli::try_parse_from(argv).is_err());
    }
}

#[test]
fn host_stack_options_after_the_namespace_are_normalized_for_clear_rejection() {
    let cli = Cli::try_parse_from(["dstack", "container", "list", "--stack", "jellyfin"]).unwrap();
    assert_eq!(cli.stack.as_deref(), Some("jellyfin"));
    assert!(matches!(cli.command, Some(Command::Container(_))));

    let cli =
        Cli::try_parse_from(["dstack", "container", "list", "--file", "compose.yaml"]).unwrap();
    assert_eq!(cli.files, [PathBuf::from("compose.yaml")]);

    let cli = Cli::try_parse_from(["dstack", "-s", "container", "up"]).unwrap();
    assert_eq!(cli.stack.as_deref(), Some("container"));
    assert!(matches!(cli.command, Some(Command::Up(_))));
}

#[test]
fn legacy_abbreviated_helpers_are_not_rust_commands() {
    for old in [
        "dps",
        "dpsa",
        "dpsg",
        "dport",
        "dip",
        "dstats",
        "dinspect",
        "dimg",
        "dvol",
        "dvolrm",
        "dvolinspect",
        "dnet",
        "dnetinspect",
    ] {
        assert!(Cli::try_parse_from(["dstack", old]).is_err(), "{old}");
    }
}

#[test]
fn image_volume_and_network_namespaces_parse_only_approved_operations() {
    let Some(Command::Image(image)) = Cli::try_parse_from([
        "dstack",
        "image",
        "list",
        "-a",
        "-f",
        "reference=nginx*",
        "--filter",
        "dangling=false",
    ])
    .unwrap()
    .command
    else {
        panic!("image list should parse");
    };
    let ImageCommand::List(arguments) = image.command else {
        panic!("image list should parse");
    };
    assert!(arguments.all);
    assert_eq!(arguments.filters, ["reference=nginx*", "dangling=false"]);

    let Some(Command::Volume(volume)) =
        Cli::try_parse_from(["dstack", "volume", "inspect", "app_data", "cache"])
            .unwrap()
            .command
    else {
        panic!("volume inspect should parse");
    };
    let VolumeCommand::Inspect(arguments) = volume.command else {
        panic!("volume inspect should parse");
    };
    assert_eq!(
        arguments.names,
        ["app_data", "cache"].map(std::ffi::OsString::from)
    );

    let Some(Command::Network(network)) =
        Cli::try_parse_from(["dstack", "network", "list", "--filter", "driver=bridge"])
            .unwrap()
            .command
    else {
        panic!("network list should parse");
    };
    let NetworkCommand::List(arguments) = network.command else {
        panic!("network list should parse");
    };
    assert_eq!(arguments.filters, ["driver=bridge"]);

    for argv in [
        vec!["dstack", "volume", "inspect"],
        vec!["dstack", "volume", "remove"],
        vec!["dstack", "network", "inspect"],
        vec!["dstack", "volume", "prune"],
        vec!["dstack", "network", "prune"],
    ] {
        assert!(Cli::try_parse_from(argv).is_err());
    }
}

#[test]
fn session_and_shell_commands_have_explicit_typed_grammar() {
    let cli = Cli::try_parse_from([
        "dstack",
        "use",
        "jellyfin 東京",
        "--file",
        "compose.yaml",
        "--file=compose prod.yaml",
    ])
    .unwrap();
    let Some(Command::Use(arguments)) = cli.command else {
        panic!("use command should parse");
    };
    assert_eq!(arguments.name, "jellyfin 東京");
    assert_eq!(
        arguments.files,
        ["compose.yaml", "compose prod.yaml"].map(PathBuf::from)
    );

    assert!(matches!(
        Cli::try_parse_from(["dstack", "unset"]).unwrap().command,
        Some(Command::Unset)
    ));
    assert!(matches!(
        Cli::try_parse_from([
            "dstack",
            "shell",
            "install",
            "--shell",
            "fish",
            "--profile",
            "custom profile",
            "--dry-run"
        ])
        .unwrap()
        .command,
        Some(Command::Shell(super::ShellArgs {
            command: ShellCommand::Install(_)
        }))
    ));
    assert!(matches!(
        Cli::try_parse_from(["dstack", "shell", "status", "--shell", "zsh"])
            .unwrap()
            .command,
        Some(Command::Shell(super::ShellArgs {
            command: ShellCommand::Status(_)
        }))
    ));
    assert!(matches!(
        Cli::try_parse_from(["dstack", "shell", "uninstall", "--dry-run"])
            .unwrap()
            .command,
        Some(Command::Shell(super::ShellArgs {
            command: ShellCommand::Uninstall(_)
        }))
    ));
    assert!(matches!(
        Cli::try_parse_from(["dstack", "shell", "__init", "fish"])
            .unwrap()
            .command,
        Some(Command::Shell(super::ShellArgs {
            command: ShellCommand::Init { .. }
        }))
    ));
    assert!(Cli::try_parse_from(["dstack", "jellyfin"]).is_err());
    assert!(Cli::try_parse_from(["dstack", "use", "jellyfin", "compose.yaml"]).is_err());
    assert!(Cli::try_parse_from(["dstack", "shell", "init"]).is_err());
    let unknown =
        Cli::try_parse_from(["dstack", "shell", "install", "--shell", "cmd"]).unwrap_err();
    let diagnostic = unknown.to_string();
    assert!(diagnostic.contains("bash"));
    assert!(diagnostic.contains("powershell"));
}

#[test]
fn internal_shell_protocol_is_hidden_from_normal_help() {
    let mut command = Cli::command();
    let shell = command
        .find_subcommand_mut("shell")
        .expect("shell command should exist");
    let mut help = Vec::new();
    shell.write_long_help(&mut help).unwrap();
    let help = String::from_utf8(help).unwrap();
    assert!(help.contains("install"));
    assert!(help.contains("status"));
    assert!(help.contains("uninstall"));
    assert!(!help.contains("__init"));
    assert!(!help.contains("__activate"));
    assert!(!help.contains("__clear"));
}
