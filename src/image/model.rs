#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ImageRow {
    pub(crate) repository: String,
    pub(crate) tag: String,
    pub(crate) id: String,
    pub(crate) size: String,
}
