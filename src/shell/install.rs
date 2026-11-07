use std::{io::Write, path::Path};

use crate::{
    cli::{ShellManageArgs, ShellStatusArgs},
    error::AppError,
    output::Output,
};

use super::{
    DetectedShell, DetectionEvidence, DetectionFailure, DetectionSource, InstallAction,
    ManagedIntegration, NativePowerShellProfileQuery, PowerShellProfileQuery, ProfileEnvironment,
    ProfileIntegration, ProfileIntegrationAnalysis, ShellError, ShellKind, UninstallAction,
    adapter_kind, detect_shell, install_profile, legacy_manual_loader, managed_block,
    manual_loader, profile_analysis, quote, resolve_profile_path, uninstall_profile,
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
    let outcome = install_profile(&profile, detected.kind, arguments.dry_run)?;
    let integration = outcome.analysis.integration();

    write_shell_selection(output, &detected, arguments.shell.is_some())?;
    output.write_label_value("Profile:", &profile.display().to_string())?;
    if arguments.dry_run {
        output.write_label_value("Profile integration:", integration_label(integration))?;
        let planned = match outcome.action {
            InstallAction::AddManaged => "add managed block",
            InstallAction::ReplaceManaged => "replace managed block",
            InstallAction::AlreadyManaged => "make no change (managed integration is current)",
            InstallAction::AlreadyManual => "make no change (already installed manually)",
            InstallAction::BlockedLegacyManual => {
                "make no change (outdated manual integration requires manual replacement)"
            }
            InstallAction::BlockedDuplicate => {
                "make no change (duplicate integration requires manual cleanup)"
            }
        };
        output.write_label_value("Would:", planned)?;
        if matches!(
            outcome.action,
            InstallAction::AddManaged | InstallAction::ReplaceManaged
        ) {
            output.write_label_line("Managed block:")?;
            writeln!(output, "{}", managed_block(detected.kind, "\n"))?;
        }
        return blocked_install_result(&profile, detected.kind, &outcome.analysis);
    }

    match outcome.action {
        InstallAction::AddManaged => {
            writeln!(output, "Installed DStack shell integration:")?;
            writeln!(output, "  {}", profile.display())?;
            write_reload(output, detected.kind, &profile)?;
        }
        InstallAction::ReplaceManaged => {
            writeln!(output, "Updated DStack shell integration:")?;
            writeln!(output, "  {}", profile.display())?;
            write_reload(output, detected.kind, &profile)?;
        }
        InstallAction::AlreadyManaged => {
            writeln!(
                output,
                "DStack shell integration is already installed (managed):"
            )?;
            writeln!(output, "  {}", profile.display())?;
        }
        InstallAction::AlreadyManual => {
            writeln!(
                output,
                "DStack shell integration is already installed manually:"
            )?;
            writeln!(output, "  {}", profile.display())?;
            writeln!(output, "No managed block was added.")?;
        }
        InstallAction::BlockedLegacyManual | InstallAction::BlockedDuplicate => {
            return blocked_install_result(&profile, detected.kind, &outcome.analysis);
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
    let analysis = profile_analysis(&profile, detected.kind)?;
    let integration = analysis.integration();

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
        ProfileIntegration::NotInstalled => {
            writeln!(output, "\nRun:\n  dstack shell install")?;
        }
        ProfileIntegration::ManagedOutdated => {
            writeln!(output, "\nUpdate it with:\n  dstack shell install")?;
        }
        ProfileIntegration::ManualLegacy => {
            write_manual_replacement(output, detected.kind, &analysis)?;
        }
        ProfileIntegration::Duplicate => {
            return Err(integration_conflict(&profile, detected.kind, &analysis).into());
        }
        ProfileIntegration::ManagedCurrent | ProfileIntegration::ManualCurrent
            if adapter == "not active" =>
        {
            write_reload(output, detected.kind, &profile)?;
        }
        ProfileIntegration::ManagedCurrent | ProfileIntegration::ManualCurrent => {}
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
    let outcome = uninstall_profile(&profile, detected.kind, arguments.dry_run)?;

    write_shell_selection(output, &detected, arguments.shell.is_some())?;
    output.write_label_value("Profile:", &profile.display().to_string())?;
    if arguments.dry_run {
        output.write_label_value(
            "Would:",
            match outcome.action {
                UninstallAction::RemoveManaged => "remove managed block",
                UninstallAction::NoManagedBlock => "make no change (managed block is absent)",
            },
        )?;
        if outcome.analysis.has_manual_integration() {
            output.write_label_value("Manual integration:", "detected; it would remain")?;
        }
        return Ok(());
    }

    match outcome.action {
        UninstallAction::RemoveManaged => {
            writeln!(
                output,
                "Removed DStack-managed shell integration from {}.",
                profile.display()
            )?;
            if outcome.analysis.has_manual_integration() {
                writeln!(
                    output,
                    "\nA manual DStack loader still remains in:\n  {}",
                    profile.display()
                )?;
                write_manual_removal(output, detected.kind, &outcome.analysis)?;
            } else {
                writeln!(
                    output,
                    "\nThe currently running shell may still have the adapter loaded."
                )?;
                writeln!(output, "Open a new terminal or reload/restart the shell.")?;
            }
        }
        UninstallAction::NoManagedBlock => {
            if outcome.analysis.has_manual_integration() {
                writeln!(
                    output,
                    "DStack shell integration is installed manually in:\n  {}",
                    profile.display()
                )?;
                writeln!(output, "\nNo DStack-managed block was removed.")?;
                write_manual_removal(output, detected.kind, &outcome.analysis)?;
            } else {
                writeln!(
                    output,
                    "DStack shell integration was not installed in {}; nothing needed removal.",
                    profile.display()
                )?;
            }
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
    let analysis = match profile_analysis(&profile, detected.kind) {
        Ok(analysis) => analysis,
        Err(error) => {
            return format!(
                "Detected shell: {}\nProfile: {}\n\nDStack could not safely inspect the profile:\n  {error}\n\nInspect it with:\n\n  dstack shell status --shell {}",
                detected.kind.label(),
                profile.display(),
                detected.kind
            );
        }
    };
    let reload = reload_instruction(detected.kind, &profile)
        .unwrap_or_else(|| "open a new terminal".to_owned());
    match analysis.integration() {
        ProfileIntegration::ManagedCurrent | ProfileIntegration::ManualCurrent => format!(
            "DStack shell integration is already installed {} in:\n  {}\n\nThe current terminal has not loaded the adapter. Reload it:\n\n  {reload}\n\nor open a new terminal.",
            if analysis.integration() == ProfileIntegration::ManualCurrent {
                "manually"
            } else {
                "as a managed block"
            },
            profile.display()
        ),
        ProfileIntegration::ManagedOutdated => format!(
            "The managed DStack shell integration in {} has an update available.\n\nUpdate it with:\n\n  dstack shell install\n\nThen reload it:\n\n  {reload}",
            profile.display()
        ),
        ProfileIntegration::ManualLegacy => format!(
            "DStack found an outdated manual shell loader in:\n  {}\n\n{}",
            profile.display(),
            manual_replacement_text(detected.kind, &analysis)
        ),
        ProfileIntegration::Duplicate => format!(
            "DStack found duplicate or conflicting shell integrations in:\n  {}\n\nDo not install another loader. Inspect the profile with:\n\n  dstack shell status --shell {}",
            profile.display(),
            detected.kind
        ),
        ProfileIntegration::NotInstalled => {
            let loader = indented_loader(&manual_loader(detected.kind, "\n"));
            format!(
                "Set it up automatically:\n\n  dstack shell install\n\nDetected shell: {}\nProfile: {}\n\nOr add this loader manually to {}:\n\n{loader}\n\nThen reload it:\n\n  {reload}",
                detected.kind.label(),
                profile.display(),
                profile.display()
            )
        }
    }
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
        ProfileIntegration::NotInstalled => "not installed",
        ProfileIntegration::ManagedCurrent => "installed (managed)",
        ProfileIntegration::ManagedOutdated => "update available (managed)",
        ProfileIntegration::ManualCurrent => "installed (manual)",
        ProfileIntegration::ManualLegacy => "outdated (manual)",
        ProfileIntegration::Duplicate => "duplicate",
    }
}

fn blocked_install_result(
    profile: &Path,
    shell: ShellKind,
    analysis: &ProfileIntegrationAnalysis,
) -> Result<(), AppError> {
    match analysis.integration() {
        ProfileIntegration::ManualLegacy | ProfileIntegration::Duplicate => {
            Err(integration_conflict(profile, shell, analysis).into())
        }
        _ => Ok(()),
    }
}

fn integration_conflict(
    profile: &Path,
    shell: ShellKind,
    analysis: &ProfileIntegrationAnalysis,
) -> ShellError {
    let reason = match analysis.integration() {
        ProfileIntegration::ManualLegacy => manual_replacement_text(shell, analysis),
        ProfileIntegration::Duplicate => {
            let managed = match analysis.managed {
                ManagedIntegration::Missing => "no managed block".to_owned(),
                ManagedIntegration::Current => "a current managed block".to_owned(),
                ManagedIntegration::Outdated => "an outdated managed block".to_owned(),
            };
            format!(
                "Duplicate DStack shell integration detected: {managed}, {} current manual loader(s), and {} outdated manual loader(s).\n\nRemove duplicate manually authored loaders or the managed block before running install again. Manual profile content is never rewritten automatically.",
                analysis.manual_current.len(),
                analysis.manual_legacy.len()
            )
        }
        _ => "the profile integration state changed unexpectedly".to_owned(),
    };
    ShellError::ProfileIntegrationConflict {
        path: profile.to_path_buf(),
        reason,
    }
}

fn manual_replacement_text(shell: ShellKind, analysis: &ProfileIntegrationAnalysis) -> String {
    let lines = analysis
        .manual_legacy
        .iter()
        .map(|occurrence| occurrence.line.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "The profile uses an older DStack shell loader at line(s) {lines}.\n\nReplace:\n{}\n\nwith:\n{}",
        indented_loader(legacy_manual_loader(shell)),
        indented_loader(&manual_loader(shell, "\n"))
    )
}

fn write_manual_replacement(
    output: &mut Output<'_>,
    shell: ShellKind,
    analysis: &ProfileIntegrationAnalysis,
) -> std::io::Result<()> {
    writeln!(output, "\n{}", manual_replacement_text(shell, analysis))
}

fn write_manual_removal(
    output: &mut Output<'_>,
    shell: ShellKind,
    analysis: &ProfileIntegrationAnalysis,
) -> std::io::Result<()> {
    writeln!(output, "\nManual profile content remains user-owned.")?;
    if !analysis.manual_current.is_empty() {
        writeln!(
            output,
            "To remove the current manual integration, delete:\n{}",
            indented_loader(&manual_loader(shell, "\n"))
        )?;
    }
    if !analysis.manual_legacy.is_empty() {
        writeln!(
            output,
            "To remove the outdated manual integration, delete:\n{}",
            indented_loader(legacy_manual_loader(shell))
        )?;
    }
    Ok(())
}

fn indented_loader(loader: &str) -> String {
    loader
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
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
