mod builder;
mod config;
mod context;
pub mod scaffold;
mod schema_view;

#[cfg(feature = "sqlx-sqlite")]
pub mod oquery;

pub use builder::ODataServiceBuilder;
pub use config::{ContainedNavConfig, EntitySetConfig};
pub use context::{
    CollectionContext, ContainedCollectionContext, ContainedEntityContext, EntityContext,
};

// Re-export so the example and other consumers can write
// `odata_service::{Error, Result}` for the common error type produced by
// `ODataServiceBuilder::from_csdl`.
pub use csdl_edm::{Error, Result};
