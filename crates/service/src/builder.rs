use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, RawQuery},
    http::{Method, StatusCode, Uri},
    response::IntoResponse,
    routing::get,
};
use serde_json::Value as JsonValue;

use crate::schema_view::Schema;
use odata_routing::{KEY_SEGMENT, ODataRouterExt};
use odata_url::QueryOptions;

use super::config::{ContainedNavConfig, EntitySetConfig};
use super::context::{
    CollectionContext, ContainedCollectionContext, ContainedEntityContext, EntityContext,
};

// TODO: surface parse errors as 400 instead of silently falling back to
// `QueryOptions::default()`. A malformed query string (e.g. `?$top=2$skip=1`
// missing the `&`) currently looks identical to "no query options" from the
// handler's perspective. See TODO/surface-query-parse-errors.md.
fn parse_query(raw: Option<String>) -> QueryOptions {
    QueryOptions::parse(raw.as_deref().unwrap_or("")).unwrap_or_default()
}

fn body_of(body: Option<Json<JsonValue>>) -> Option<JsonValue> {
    body.map(|Json(v)| v)
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builds an axum `Router` from an EDM schema plus per-entity-set handler
/// registrations.
///
/// The builder is generic over an application state type `S`. The default,
/// `()`, is used by stateless services. Use [`Self::with_state`] to attach a
/// state value (typically a clonable resource handle like `Arc<SqlitePool>`)
/// that every handler will receive as its second argument.
///
/// `with_state` is only available on `ODataServiceBuilder<()>`, so it must be
/// called before any `entity_set` registration. After it, the builder type
/// becomes `ODataServiceBuilder<S>` and handler signatures must match
/// `Fn(Context, S) -> Future`.
pub struct ODataServiceBuilder<S = ()> {
    schema: Arc<Schema>,
    state: S,
    configs: HashMap<String, EntitySetConfig<S>>,
}

impl ODataServiceBuilder<()> {
    /// Build a stateless service from a resolved EDM model. The router
    /// derives its internal view by walking the model's entity container —
    /// see `crate::schema_view`. The caller retains ownership of the model;
    /// only the router-relevant slice is copied.
    pub fn new(model: &csdl_edm::edm::Model) -> Self {
        Self {
            schema: Arc::new(Schema::from_model(model)),
            state: (),
            configs: HashMap::new(),
        }
    }

    /// Parse, resolve, and project CSDL XML in one shot. Convenience for
    /// services whose schema is a static CSDL document.
    ///
    /// Equivalent to:
    /// ```ignore
    /// let document = csdl_edm::parser::from_xml_reader(csdl.as_bytes())?;
    /// let edmx = document.edmx.ok_or(...)?;
    /// let document_model = csdl_edm::resolver::Resolver::resolve_document(edmx)?;
    /// let model = document_model.schemas.first()?.clone();
    /// ODataServiceBuilder::new(&model)
    /// ```
    pub fn from_csdl(csdl: &str) -> crate::Result<Self> {
        use csdl_edm::resolver::Resolver;

        let document = csdl_edm::parser::from_xml_reader(csdl.as_bytes())
            .map_err(|err| crate::Error::Csdl(format!("failed to parse CSDL: {err}")))?;
        let edmx = document
            .edmx
            .ok_or_else(|| crate::Error::Csdl("CSDL document has no <edmx:Edmx> root".to_string()))?;
        let document_model = Resolver::resolve_document(edmx).map_err(|err| {
            crate::Error::Csdl(format!("failed to resolve CSDL: {err:?}"))
        })?;
        let model = document_model
            .schemas
            .first()
            .cloned()
            .ok_or_else(|| crate::Error::Csdl("CSDL document has no <Schema>".to_string()))?;
        Ok(Self::new(&model))
    }

    /// Attach a state value that every handler will receive as its second
    /// argument. Returns a new `ODataServiceBuilder<S>`.
    ///
    /// Only callable on an `ODataServiceBuilder<()>` — i.e. before any
    /// `entity_set` registration. This is enforced at the type level: once
    /// you've registered a handler with shape `Fn(Context) -> Fut`, the
    /// builder type is fixed at `<()>` and the state-shape change cannot
    /// retroactively apply to it.
    pub fn with_state<S>(self, state: S) -> ODataServiceBuilder<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        ODataServiceBuilder {
            schema: self.schema,
            state,
            configs: HashMap::new(),
        }
    }
}

impl<S> ODataServiceBuilder<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Register handlers for an entity set.
    ///
    /// Panics at call time if `name` is not an entity set in the schema.
    pub fn entity_set(
        mut self,
        name: &str,
        f: impl FnOnce(EntitySetConfig<S>) -> EntitySetConfig<S>,
    ) -> Self {
        assert!(
            self.schema.entity_set(name).is_some(),
            "[odata-rs] ERROR: entity set '{}' is not defined in the EDM schema",
            name
        );
        self.configs
            .insert(name.to_string(), f(EntitySetConfig::default()));
        self
    }

    /// Render a stub-handler source file for the schema this builder was
    /// constructed with. Development aid; see [`crate::scaffold`].
    pub fn scaffold(&self) -> String {
        crate::scaffold::render(&self.schema)
    }

    /// Validate registrations against the schema, warn on gaps, then build the
    /// axum `Router`.
    ///
    /// Gap warnings (unregistered handlers — every such route falls through to
    /// `501 Not Implemented`) are emitted via `tracing::warn!` at build time.
    /// They are a development aid: install a `tracing` subscriber (e.g.
    /// `tracing_subscriber::fmt()`) **before** calling `build()` to see them.
    /// Without a subscriber installed they are silently dropped, matching the
    /// rest of the `tracing` ecosystem.
    pub fn build(self) -> Router {
        self.validate_contained_nav_props();
        self.warn_gaps();
        self.assemble_router()
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    fn validate_contained_nav_props(&self) {
        for (es_name, config) in &self.configs {
            let es = self.schema.entity_set(es_name).unwrap();
            let Some(et) = self.schema.entity_type(&es.entity_type_name) else {
                continue;
            };
            for nav_name in config.contained.keys() {
                assert!(
                    et.contained_nav_props().any(|n| n.name == *nav_name),
                    "[odata-rs] ERROR: '{}' is not a contained navigation property on entity type '{}'",
                    nav_name,
                    et.name
                );
            }
        }
    }

    fn warn_gaps(&self) {
        for es_name in self.unimplemented_entity_sets() {
            tracing::warn!(
                "entity set '{}' has no registered handlers — all operations return 501",
                es_name
            );
        }

        for (es_name, nav_name) in self.unimplemented_contained_collections() {
            tracing::warn!(
                "contained nav prop '{}/{}' has no registered handlers — all operations return 501",
                es_name,
                nav_name
            );
        }

        for es in self.schema.entity_sets() {
            if let Some(config) = self.configs.get(&es.name) {
                warn_entity_set_gaps(&es.name, config);

                if let Some(et) = self.schema.entity_type(&es.entity_type_name) {
                    for nav in et.contained_nav_props() {
                        if let Some(nav_config) = config.contained.get(&nav.name) {
                            warn_contained_gaps(&es.name, &nav.name, nav_config);
                        }
                    }
                }
            }
        }
    }

    fn unimplemented_entity_sets(&self) -> Vec<String> {
        self.schema
            .entity_sets()
            .map(|es| es.name.clone())
            .filter(|es_name| !self.configs.contains_key(es_name))
            .collect()
    }

    fn unimplemented_contained_collections(&self) -> Vec<(String, String)> {
        let mut missing = Vec::new();

        for es in self.schema.entity_sets() {
            let Some(et) = self.schema.entity_type(&es.entity_type_name) else {
                continue;
            };

            for nav in et.contained_nav_props() {
                let implemented = self
                    .configs
                    .get(&es.name)
                    .is_some_and(|cfg| cfg.contained.contains_key(&nav.name));

                if !implemented {
                    missing.push((es.name.clone(), nav.name.clone()));
                }
            }
        }

        missing
    }

    // -----------------------------------------------------------------------
    // Router assembly
    // -----------------------------------------------------------------------

    fn assemble_router(mut self) -> Router {
        let es_names: Vec<String> = self.schema.entity_sets().map(|e| e.name.clone()).collect();
        let state = self.state.clone();

        let mut router = Router::new();

        // Service document at the service root (OData JSON Format §5):
        // an enumeration of the entity sets, singletons, and function imports
        // the service exposes. Built once at construction time and cloned
        // per request — the body is small and immutable.
        let service_doc = build_service_document(&self.schema);
        router = router.route(
            "/",
            get(move || {
                let doc = service_doc.clone();
                async move { Json(doc) }
            }),
        );

        for es_name in es_names {
            let config = self.configs.remove(&es_name).unwrap_or_default();

            // --- collection: /EntitySet ---
            let collection = format!("/{es_name}");
            {
                let list = config.list.clone();
                let create = config.create.clone();
                let es = es_name.clone();
                let state_get = state.clone();
                let state_post = state.clone();
                router = router.route(
                    &collection,
                    get({
                        let list = list.clone();
                        let es = es.clone();
                        move |RawQuery(q): RawQuery| {
                            let list = list.clone();
                            let es = es.clone();
                            let s = state_get.clone();
                            async move {
                                dispatch_collection(
                                    list,
                                    CollectionContext {
                                        entity_set: es,
                                        query: parse_query(q),
                                        body: None,
                                    },
                                    s,
                                )
                                .await
                            }
                        }
                    })
                    .post({
                        let es = es.clone();
                        move |RawQuery(q): RawQuery, body: Option<Json<JsonValue>>| {
                            let create = create.clone();
                            let es = es.clone();
                            let s = state_post.clone();
                            async move {
                                dispatch_collection(
                                    create,
                                    CollectionContext {
                                        entity_set: es,
                                        query: parse_query(q),
                                        body: body_of(body),
                                    },
                                    s,
                                )
                                .await
                            }
                        }
                    }),
                );
            }

            // --- entity: /EntitySet/{id} ---
            // Dual registration: segment-style and rewrite-style (__key__).
            let entity = format!("/{es_name}/{{id}}");
            let entity_rewrite = format!("/{es_name}/{KEY_SEGMENT}/{{id}}");
            {
                let get_h = config.get.clone();
                let update = config.update.clone();
                let delete_h = config.delete.clone();
                let es = es_name.clone();
                let state_get = state.clone();
                let state_patch = state.clone();
                let state_delete = state.clone();
                let methods = get({
                        let get_h = get_h.clone();
                        let es = es.clone();
                        move |Path(id): Path<String>, RawQuery(q): RawQuery| {
                            let get_h = get_h.clone();
                            let es = es.clone();
                            let s = state_get.clone();
                            async move {
                                dispatch_entity(
                                    get_h,
                                    EntityContext {
                                        entity_set: es,
                                        key: id,
                                        query: parse_query(q),
                                        body: None,
                                    },
                                    s,
                                )
                                .await
                            }
                        }
                    })
                    .patch({
                        let es = es.clone();
                        move |Path(id): Path<String>,
                              RawQuery(q): RawQuery,
                              body: Option<Json<JsonValue>>| {
                            let update = update.clone();
                            let es = es.clone();
                            let s = state_patch.clone();
                            async move {
                                dispatch_entity(
                                    update,
                                    EntityContext {
                                        entity_set: es,
                                        key: id,
                                        query: parse_query(q),
                                        body: body_of(body),
                                    },
                                    s,
                                )
                                .await
                            }
                        }
                    })
                    .delete({
                        let es = es.clone();
                        move |Path(id): Path<String>, RawQuery(q): RawQuery| {
                            let delete_h = delete_h.clone();
                            let es = es.clone();
                            let s = state_delete.clone();
                            async move {
                                dispatch_entity(
                                    delete_h,
                                    EntityContext {
                                        entity_set: es,
                                        key: id,
                                        query: parse_query(q),
                                        body: None,
                                    },
                                    s,
                                )
                                .await
                            }
                        }
                    });
                router = router
                    .route(&entity, methods.clone())
                    .route(&entity_rewrite, methods);
            }

            // --- contained nav props ---
            let es = self.schema.entity_set(&es_name).unwrap();
            if let Some(et) = self.schema.entity_type(&es.entity_type_name) {
                let nav_names: Vec<String> =
                    et.contained_nav_props().map(|n| n.name.clone()).collect();

                for nav_name in nav_names {
                    let nav_config = config.contained.get(&nav_name).cloned().unwrap_or_default();

                    // /EntitySet/{id}/NavProp — dual registration
                    let nav_collection = format!("/{es_name}/{{id}}/{nav_name}");
                    let nav_collection_rewrite =
                        format!("/{es_name}/{KEY_SEGMENT}/{{id}}/{nav_name}");
                    {
                        let list = nav_config.list.clone();
                        let create = nav_config.create.clone();
                        let esn = es_name.clone();
                        let nav = nav_name.clone();
                        let state_get = state.clone();
                        let state_post = state.clone();
                        let methods = get({
                                let list = list.clone();
                                let esn = esn.clone();
                                let nav = nav.clone();
                                move |Path(id): Path<String>, RawQuery(q): RawQuery| {
                                    let list = list.clone();
                                    let esn = esn.clone();
                                    let nav = nav.clone();
                                    let s = state_get.clone();
                                    async move {
                                        dispatch_contained_collection(
                                            list,
                                            ContainedCollectionContext {
                                                entity_set: esn,
                                                parent_key: id,
                                                nav_prop: nav,
                                                query: parse_query(q),
                                                body: None,
                                            },
                                            s,
                                        )
                                        .await
                                    }
                                }
                            })
                            .post({
                                let esn = esn.clone();
                                let nav = nav.clone();
                                move |Path(id): Path<String>,
                                      RawQuery(q): RawQuery,
                                      body: Option<Json<JsonValue>>| {
                                    let create = create.clone();
                                    let esn = esn.clone();
                                    let nav = nav.clone();
                                    let s = state_post.clone();
                                    async move {
                                        dispatch_contained_collection(
                                            create,
                                            ContainedCollectionContext {
                                                entity_set: esn,
                                                parent_key: id,
                                                nav_prop: nav,
                                                query: parse_query(q),
                                                body: body_of(body),
                                            },
                                            s,
                                        )
                                        .await
                                    }
                                }
                            });
                        router = router
                            .route(&nav_collection, methods.clone())
                            .route(&nav_collection_rewrite, methods);
                    }

                    // /EntitySet/{id}/NavProp/{nav_id} — dual registration
                    let nav_entity = format!("/{es_name}/{{id}}/{nav_name}/{{nav_id}}");
                    let nav_entity_rewrite = format!(
                        "/{es_name}/{KEY_SEGMENT}/{{id}}/{nav_name}/{KEY_SEGMENT}/{{nav_id}}"
                    );
                    {
                        let get_h = nav_config.get.clone();
                        let update = nav_config.update.clone();
                        let delete_h = nav_config.delete.clone();
                        let esn = es_name.clone();
                        let nav = nav_name.clone();
                        let state_get = state.clone();
                        let state_patch = state.clone();
                        let state_delete = state.clone();
                        let methods = get({
                                let get_h = get_h.clone();
                                let esn = esn.clone();
                                let nav = nav.clone();
                                move |Path((id, nav_id)): Path<(String, String)>,
                                      RawQuery(q): RawQuery| {
                                    let get_h = get_h.clone();
                                    let esn = esn.clone();
                                    let nav = nav.clone();
                                    let s = state_get.clone();
                                    async move {
                                        dispatch_contained_entity(
                                            get_h,
                                            ContainedEntityContext {
                                                entity_set: esn,
                                                parent_key: id,
                                                nav_prop: nav,
                                                key: nav_id,
                                                query: parse_query(q),
                                                body: None,
                                            },
                                            s,
                                        )
                                        .await
                                    }
                                }
                            })
                            .patch({
                                let esn = esn.clone();
                                let nav = nav.clone();
                                move |Path((id, nav_id)): Path<(String, String)>,
                                      RawQuery(q): RawQuery,
                                      body: Option<Json<JsonValue>>| {
                                    let update = update.clone();
                                    let esn = esn.clone();
                                    let nav = nav.clone();
                                    let s = state_patch.clone();
                                    async move {
                                        dispatch_contained_entity(
                                            update,
                                            ContainedEntityContext {
                                                entity_set: esn,
                                                parent_key: id,
                                                nav_prop: nav,
                                                key: nav_id,
                                                query: parse_query(q),
                                                body: body_of(body),
                                            },
                                            s,
                                        )
                                        .await
                                    }
                                }
                            })
                            .delete({
                                let esn = esn.clone();
                                let nav = nav.clone();
                                move |Path((id, nav_id)): Path<(String, String)>,
                                      RawQuery(q): RawQuery| {
                                    let delete_h = delete_h.clone();
                                    let esn = esn.clone();
                                    let nav = nav.clone();
                                    let s = state_delete.clone();
                                    async move {
                                        dispatch_contained_entity(
                                            delete_h,
                                            ContainedEntityContext {
                                                entity_set: esn,
                                                parent_key: id,
                                                nav_prop: nav,
                                                key: nav_id,
                                                query: parse_query(q),
                                                body: None,
                                            },
                                            s,
                                        )
                                        .await
                                    }
                                }
                            });
                        router = router
                            .route(&nav_entity, methods.clone())
                            .route(&nav_entity_rewrite, methods);
                    }
                }
            }
        }

        // OData resource names are case-sensitive, and a missing `s`, wrong
        // casing, or a typo in the path otherwise produces a body-less 404
        // from axum's default fallback. Replace it with a message that names
        // the unmatched method + path so the client sees what went wrong.
        router.fallback(unmatched_route).with_odata_rewrite()
    }
}

/// Build the OData Service Document body (OData JSON Format §5).
///
/// Enumerates entity sets, singletons, and function imports. Today's schema
/// only carries entity sets; singletons and function imports will appear here
/// when the schema model grows. `kind` and `url` are emitted explicitly even
/// where the spec would allow defaulting, because most consumers — including
/// the .NET reference client — expect them present.
fn build_service_document(schema: &Schema) -> JsonValue {
    let entries: Vec<JsonValue> = schema
        .entity_sets()
        .map(|es| {
            serde_json::json!({
                "name": es.name,
                "kind": "EntitySet",
                "url": es.name,
            })
        })
        .collect();
    serde_json::json!({
        "@odata.context": "$metadata",
        "value": entries,
    })
}

async fn unmatched_route(method: Method, uri: Uri) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        format!(
            "no route matches {method} {path} — check resource-name casing and spelling (OData paths are case-sensitive)",
            path = uri.path(),
        ),
    )
}

// ---------------------------------------------------------------------------
// Dispatch helpers
// ---------------------------------------------------------------------------

async fn dispatch_collection<S>(
    handler: Option<super::config::CollectionHandlerFn<S>>,
    ctx: CollectionContext,
    state: S,
) -> axum::response::Response {
    match handler {
        Some(h) => h(ctx, state).await,
        None => StatusCode::NOT_IMPLEMENTED.into_response(),
    }
}

async fn dispatch_entity<S>(
    handler: Option<super::config::EntityHandlerFn<S>>,
    ctx: EntityContext,
    state: S,
) -> axum::response::Response {
    match handler {
        Some(h) => h(ctx, state).await,
        None => StatusCode::NOT_IMPLEMENTED.into_response(),
    }
}

async fn dispatch_contained_collection<S>(
    handler: Option<super::config::ContainedCollectionHandlerFn<S>>,
    ctx: ContainedCollectionContext,
    state: S,
) -> axum::response::Response {
    match handler {
        Some(h) => h(ctx, state).await,
        None => StatusCode::NOT_IMPLEMENTED.into_response(),
    }
}

async fn dispatch_contained_entity<S>(
    handler: Option<super::config::ContainedEntityHandlerFn<S>>,
    ctx: ContainedEntityContext,
    state: S,
) -> axum::response::Response {
    match handler {
        Some(h) => h(ctx, state).await,
        None => StatusCode::NOT_IMPLEMENTED.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Warning helpers
// ---------------------------------------------------------------------------

fn warn_entity_set_gaps<S>(es_name: &str, config: &EntitySetConfig<S>) {
    let ops = [
        ("GET",    format!("/{es_name}"),         config.list.is_some()),
        ("POST",   format!("/{es_name}"),         config.create.is_some()),
        ("GET",    format!("/{es_name}/{{id}}"),  config.get.is_some()),
        ("PATCH",  format!("/{es_name}/{{id}}"),  config.update.is_some()),
        ("DELETE", format!("/{es_name}/{{id}}"),  config.delete.is_some()),
    ];
    for (method, route, registered) in ops {
        if !registered {
            tracing::warn!("{method} {route} not implemented — returns 501");
        }
    }
}

fn warn_contained_gaps<S>(es_name: &str, nav_name: &str, config: &ContainedNavConfig<S>) {
    let ops = [
        ("GET",    format!("/{es_name}/{{id}}/{nav_name}"),            config.list.is_some()),
        ("POST",   format!("/{es_name}/{{id}}/{nav_name}"),            config.create.is_some()),
        ("GET",    format!("/{es_name}/{{id}}/{nav_name}/{{nav_id}}"), config.get.is_some()),
        ("PATCH",  format!("/{es_name}/{{id}}/{nav_name}/{{nav_id}}"), config.update.is_some()),
        ("DELETE", format!("/{es_name}/{{id}}/{nav_name}/{{nav_id}}"), config.delete.is_some()),
    ];
    for (method, route, registered) in ops {
        if !registered {
            tracing::warn!("{method} {route} not implemented — returns 501");
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use super::ODataServiceBuilder;

    fn minimal_router() -> axum::Router {
        use axum::{extract::Path, routing::get};
        axum::Router::new()
            .route("/Rooms", get(|| async { "collection" }))
            .route(
                "/Rooms/{id}",
                get(|Path(id): Path<String>| async move { format!("entity:{id}") }),
            )
            .route(
                "/Rooms/{id}/Printers",
                get(|Path(id): Path<String>| async move { format!("nav-collection:{id}") }),
            )
            .route(
                "/Rooms/{id}/Printers/{nav_id}",
                get(|Path((id, nav_id)): Path<(String, String)>| async move {
                    format!("nav-entity:{id}:{nav_id}")
                }),
            )
    }

    async fn status(router: axum::Router, uri: &str) -> u16 {
        router
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
            .as_u16()
    }

    #[tokio::test]
    async fn all_routes_resolve() {
        assert_eq!(status(minimal_router(), "/Rooms").await, 200);
        assert_eq!(status(minimal_router(), "/Rooms/oak-204").await, 200);
        assert_eq!(
            status(minimal_router(), "/Rooms/redw-1002/Printers").await,
            200
        );
        assert_eq!(
            status(minimal_router(), "/Rooms/redw-1002/Printers/prn-100").await,
            200
        );
    }

    #[tokio::test]
    async fn entity_route_without_nav_props() {
        use axum::{extract::Path, routing::get};
        let r = axum::Router::new()
            .route("/Rooms", get(|| async { "ok" }))
            .route(
                "/Rooms/{id}",
                get(|Path(id): Path<String>| async move { id }),
            );
        assert_eq!(
            status(r, "/Rooms/oak-204").await,
            200,
            "entity route alone broken"
        );
    }

    #[tokio::test]
    async fn entity_route_with_nav_collection_added() {
        use axum::{extract::Path, routing::get};
        let r = axum::Router::new()
            .route("/Rooms", get(|| async { "ok" }))
            .route(
                "/Rooms/{id}",
                get(|Path(id): Path<String>| async move { id.clone() }),
            )
            .route(
                "/Rooms/{id}/Printers",
                get(|Path(id): Path<String>| async move { id }),
            );
        assert_eq!(
            status(r.clone(), "/Rooms/oak-204").await,
            200,
            "entity broken after adding nav collection"
        );
        assert_eq!(
            status(r, "/Rooms/redw/Printers").await,
            200,
            "nav collection broken"
        );
    }

    /// Minimal CSDL fixture for tests. Resolves into an EDM model with one
    /// EntityContainer / EntitySet "Rooms" → "Room" and a single contained
    /// nav prop "Printers" on Room.
    const ROOMS_CSDL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
        <edmx:Edmx xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx" Version="4.0">
          <edmx:DataServices>
            <Schema Namespace="Bm" xmlns="http://docs.oasis-open.org/odata/ns/edm">
              <EntityType Name="Printer">
                <Key><PropertyRef Name="Id" /></Key>
                <Property Name="Id" Type="Edm.String" Nullable="false" />
              </EntityType>
              <EntityType Name="Room">
                <Key><PropertyRef Name="Id" /></Key>
                <Property Name="Id" Type="Edm.String" Nullable="false" />
                <NavigationProperty Name="Printers" Type="Collection(Bm.Printer)" ContainsTarget="true" />
              </EntityType>
              <EntityContainer Name="C">
                <EntitySet Name="Rooms" EntityType="Bm.Room" />
              </EntityContainer>
            </Schema>
          </edmx:DataServices>
        </edmx:Edmx>"#;

    #[test]
    fn detects_unimplemented_entity_sets_from_schema() {
        let builder = ODataServiceBuilder::from_csdl(ROOMS_CSDL).expect("csdl");
        assert_eq!(
            builder.unimplemented_entity_sets(),
            vec!["Rooms".to_string()]
        );
    }

    #[test]
    fn detects_unimplemented_contained_collections_from_schema() {
        let builder = ODataServiceBuilder::from_csdl(ROOMS_CSDL)
            .expect("csdl")
            .entity_set("Rooms", |es| es.list(|_, _: ()| async { "ok" }));

        assert_eq!(
            builder.unimplemented_contained_collections(),
            vec![("Rooms".to_string(), "Printers".to_string())]
        );
    }

    #[test]
    fn does_not_mark_registered_contained_collection_as_missing() {
        let builder = ODataServiceBuilder::from_csdl(ROOMS_CSDL)
            .expect("csdl")
            .entity_set("Rooms", |es| {
                es.contained("Printers", |nav| nav.list(|_, _: ()| async { "ok" }))
            });

        assert!(builder.unimplemented_contained_collections().is_empty());
    }

    #[tokio::test]
    async fn service_document_lists_entity_sets() {
        use axum::body::to_bytes;

        let router = ODataServiceBuilder::from_csdl(ROOMS_CSDL).expect("csdl").build();
        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(doc["@odata.context"], "$metadata");
        let entries = doc["value"].as_array().expect("value must be an array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"], "Rooms");
        assert_eq!(entries[0]["kind"], "EntitySet");
        assert_eq!(entries[0]["url"], "Rooms");
    }
}
