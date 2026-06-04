use odata_url::QueryOptions;
use serde_json::Value as JsonValue;

// There are four Contexts at the moment that convey the information
// from the URL to the handler. They follow a regular pattern: each level
// of contained navigation adds (nav_prop, parent_key), and an entity leaf
// adds its own key.
// They are implemented manually up to one level of contained navigation
// property:
//     Collection {};
//     Entity { key: String };
//     ContainedCollection { parent_key: String, nav_prop: String };
//     ContainedEntity { parent_key: String, nav_prop: String, key: String };
// If we need to support a second level of contained navigation we'd add:
//     ContainedContainedCollection { grandparent_key: String, parent_nav_prop: String, parent_key: String, nav_prop: String };
//     ContainedContainedEntity { grandparent_key: String, parent_nav_prop: String, parent_key: String, nav_prop: String, key: String };
//
// Two alternatives were considered and intentionally not adopted at this
// scale: a single dynamic context carrying `Vec<Parent>`, and a trait
// facade over a shared base struct. The pattern scales by adding structs,
// not by parameterizing existing ones — bounded by the depth the schema
// actually uses. See the architecture paper for the trade-off discussion.



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
