use odata_url::QueryOptions;
use serde_json::Value as JsonValue;

/// Context passed to a collection-level handler (GET /EntitySet, POST /EntitySet).
#[derive(Debug)]
pub struct CollectionContext {
    pub entity_set: String,
    pub query: QueryOptions,
    /// JSON request body, when present (e.g. POST create).
    pub body: Option<JsonValue>,
}

/// Context passed to an entity-level handler (GET /EntitySet({key}), PATCH, DELETE).
#[derive(Debug)]
pub struct EntityContext {
    pub entity_set: String,
    /// Single string key. Composite keys are a future feature.
    pub key: String,
    pub query: QueryOptions,
    /// JSON request body, when present (e.g. PATCH update).
    pub body: Option<JsonValue>,
}

/// Context for a collection operation on a contained navigation property
/// (GET /EntitySet({parent_key})/NavProp).
#[derive(Debug)]
pub struct ContainedCollectionContext {
    pub entity_set: String,
    pub parent_key: String,
    pub nav_prop: String,
    pub query: QueryOptions,
    pub body: Option<JsonValue>,
}

/// Context for an entity operation on a contained navigation property
/// (GET /EntitySet({parent_key})/NavProp({key})).
#[derive(Debug)]
pub struct ContainedEntityContext {
    pub entity_set: String,
    pub parent_key: String,
    pub nav_prop: String,
    pub key: String,
    pub query: QueryOptions,
    pub body: Option<JsonValue>,
}
