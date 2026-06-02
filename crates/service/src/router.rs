use crate::edm::Schema;

/// Routes OData requests against registered schemas.
pub struct Router {
    pub(super) schemas: Vec<Schema>,
}

impl Router {
    pub fn schema_count(&self) -> usize {
        self.schemas.len()
    }
}
