use std::{env, ffi::OsString};

use super::ShellKind;

pub const ADAPTER_VARIABLE: &str = "DSTACK_SHELL_ADAPTER";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionSource {
    Explicit,
    Adapter,
    ParentProcess,
    PlatformEnvironment,
    LoginShell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedShell {
    pub kind: ShellKind,
    pub source: DetectionSource,
    pub parent_process: Option<OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionFailure {
    Unsupported(String),
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub struct DetectionEvidence {
    pub adapter_hint: Option<OsString>,
    pub parent_process: Option<OsString>,
    pub platform_hints: Vec<OsString>,
    pub login_shell: Option<OsString>,
}

impl DetectionEvidence {
    pub fn from_process() -> Self {
        let mut platform_hints = Vec::new();
        if env::var_os("MSYSTEM").is_some() {
            platform_hints.push(OsString::from("bash"));
        }
        if env::var_os("POWERSHELL_DISTRIBUTION_CHANNEL").is_some() {
            platform_hints.push(OsString::from("powershell"));
        }
        if env::var_os("FISH_VERSION").is_some() {
            platform_hints.push(OsString::from("fish"));
        }
        if env::var_os("ZSH_VERSION").is_some() {
            platform_hints.push(OsString::from("zsh"));
        }

        Self {
            adapter_hint: env::var_os(ADAPTER_VARIABLE),
            parent_process: parent_process_name(),
            platform_hints,
            login_shell: env::var_os("SHELL"),
        }
    }
}

pub fn detect_shell(
    explicit: Option<ShellKind>,
    evidence: &DetectionEvidence,
) -> Result<DetectedShell, DetectionFailure> {
    if let Some(kind) = explicit {
        return Ok(DetectedShell {
            kind,
            source: DetectionSource::Explicit,
            parent_process: evidence.parent_process.clone(),
        });
    }

    if let Some(hint) = evidence.adapter_hint.as_deref() {
        return match classify_process_name(hint) {
            ProcessClassification::Supported(kind) => Ok(DetectedShell {
                kind,
                source: DetectionSource::Adapter,
                parent_process: evidence.parent_process.clone(),
            }),
            ProcessClassification::Unsupported(name) => Err(DetectionFailure::Unsupported(name)),
            ProcessClassification::Unknown => Err(DetectionFailure::Unknown),
        };
    }

    if let Some(parent) = evidence.parent_process.as_deref() {
        match classify_process_name(parent) {
            ProcessClassification::Supported(kind) => {
                return Ok(DetectedShell {
                    kind,
                    source: DetectionSource::ParentProcess,
                    parent_process: evidence.parent_process.clone(),
                });
            }
            ProcessClassification::Unsupported(name) => {
                return Err(DetectionFailure::Unsupported(name));
            }
            ProcessClassification::Unknown => {}
        }
    }

    let mut platform_kind = None;
    for hint in &evidence.platform_hints {
        if let ProcessClassification::Supported(kind) = classify_process_name(hint) {
            if platform_kind.is_some_and(|existing| existing != kind) {
                return Err(DetectionFailure::Unknown);
            }
            platform_kind = Some(kind);
        }
    }
    if let Some(kind) = platform_kind {
        return Ok(DetectedShell {
            kind,
            source: DetectionSource::PlatformEnvironment,
            parent_process: evidence.parent_process.clone(),
        });
    }

    if let Some(login_shell) = evidence.login_shell.as_deref() {
        return match classify_process_name(login_shell) {
            ProcessClassification::Supported(kind) => Ok(DetectedShell {
                kind,
                source: DetectionSource::LoginShell,
                parent_process: evidence.parent_process.clone(),
            }),
            ProcessClassification::Unsupported(name) => Err(DetectionFailure::Unsupported(name)),
            ProcessClassification::Unknown => Err(DetectionFailure::Unknown),
        };
    }

    Err(DetectionFailure::Unknown)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProcessClassification {
    Supported(ShellKind),
    Unsupported(String),
    Unknown,
}

fn classify_process_name(value: &std::ffi::OsStr) -> ProcessClassification {
    let Some(value) = value.to_str() else {
        return ProcessClassification::Unknown;
    };
    let basename = value
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(value)
        .trim_start_matches('-')
        .to_ascii_lowercase();
    let normalized = basename.strip_suffix(".exe").unwrap_or(&basename);
    match normalized {
        "bash" => ProcessClassification::Supported(ShellKind::Bash),
        "zsh" => ProcessClassification::Supported(ShellKind::Zsh),
        "fish" => ProcessClassification::Supported(ShellKind::Fish),
        "pwsh" | "powershell" => ProcessClassification::Supported(ShellKind::Powershell),
        "cmd" | "sh" | "dash" | "ksh" | "mksh" | "nu" | "nushell" | "tcsh" | "csh" => {
            ProcessClassification::Unsupported(basename)
        }
        _ => ProcessClassification::Unknown,
    }
}

pub fn adapter_kind(value: Option<&std::ffi::OsStr>) -> Option<Result<ShellKind, ()>> {
    value.map(|value| match classify_process_name(value) {
        ProcessClassification::Supported(kind) => Ok(kind),
        ProcessClassification::Unsupported(_) | ProcessClassification::Unknown => Err(()),
    })
}

#[cfg(unix)]
fn parent_process_name() -> Option<OsString> {
    // SAFETY: getppid has no preconditions and does not dereference memory.
    let parent = unsafe { libc::getppid() };
    if parent <= 0 {
        return None;
    }
    parent_process_name_unix(parent)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn parent_process_name_unix(parent: libc::pid_t) -> Option<OsString> {
    let value = std::fs::read_to_string(format!("/proc/{parent}/comm")).ok()?;
    let value = value.trim_end_matches(['\r', '\n']);
    (!value.is_empty()).then(|| OsString::from(value))
}

#[cfg(target_os = "macos")]
fn parent_process_name_unix(parent: libc::pid_t) -> Option<OsString> {
    use std::ffi::CStr;

    let mut buffer = vec![0_u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: buffer is writable for the supplied length and proc_pidpath writes a C string on
    // success. The parent pid came directly from getppid.
    let length = unsafe {
        libc::proc_pidpath(
            parent,
            buffer.as_mut_ptr().cast(),
            buffer.len().try_into().ok()?,
        )
    };
    if length <= 0 {
        return None;
    }
    // SAFETY: proc_pidpath returned success and guarantees a NUL-terminated path in the buffer.
    let path = unsafe { CStr::from_ptr(buffer.as_ptr().cast()) };
    std::path::Path::new(path.to_str().ok()?)
        .file_name()
        .map(|name| name.to_os_string())
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android", target_os = "macos"))
))]
fn parent_process_name_unix(_parent: libc::pid_t) -> Option<OsString> {
    None
}

#[cfg(windows)]
fn parent_process_name() -> Option<OsString> {
    use std::{mem, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::GetCurrentProcessId,
        },
    };

    // SAFETY: Toolhelp iteration follows the documented initialization and lifetime rules.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry: PROCESSENTRY32W = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
        let current = GetCurrentProcessId();
        let mut parent = None;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32ProcessID == current {
                    parent = Some(entry.th32ParentProcessID);
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        let mut result = None;
        if let Some(parent) = parent {
            entry = mem::zeroed();
            entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    if entry.th32ProcessID == parent {
                        let length = entry
                            .szExeFile
                            .iter()
                            .position(|character| *character == 0)
                            .unwrap_or(entry.szExeFile.len());
                        result = Some(OsString::from_wide(&entry.szExeFile[..length]));
                        break;
                    }
                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }
        }
        CloseHandle(snapshot);
        result
    }
}

#[cfg(not(any(unix, windows)))]
fn parent_process_name() -> Option<OsString> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(parent: &str) -> DetectionEvidence {
        DetectionEvidence {
            parent_process: Some(parent.into()),
            ..DetectionEvidence::default()
        }
    }

    #[test]
    fn adapter_hint_has_highest_automatic_precedence() {
        for (value, expected) in [
            ("bash", ShellKind::Bash),
            ("zsh", ShellKind::Zsh),
            ("fish", ShellKind::Fish),
            ("powershell", ShellKind::Powershell),
        ] {
            let input = DetectionEvidence {
                adapter_hint: Some(value.into()),
                parent_process: Some("cmd.exe".into()),
                ..DetectionEvidence::default()
            };
            assert_eq!(detect_shell(None, &input).unwrap().kind, expected);
        }
    }

    #[test]
    fn parent_process_names_are_normalized_cross_platform() {
        for (value, expected) in [
            ("bash", ShellKind::Bash),
            ("-bash", ShellKind::Bash),
            ("C:\\Program Files\\Git\\bash.exe", ShellKind::Bash),
            ("zsh", ShellKind::Zsh),
            ("fish", ShellKind::Fish),
            ("pwsh", ShellKind::Powershell),
            ("pwsh.exe", ShellKind::Powershell),
            ("powershell.exe", ShellKind::Powershell),
        ] {
            assert_eq!(detect_shell(None, &evidence(value)).unwrap().kind, expected);
        }
    }

    #[test]
    fn known_unsupported_and_unknown_parent_processes_are_distinct() {
        assert!(matches!(
            detect_shell(None, &evidence("cmd.exe")),
            Err(DetectionFailure::Unsupported(_))
        ));
        assert!(matches!(
            detect_shell(None, &evidence("terminal-emulator")),
            Err(DetectionFailure::Unknown)
        ));
    }

    #[test]
    fn login_shell_is_only_a_fallback() {
        for (value, expected) in [
            ("/bin/bash", ShellKind::Bash),
            ("/usr/bin/zsh", ShellKind::Zsh),
        ] {
            let input = DetectionEvidence {
                parent_process: Some("terminal-emulator".into()),
                login_shell: Some(value.into()),
                ..DetectionEvidence::default()
            };
            let detected = detect_shell(None, &input).unwrap();
            assert_eq!(detected.kind, expected);
            assert_eq!(detected.source, DetectionSource::LoginShell);
        }
    }

    #[test]
    fn explicit_shell_overrides_every_other_evidence() {
        let input = DetectionEvidence {
            adapter_hint: Some("bash".into()),
            parent_process: Some("cmd.exe".into()),
            login_shell: Some("/bin/zsh".into()),
            ..DetectionEvidence::default()
        };
        let detected = detect_shell(Some(ShellKind::Fish), &input).unwrap();
        assert_eq!(detected.kind, ShellKind::Fish);
        assert_eq!(detected.source, DetectionSource::Explicit);
    }
}
