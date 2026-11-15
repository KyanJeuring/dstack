use std::path::Path;

use self_update::{ReleaseStatus, backends::github, update::Release};

use super::{BackendFailureKind, Platform, ReleaseVersion, UpdateError};

const REPOSITORY_OWNER: &str = "KyanJeuring";
const REPOSITORY_NAME: &str = "dstack";
const RELEASE_LISTING_FLOOR: &str = "0.0.0";
pub(super) const CHECKSUM_ASSET: &str = "SHA256SUMS";

pub trait UpdateBackend {
    fn latest_compatible(
        &self,
        current: ReleaseVersion,
        platform: Platform,
    ) -> Result<Option<ReleaseVersion>, UpdateError>;

    fn apply(
        &self,
        current: ReleaseVersion,
        target: ReleaseVersion,
        platform: Platform,
        executable: &Path,
    ) -> Result<(), UpdateError>;
}

#[derive(Debug, Default)]
pub struct GitHubBackend;

impl UpdateBackend for GitHubBackend {
    fn latest_compatible(
        &self,
        _current: ReleaseVersion,
        platform: Platform,
    ) -> Result<Option<ReleaseVersion>, UpdateError> {
        // `get_newer_releases` deliberately omits the configured current version. Use a
        // lower listing floor here so a published release equal to the running version is
        // still visible to the command layer and can be reported as up to date.
        let updater = base_builder(RELEASE_LISTING_FLOOR, platform)
            .build()
            .map_err(map_release_error)?;
        let releases = match updater.get_newer_releases() {
            Ok(releases) => releases,
            Err(self_update::Error::NoReleaseFound { .. }) => return Ok(None),
            Err(error) => return Err(map_release_error(error)),
        };
        Ok(latest_compatible_release(releases.all(), platform))
    }

    fn apply(
        &self,
        current: ReleaseVersion,
        target: ReleaseVersion,
        platform: Platform,
        executable: &Path,
    ) -> Result<(), UpdateError> {
        let asset_name = platform.archive_name(target);
        let mut builder = base_builder(current.to_string(), platform);
        builder
            .release_tag(format!("v{target}"))
            .bin_install_path(executable)
            .bin_path_in_archive(platform.binary_name())
            .check_install_path_writable(true)
            .checksum_from_asset(CHECKSUM_ASSET)
            .asset_matcher(move |assets| {
                assets
                    .iter()
                    .find(|asset| standalone_asset_matches(asset.name(), &asset_name))
                    .cloned()
            });
        let updater = builder.build().map_err(map_apply_error)?;
        match updater.update_extended().map_err(map_apply_error)? {
            ReleaseStatus::Updated(release) if release.version() == target.to_string() => Ok(()),
            ReleaseStatus::Updated(release) => Err(UpdateError::backend(
                BackendFailureKind::Release,
                format!(
                    "GitHub installed unexpected version {} instead of {target}",
                    release.version()
                ),
            )),
            ReleaseStatus::UpToDate => Err(UpdateError::backend(
                BackendFailureKind::Release,
                format!("GitHub did not install selected release v{target}"),
            )),
            _ => Err(UpdateError::backend(
                BackendFailureKind::Release,
                format!("GitHub returned an unknown result for selected release v{target}"),
            )),
        }
    }
}

pub(super) fn standalone_asset_matches(candidate: &str, expected: &str) -> bool {
    candidate == expected
}

pub(super) fn latest_compatible_release(
    releases: &[Release],
    platform: Platform,
) -> Option<ReleaseVersion> {
    releases
        .iter()
        .filter_map(|release| {
            let version = ReleaseVersion::parse_v2(release.version())?;
            let archive_name = platform.archive_name(version);
            release
                .assets()
                .iter()
                .any(|asset| standalone_asset_matches(asset.name(), &archive_name))
                .then_some(version)
        })
        .max()
}

fn base_builder(current: impl Into<String>, platform: Platform) -> github::UpdateBuilder {
    let mut builder = github::Update::configure();
    builder
        .repo_owner(REPOSITORY_OWNER)
        .repo_name(REPOSITORY_NAME)
        .bin_name("dstack")
        .current_version(current)
        .target(platform.key())
        .tag_prefix("v")
        .show_download_progress(false)
        .unattended();
    builder
}

fn map_release_error(error: self_update::Error) -> UpdateError {
    let kind = match error {
        self_update::Error::Transport(_)
        | self_update::Error::NotFound { .. }
        | self_update::Error::Unauthorized { .. }
        | self_update::Error::RateLimited { .. }
        | self_update::Error::HttpStatus { .. } => BackendFailureKind::Network,
        _ => BackendFailureKind::Release,
    };
    UpdateError::backend(kind, error)
}

pub(super) fn map_apply_error(error: self_update::Error) -> UpdateError {
    match error {
        self_update::Error::InstallPathNotWritable { path, .. } => {
            UpdateError::InstallPathNotWritable { path }
        }
        error @ (self_update::Error::ChecksumSourceInvalid { .. }
        | self_update::Error::ChecksumMismatch { .. }) => {
            UpdateError::backend(BackendFailureKind::Checksum, error)
        }
        error @ (self_update::Error::Zip(_)
        | self_update::Error::Internal { .. }
        | self_update::Error::ArchiveNotEnabled(_)
        | self_update::Error::CompressionNotEnabled(_)
        | self_update::Error::InvalidAssetName { .. }) => {
            UpdateError::backend(BackendFailureKind::Archive, error)
        }
        error @ (self_update::Error::Transport(_)
        | self_update::Error::NotFound { .. }
        | self_update::Error::Unauthorized { .. }
        | self_update::Error::RateLimited { .. }
        | self_update::Error::HttpStatus { .. }) => {
            UpdateError::backend(BackendFailureKind::Network, error)
        }
        self_update::Error::Io(error) => {
            // self_update annotates every non-permission install-step I/O error with this
            // documented prefix. Other I/O errors occur while staging or extracting the archive.
            let kind = if error.to_string().starts_with("installing to ") {
                BackendFailureKind::Replacement
            } else {
                BackendFailureKind::Archive
            };
            UpdateError::backend(kind, error)
        }
        error => UpdateError::backend(BackendFailureKind::Release, error),
    }
}
