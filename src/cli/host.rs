pub(super) fn nonempty_filter(value: &str) -> Result<String, String> {
    if value.is_empty() {
        Err("filter must not be empty".to_owned())
    } else {
        Ok(value.to_owned())
    }
}
