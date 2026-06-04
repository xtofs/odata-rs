mod builder;
mod config;
mod context;
pub mod scaffold;

#[cfg(feature = "sqlx-sqlite")]
pub mod oquery;

pub use builder::ODataServiceBuilder;
pub use config::{ContainedNavConfig, EntitySetConfig};
pub use context::{
    CollectionContext, ContainedCollectionContext, ContainedEntityContext, EntityContext,
};
