#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NetworkRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) driver: String,
    pub(crate) scope: String,
}
