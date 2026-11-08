#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DesiredState {
    Running,
    Stopped,
}

impl DesiredState {
    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DesiredStateAssessment {
    AlreadySatisfied,
    NeedsCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerRuntimeState {
    Running,
    Stopped,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposeContainerState {
    pub(crate) service: String,
    pub(crate) container_id: String,
    pub(crate) runtime: ContainerRuntimeState,
}
