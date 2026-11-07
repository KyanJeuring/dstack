use std::{env, ffi::OsString};

pub const SESSION_VARIABLE: &str = "DSTACK_SESSION";

#[derive(Debug, Clone, Default)]
pub struct SessionEnvironment {
    value: Option<OsString>,
}

impl SessionEnvironment {
    pub fn from_process() -> Self {
        Self {
            value: env::var_os(SESSION_VARIABLE),
        }
    }

    pub fn absent() -> Self {
        Self::default()
    }

    pub fn with_value(value: impl Into<OsString>) -> Self {
        Self {
            value: Some(value.into()),
        }
    }

    pub(crate) fn value(&self) -> Option<&std::ffi::OsStr> {
        self.value.as_deref()
    }
}
