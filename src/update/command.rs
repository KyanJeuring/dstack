use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    cli::SelfUpdateArgs,
    output::{Output, TextStyle},
};

use super::{Platform, ReleaseVersion, UpdateBackend, UpdateError};

pub const PACKAGE_MARKER_PATH: &str = "/usr/share/dstack-compose/install-method";
pub const PACKAGE_MARKER_VALUE: &str = "system-package";

#[derive(Debug, Clone)]
pub struct UpdateEnvironment {
    pub platform: Platform,
    pub executable: PathBuf,
    pub package_marker: PathBuf,
    pub cargo_home: Option<PathBuf>,
    pub home_dir: Option<PathBuf>,
}

impl UpdateEnvironment {
    pub fn from_process() -> Result<Self, UpdateError> {
        Ok(Self {
            platform: Platform::current()?,
            executable: env::current_exe().map_err(UpdateError::CurrentExecutable)?,
            package_marker: PathBuf::from(PACKAGE_MARKER_PATH),
            cargo_home: env::var_os("CARGO_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            home_dir: dirs::home_dir(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOutcome {
    UpToDate {
        current: ReleaseVersion,
    },
    UpdateAvailable {
        current: ReleaseVersion,
        latest: ReleaseVersion,
    },
    Updated {
        from: ReleaseVersion,
        to: ReleaseVersion,
    },
    NoCompatiblePublishedRelease,
}

pub fn run(
    arguments: &SelfUpdateArgs,
    backend: &dyn UpdateBackend,
    environment: &UpdateEnvironment,
    stdout: &mut Output<'_>,
) -> Result<UpdateOutcome, UpdateError> {
    let current = ReleaseVersion::running()?;
    let Some(latest) = backend.latest_compatible(current, environment.platform)? else {
        let outcome = UpdateOutcome::NoCompatiblePublishedRelease;
        render(outcome, stdout)?;
        return Ok(outcome);
    };

    if latest <= current {
        let outcome = UpdateOutcome::UpToDate { current };
        render(outcome, stdout)?;
        return Ok(outcome);
    }

    if arguments.check {
        let outcome = UpdateOutcome::UpdateAvailable { current, latest };
        render(outcome, stdout)?;
        return Ok(outcome);
    }

    enforce_installation_ownership(environment)?;
    backend.apply(
        current,
        latest,
        environment.platform,
        &environment.executable,
    )?;
    let outcome = UpdateOutcome::Updated {
        from: current,
        to: latest,
    };
    render(outcome, stdout)?;
    Ok(outcome)
}

fn render(outcome: UpdateOutcome, stdout: &mut Output<'_>) -> Result<(), UpdateError> {
    match outcome {
        UpdateOutcome::UpToDate { current } => {
            stdout
                .write_styled(TextStyle::Success, "[OK]")
                .map_err(output_error)?;
            writeln!(stdout, " DStack {current} is up to date.").map_err(output_error)
        }
        UpdateOutcome::UpdateAvailable { current, latest } => {
            stdout
                .write_styled(TextStyle::Info, "[INFO]")
                .map_err(output_error)?;
            writeln!(stdout, " Update available: {current} -> {latest}").map_err(output_error)
        }
        UpdateOutcome::Updated { from, to } => {
            stdout
                .write_styled(TextStyle::Success, "[OK]")
                .map_err(output_error)?;
            writeln!(stdout, " Updated DStack {from} -> {to}.").map_err(output_error)?;
            writeln!(
                stdout,
                "Open a new terminal or reload your shell profile to refresh DStack's static completion definition."
            )
            .map_err(output_error)
        }
        UpdateOutcome::NoCompatiblePublishedRelease => {
            stdout
                .write_styled(TextStyle::Info, "[INFO]")
                .map_err(output_error)?;
            writeln!(
                stdout,
                " No compatible published DStack v2 release is available."
            )
            .map_err(output_error)
        }
    }
}

fn output_error(error: std::io::Error) -> UpdateError {
    UpdateError::Output(error)
}

fn enforce_installation_ownership(environment: &UpdateEnvironment) -> Result<(), UpdateError> {
    if environment.platform.is_linux()
        && paths_correspond(&environment.executable, Path::new("/usr/bin/dstack"))
        && official_package_marker(&environment.package_marker)?
    {
        return Err(UpdateError::SystemPackageManaged {
            executable: environment.executable.clone(),
        });
    }

    let cargo_home = environment.cargo_home.clone().or_else(|| {
        environment
            .home_dir
            .as_ref()
            .map(|home| home.join(".cargo"))
    });
    if let Some(cargo_home) = cargo_home {
        let cargo_binary = cargo_home
            .join("bin")
            .join(environment.platform.binary_name());
        if paths_correspond(&environment.executable, &cargo_binary) {
            return Err(UpdateError::CargoManaged {
                executable: environment.executable.clone(),
            });
        }
    }
    Ok(())
}

fn official_package_marker(path: &Path) -> Result<bool, UpdateError> {
    match fs::read_to_string(path) {
        Ok(value) => Ok(value.trim() == PACKAGE_MARKER_VALUE),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(UpdateError::OwnershipInspection {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn paths_correspond(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}
