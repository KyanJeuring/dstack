mod detection;
mod error;
mod init;
mod install;
mod kind;
mod mutation;
mod profile;
mod quote;

pub use detection::{
    ADAPTER_VARIABLE, DetectedShell, DetectionEvidence, DetectionFailure, DetectionSource,
    adapter_kind, detect_shell,
};
pub use error::ShellError;
pub use init::init_script;
pub use install::{
    ManagementEnvironment, integration_guidance, run_install, run_status, run_uninstall,
};
pub use kind::ShellKind;
pub use mutation::{ShellMutation, render_mutation};
pub use profile::{
    InstallAction, MANAGED_BLOCK_END, MANAGED_BLOCK_START, NativePowerShellProfileQuery,
    PowerShellProfileQuery, ProfileEnvironment, ProfileIntegration, UninstallAction,
    install_profile, managed_block, manual_loader, profile_status, resolve_profile_path,
    uninstall_profile,
};

#[cfg(test)]
mod tests;
