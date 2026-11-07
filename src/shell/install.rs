use std::{io::Write, path::Path};

use crate::{
    cli::{ShellManageArgs, ShellStatusArgs},
    error::AppError,
    output::Output,
};

use super::{
    DetectedShell, DetectionEvidence, DetectionFailure, DetectionSource,
    NativePowerShellProfileQuery, PowerShellProfileQuery, ProfileEnvironment, ProfileIntegration,
    ShellError, ShellKind, adapter_kind, detect_shell, install_profile, managed_block,
    manual_loader, profile_status, quote, resolve_profile_path, uninstall_profile,
};

#[derive(Debug, Clone, Default)]
pub struct ManagementEnvironment {
    pub detection: DetectionEvidence,
    pub profiles: ProfileEnvironment,
}

impl ManagementEnvironment {
    pub fn from_process() -> Self {
        Self {
            detection: DetectionEvidence::from_process(),
            profiles: ProfileEnvironment::from_process(),
        }
    }
}

pub fn run_install(
    arguments: &ShellManageArgs,
    environment: &ManagementEnvironment,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    run_install_with_query(
        arguments,
        environment,
        &NativePowerShellProfileQuery,
        output,
    )
}

fn run_install_with_query(
    arguments: &ShellManageArgs,
    environment: &ManagementEnvironment,
    powershell: &dyn PowerShellProfileQuery,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    let detected = select_shell("install", arguments.shell, &environment.detection)?;
    let profile = resolve_profile_path(
        &detected,
        arguments.profile.as_deref(),
        &environment.profiles,
        powershell,
    )?;
    let before = profile_status(&profile, detected.kind)?;
    let action = install_profile(&profile, detected.kind, arguments.dry_run)?;

    write_shell_selection(output, &detected, arguments.shell.is_some())?;
    output.write_label_value("Profile:", &profile.display().to_string())?;
    if arguments.dry_run {
        output.write_label_value("Profile integration:", integration_label(before))?;
        let planned = match action {
            super::InstallAction::Add => "add managed block",
            super::InstallAction::Replace => "replace managed block",
            super::InstallAction::NoOp => "no change",
        };
        output.write_label_value("Would:", planned)?;
        output.write_label_line("Managed block:")?;
        writeln!(output, "{}", managed_block(detected.kind, "\n"))?;
        return Ok(());
    }

    match action {
        super::InstallAction::Add => {
            writeln!(output, "Installed DStack shell integration:")?;
            writeln!(output, "  {}", profile.display())?;
            write_reload(output, detected.kind, &profile)?;
        }
        super::InstallAction::Replace => {
            writeln!(output, "Updated DStack shell integration:")?;
            writeln!(output, "  {}", profile.display())?;
            write_reload(output, detected.kind, &profile)?;
        }
        super::InstallAction::NoOp => {
            writeln!(output, "DStack shell integration is already installed:")?;
            writeln!(output, "  {}", profile.display())?;
        }
    }
    Ok(())
}

pub fn run_status(
    arguments: &ShellStatusArgs,
    environment: &ManagementEnvironment,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    run_status_with_query(
        arguments,
        environment,
        &NativePowerShellProfileQuery,
        output,
    )
}

fn run_status_with_query(
    arguments: &ShellStatusArgs,
    environment: &ManagementEnvironment,
    powershell: &dyn PowerShellProfileQuery,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    let detected = select_shell("status", arguments.shell, &environment.detection)?;
    let profile = resolve_profile_path(
        &detected,
        arguments.profile.as_deref(),
        &environment.profiles,
        powershell,
    )?;
    let integration = profile_status(&profile, detected.kind)?;

    output.write_label_value("Shell:", detected.kind.label())?;
    output.write_label_value("Profile:", &profile.display().to_string())?;
    output.write_label_value("Profile integration:", integration_label(integration))?;
    let adapter = match adapter_kind(environment.detection.adapter_hint.as_deref()) {
        Some(Ok(kind)) if kind == detected.kind => "active".to_owned(),
        Some(Ok(kind)) => format!(
            "active for {} (not the selected {} profile)",
            kind.label(),
            detected.kind.label()
        ),
        Some(Err(())) => "unknown".to_owned(),
        None => "not active".to_owned(),
    };
    output.write_label_value("Current terminal adapter:", &adapter)?;

    match integration {
        ProfileIntegration::Missing => {
            writeln!(output, "\nRun:\n  dstack shell install")?;
        }
        ProfileIntegration::Outdated => {
            writeln!(output, "\nUpdate it with:\n  dstack shell install")?;
        }
        ProfileIntegration::Current if adapter == "not active" => {
            write_reload(output, detected.kind, &profile)?;
        }
        ProfileIntegration::Current => {}
    }
    Ok(())
}

pub fn run_uninstall(
    arguments: &ShellManageArgs,
    environment: &ManagementEnvironment,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    run_uninstall_with_query(
        arguments,
        environment,
        &NativePowerShellProfileQuery,
        output,
    )
}

fn run_uninstall_with_query(
    arguments: &ShellManageArgs,
    environment: &ManagementEnvironment,
    powershell: &dyn PowerShellProfileQuery,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    let detected = select_shell("uninstall", arguments.shell, &environment.detection)?;
    let profile = resolve_profile_path(
        &detected,
        arguments.profile.as_deref(),
        &environment.profiles,
        powershell,
    )?;
    let action = uninstall_profile(&profile, arguments.dry_run)?;

    write_shell_selection(output, &detected, arguments.shell.is_some())?;
    output.write_label_value("Profile:", &profile.display().to_string())?;
    if arguments.dry_run {
        output.write_label_value(
            "Would:",
            match action {
                super::UninstallAction::Remove => "remove managed block",
                super::UninstallAction::NoOp => "make no change (managed block is absent)",
            },
        )?;
        return Ok(());
    }

    match action {
        super::UninstallAction::Remove => {
            writeln!(
                output,
                "DStack shell integration was removed from {}.",
                profile.display()
            )?;
            writeln!(
                output,
                "\nThe currently running shell may still have the adapter loaded."
            )?;
            writeln!(output, "Open a new terminal or reload/restart the shell.")?;
        }
        super::UninstallAction::NoOp => {
            writeln!(
                output,
                "DStack shell integration was not installed in {}; nothing needed removal.",
                profile.display()
            )?;
        }
    }
    Ok(())
}

pub fn integration_guidance(environment: &ManagementEnvironment) -> String {
    let detected = match detect_shell(None, &environment.detection) {
        Ok(detected) => detected,
        Err(DetectionFailure::Unsupported(process)) => {
            return format!(
                "The detected shell process '{process}' is not supported.\n\nSet it up with an explicit shell:\n\n  dstack shell install --shell bash\n  dstack shell install --shell zsh\n  dstack shell install --shell fish\n  dstack shell install --shell powershell"
            );
        }
        Err(DetectionFailure::Unknown) => {
            return "Could not reliably detect the current shell.\n\nSet it up with an explicit shell:\n\n  dstack shell install --shell bash\n  dstack shell install --shell zsh\n  dstack shell install --shell fish\n  dstack shell install --shell powershell".to_owned();
        }
    };
    let profile = match resolve_profile_path(
        &detected,
        None,
        &environment.profiles,
        &NativePowerShellProfileQuery,
    ) {
        Ok(profile) => profile,
        Err(error) => {
            return format!(
                "Set it up automatically with an explicit profile:\n\n  dstack shell install --shell {} --profile PATH\n\nDetected shell: {}\nAutomatic profile resolution failed: {error}",
                detected.kind,
                detected.kind.label()
            );
        }
    };
    let loader = manual_loader(detected.kind, "\n")
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let reload = reload_instruction(detected.kind, &profile)
        .unwrap_or_else(|| "open a new terminal".to_owned());
    format!(
        "Set it up automatically:\n\n  dstack shell install\n\nDetected shell: {}\nProfile: {}\n\nOr add this loader manually to {}:\n\n{loader}\n\nThen reload it:\n\n  {reload}",
        detected.kind.label(),
        profile.display(),
        profile.display()
    )
}

fn select_shell(
    action: &'static str,
    explicit: Option<ShellKind>,
    evidence: &DetectionEvidence,
) -> Result<DetectedShell, ShellError> {
    detect_shell(explicit, evidence).map_err(|failure| match failure {
        DetectionFailure::Unsupported(process) => {
            ShellError::UnsupportedDetection { action, process }
        }
        DetectionFailure::Unknown => ShellError::UnknownDetection { action },
    })
}

fn write_shell_selection(
    output: &mut Output<'_>,
    detected: &DetectedShell,
    explicit: bool,
) -> std::io::Result<()> {
    let label = if explicit || detected.source == DetectionSource::Explicit {
        "Selected shell:"
    } else {
        "Detected shell:"
    };
    output.write_label_value(label, detected.kind.label())
}

fn integration_label(integration: ProfileIntegration) -> &'static str {
    match integration {
        ProfileIntegration::Missing => "not installed",
        ProfileIntegration::Current => "installed",
        ProfileIntegration::Outdated => "installed (update available)",
    }
}

fn write_reload(output: &mut Output<'_>, shell: ShellKind, profile: &Path) -> std::io::Result<()> {
    match reload_instruction(shell, profile) {
        Some(command) => writeln!(output, "\nReload this shell with:\n  {command}"),
        None => writeln!(output, "\nOpen a new {} session to load it.", shell.label()),
    }
}

fn reload_instruction(shell: ShellKind, profile: &Path) -> Option<String> {
    let path = profile.to_str()?;
    match shell {
        ShellKind::Bash => Some(format!("source {}", quote::bash(path))),
        ShellKind::Zsh => Some(format!("source {}", quote::zsh(path))),
        ShellKind::Fish => Some(format!("source {}", quote::fish(path))),
        ShellKind::Powershell => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedPowerShell(PathBuf);

    use std::path::PathBuf;

    impl PowerShellProfileQuery for FixedPowerShell {
        fn query(&self, _executable_hint: Option<&std::ffi::OsStr>) -> Result<PathBuf, ShellError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn status_distinguishes_profile_state_from_active_adapter() {
        let temp = tempfile::tempdir().unwrap();
        let profile = temp.path().join("profile with spaces");
        let environment = ManagementEnvironment {
            detection: DetectionEvidence {
                adapter_hint: Some("bash".into()),
                ..DetectionEvidence::default()
            },
            profiles: ProfileEnvironment::default(),
        };
        let install = ShellManageArgs {
            shell: Some(ShellKind::Bash),
            profile: Some(profile.clone()),
            dry_run: false,
        };
        let mut sink = Vec::new();
        run_install_with_query(
            &install,
            &environment,
            &FixedPowerShell(profile.clone()),
            &mut Output::plain(&mut sink),
        )
        .unwrap();

        let status = ShellStatusArgs {
            shell: None,
            profile: Some(profile.clone()),
        };
        let mut output = Vec::new();
        run_status_with_query(
            &status,
            &environment,
            &FixedPowerShell(PathBuf::new()),
            &mut Output::plain(&mut output),
        )
        .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Profile integration: installed"));
        assert!(output.contains("Current terminal adapter: active"));

        let inactive_environment = ManagementEnvironment {
            detection: DetectionEvidence::default(),
            profiles: ProfileEnvironment::default(),
        };
        let explicit_status = ShellStatusArgs {
            shell: Some(ShellKind::Bash),
            profile: Some(profile),
        };
        let mut inactive = Vec::new();
        run_status_with_query(
            &explicit_status,
            &inactive_environment,
            &FixedPowerShell(PathBuf::new()),
            &mut Output::plain(&mut inactive),
        )
        .unwrap();
        assert!(
            String::from_utf8(inactive)
                .unwrap()
                .contains("Current terminal adapter: not active")
        );
    }
}
