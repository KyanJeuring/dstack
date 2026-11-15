use std::fmt;

use super::UpdateError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    LinuxX86_64,
    LinuxAarch64,
    MacosX86_64,
    MacosAarch64,
    WindowsX86_64,
}

impl Platform {
    pub fn current() -> Result<Self, UpdateError> {
        Self::detect(std::env::consts::OS, std::env::consts::ARCH)
    }

    pub fn detect(os: &str, arch: &str) -> Result<Self, UpdateError> {
        match (os, arch) {
            ("linux", "x86_64") => Ok(Self::LinuxX86_64),
            ("linux", "aarch64") => Ok(Self::LinuxAarch64),
            ("macos", "x86_64") => Ok(Self::MacosX86_64),
            ("macos", "aarch64") => Ok(Self::MacosAarch64),
            ("windows", "x86_64") => Ok(Self::WindowsX86_64),
            _ => Err(UpdateError::UnsupportedPlatform {
                os: os.to_owned(),
                arch: arch.to_owned(),
            }),
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "linux-x86_64",
            Self::LinuxAarch64 => "linux-aarch64",
            Self::MacosX86_64 => "macos-x86_64",
            Self::MacosAarch64 => "macos-aarch64",
            Self::WindowsX86_64 => "windows-x86_64",
        }
    }

    pub const fn archive_extension(self) -> &'static str {
        match self {
            Self::WindowsX86_64 => "zip",
            Self::LinuxX86_64 | Self::LinuxAarch64 | Self::MacosX86_64 | Self::MacosAarch64 => {
                "tar.gz"
            }
        }
    }

    pub const fn binary_name(self) -> &'static str {
        match self {
            Self::WindowsX86_64 => "dstack.exe",
            Self::LinuxX86_64 | Self::LinuxAarch64 | Self::MacosX86_64 | Self::MacosAarch64 => {
                "dstack"
            }
        }
    }

    pub const fn is_linux(self) -> bool {
        matches!(self, Self::LinuxX86_64 | Self::LinuxAarch64)
    }

    pub fn archive_name(self, version: ReleaseVersion) -> String {
        format!(
            "dstack-v{version}-{}.{}",
            self.key(),
            self.archive_extension()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReleaseVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

impl ReleaseVersion {
    pub fn parse_v2(value: &str) -> Option<Self> {
        let value = value.strip_prefix('v').unwrap_or(value);
        let mut components = value.split('.');
        let major = components.next()?.parse().ok()?;
        let minor = components.next()?.parse().ok()?;
        let patch = components.next()?.parse().ok()?;
        if components.next().is_some() || major != 2 {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    pub(crate) fn running() -> Result<Self, UpdateError> {
        Self::parse_v2(env!("CARGO_PKG_VERSION"))
            .ok_or_else(|| UpdateError::InvalidRunningVersion(env!("CARGO_PKG_VERSION").to_owned()))
    }
}

impl fmt::Display for ReleaseVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
