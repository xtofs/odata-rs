pub mod builder;
pub mod error;
pub mod expr;
pub mod model;
pub mod reader;
pub mod syntactic;

// Surface re-export: consumers can write `edm::EdmModel` for the canonical
// surface API (the resolved model). The syntactic form is available as
// `edm::syntactic::EdmModel` when needed.
pub use model::EdmModel;

pub use error::{Error, Result};
