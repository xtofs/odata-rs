use odata_url::ODataQuery;

/// Lightweight OData response envelope that backends can populate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ODataResponse<TEntity> {
    pub value: Vec<TEntity>,
    pub count: Option<u64>,
    pub next_link: Option<String>,
}

/// Backend execution contract: the service layer owns query representation,
/// while each backend owns execution.
pub trait ODataSource {
    type Entity;
    type Error;

    fn execute(&self, query: ODataQuery) -> Result<ODataResponse<Self::Entity>, Self::Error>;
}
