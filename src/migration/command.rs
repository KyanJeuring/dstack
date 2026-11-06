use std::io::Write;

use crate::{
    error::AppError,
    migration::{MigrationReport, migrate_registry},
    output::Output,
    registry::RegistryStore,
};

pub fn run(
    store: &RegistryStore,
    source: &std::path::Path,
    dry_run: bool,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<(), AppError> {
    let report = migrate_registry(store, source, dry_run)?;
    write_report(&report, stdout, stderr)?;
    if report.has_issues() {
        Err(AppError::MigrationIncomplete {
            skipped: report.skipped.len(),
            conflicts: report.conflicts.len(),
        })
    } else {
        Ok(())
    }
}

fn write_report(
    report: &MigrationReport,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<(), AppError> {
    stdout.write_label_value("Legacy source:", &report.source.display().to_string())?;
    stdout.write_label_value("V2 destination:", &report.destination.display().to_string())?;
    stdout.write_label_value("Mode:", if report.dry_run { "dry-run" } else { "write" })?;
    stdout.write_label_value("Discovered:", &report.discovered.to_string())?;
    stdout.write_label_value(
        "Accepted:",
        &(report.added + report.already_present).to_string(),
    )?;
    stdout.write_label_value("Added:", &report.added.to_string())?;
    stdout.write_label_value("Already present:", &report.already_present.to_string())?;
    stdout.write_label_value("Skipped:", &report.skipped.len().to_string())?;
    stdout.write_label_value("Conflicts:", &report.conflicts.len().to_string())?;
    stdout.write_label_value("Written:", if report.wrote { "yes" } else { "no" })?;

    for issue in &report.skipped {
        writeln!(
            stderr,
            "dstack: legacy line {} skipped{}: {}",
            issue.line,
            issue
                .name
                .as_deref()
                .map(|name| format!(" ('{name}')"))
                .unwrap_or_default(),
            issue.reason
        )?;
    }
    for issue in &report.conflicts {
        writeln!(
            stderr,
            "dstack: legacy line {} conflict{}: {}",
            issue.line,
            issue
                .name
                .as_deref()
                .map(|name| format!(" ('{name}')"))
                .unwrap_or_default(),
            issue.reason
        )?;
    }
    Ok(())
}
