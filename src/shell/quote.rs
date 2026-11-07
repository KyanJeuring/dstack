pub(super) fn bash(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) fn zsh(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) fn fish(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}'")
}

pub(super) fn powershell(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
