mod completion;
mod detection;
mod error;
mod init;
mod install;
mod integration;
mod kind;
mod mutation;
mod profile;
mod quote;

pub use completion::completion_script;
pub use detection::{
    ADAPTER_VARIABLE, DetectedShell, DetectionEvidence, DetectionFailure, DetectionSource,
    adapter_kind, detect_shell,
};
pub use error::ShellError;
pub use init::init_script;
pub use install::{
    ManagementEnvironment, integration_guidance, run_install, run_status, run_uninstall,
};
pub use integration::{
    MANAGED_BLOCK_END, MANAGED_BLOCK_START, ManagedIntegration, ManualLoaderOccurrence,
    ProfileIntegration, ProfileIntegrationAnalysis, legacy_manual_loader, managed_block,
    manual_loader,
};
pub use kind::ShellKind;
pub use mutation::{ShellMutation, render_mutation};
pub use profile::{
    InstallAction, InstallOutcome, NativePowerShellProfileQuery, PowerShellProfileQuery,
    ProfileEnvironment, UninstallAction, UninstallOutcome, install_profile, profile_analysis,
    resolve_profile_path, uninstall_profile,
};

#[cfg(test)]
mod tests;
