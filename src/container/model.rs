#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ContainerRow {
    pub(crate) id: String,
    pub(crate) service: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) ports: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PortRow {
    pub(crate) name: String,
    pub(crate) ports: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NetworkAddressRow {
    pub(crate) network: String,
    pub(crate) ip: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ContainerNetworkAddressRow {
    pub(crate) container: String,
    pub(crate) network: String,
    pub(crate) ip: String,
}
