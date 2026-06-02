pub mod builder;
pub mod error;
pub mod expr;
pub mod model;
pub mod reader;
pub mod service_schema;
pub mod syntactic;

// Surface re-export: consumers can write `odata_edm::EdmModel` for the canonical
// surface API (the resolved model). The syntactic form is available as
// `odata_edm::syntactic::EdmModel` when needed.
pub use model::EdmModel;
pub use service_schema::{EntitySet, EntityType, NavigationProperty, Schema};

pub use error::{Error, Result};
