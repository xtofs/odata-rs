//! First external integration test for the service router.
//!
//! Builds a router through the public [`ODataServiceBuilder`] API, backed by an
//! in-memory SQLite database (mirroring `examples/rooms`), and drives it with
//! `tower::ServiceExt::oneshot`. Exercises routing, context extraction,
//! contained-nav routes, and the status-code matrix.
//!
//! Gated on `sqlx-sqlite` so the file compiles to an empty test binary under
//! default features. Run with:
//!
//! ```sh
//! cargo test -p odata-rs-service --features sqlx-sqlite
//! ```
#![cfg(feature = "sqlx-sqlite")]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{FromRow, SqlitePool};
use tower::ServiceExt;

use odata_service::oquery::{
    CollectionCtxQuery, ContainedCollectionCtxQuery, ContainedEntityCtxQuery, EntityCtxQuery,
    project,
};
use odata_service::{
    CollectionContext, ContainedCollectionContext, ContainedEntityContext, EntityContext,
    ODataServiceBuilder,
};

// ---------------------------------------------------------------------------
// Fixture: CSDL, domain types, seeded in-memory database, handlers, app
// ---------------------------------------------------------------------------

const CSDL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<edmx:Edmx xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx" Version="4.0">
  <edmx:DataServices>
    <Schema Namespace="BuildingManagement" Alias="Bm"
            xmlns="http://docs.oasis-open.org/odata/ns/edm">
      <EntityType Name="printer">
        <Key><PropertyRef Name="id" /></Key>
        <Property Name="id" Type="Edm.String" Nullable="false" />
        <Property Name="model" Type="Edm.String" Nullable="false" />
      </EntityType>
      <EntityType Name="room">
        <Key><PropertyRef Name="id" /></Key>
        <Property Name="id" Type="Edm.String" Nullable="false" />
        <Property Name="name" Type="Edm.String" Nullable="false" />
        <NavigationProperty Name="printers"
                            Type="Collection(BuildingManagement.printer)"
                            ContainsTarget="true" />
      </EntityType>
      <EntityContainer Name="Container">
        <EntitySet Name="rooms" EntityType="BuildingManagement.room" />
      </EntityContainer>
    </Schema>
  </edmx:DataServices>
</edmx:Edmx>"#;

type AppState = Arc<SqlitePool>;

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct Room {
    id: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct Printer {
    id: String,
    model: String,
    // Populated by sqlx `FromRow` from the `room_id` column; never read in Rust
    // (the contained parent-key assertions read it from the dynamic JSON path).
    #[serde(skip)]
    #[sqlx(default)]
    #[allow(dead_code)]
    room_id: String,
}

static DB_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Create a fresh, uniquely-named shared-cache in-memory database, seeded with
/// two rooms and three printers. A unique name per call isolates tests that run
/// in parallel; `min_connections(1)` keeps the in-memory DB alive for the pool.
async fn seeded_pool() -> SqlitePool {
    let n = DB_COUNTER.fetch_add(1, Ordering::Relaxed);
    let opts: SqliteConnectOptions =
        format!("sqlite:file:odata_test_{n}?mode=memory&cache=shared")
            .parse()
            .expect("invalid sqlite connect string");

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .min_connections(1)
        .connect_with(opts)
        .await
        .expect("open in-memory sqlite");

    sqlx::query(
        r#"
        CREATE TABLE rooms (
            id   TEXT PRIMARY KEY,
            name TEXT NOT NULL
        );
        CREATE TABLE printers (
            id      TEXT NOT NULL,
            room_id TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
            model   TEXT NOT NULL,
            PRIMARY KEY (room_id, id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create schema");

    for (id, name) in [("redw-1002", "Redwood 1002"), ("oak-204", "Oak 204")] {
        sqlx::query("INSERT INTO rooms (id, name) VALUES (?, ?)")
            .bind(id)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }
    for (id, room, model) in [
        ("prn1002-100", "redw-1002", "HP LaserJet"),
        ("prn1002-200", "redw-1002", "Canon ImageRunner"),
        ("prn0204-100", "oak-204", "Brother HL-L6400"),
    ] {
        sqlx::query("INSERT INTO printers (id, room_id, model) VALUES (?, ?, ?)")
            .bind(id)
            .bind(room)
            .bind(model)
            .execute(&pool)
            .await
            .unwrap();
    }

    pool
}

/// Build the router. Registers `rooms` (list/get/create/delete) and contained
/// `printers` (list/get/create/delete). `update`/PATCH is intentionally left
/// unregistered so PATCH routes fall through to `501 Not Implemented`.
async fn build_app() -> Router {
    let state: AppState = Arc::new(seeded_pool().await);

    ODataServiceBuilder::from_csdl(CSDL)
        .expect("parse rooms CSDL")
        .with_state(state)
        .entity_set("rooms", |es| {
            es.list(list_rooms)
                .get(get_room)
                .create(create_room)
                .delete(delete_room)
                .contained("printers", |p| {
                    p.list(list_printers)
                        .get(get_printer)
                        .create(create_printer)
                        .delete(delete_printer)
                })
        })
        .build()
}

// ---------------------------------------------------------------------------
// Handlers (mirroring examples/rooms)
// ---------------------------------------------------------------------------

async fn list_rooms(ctx: CollectionContext, state: AppState) -> impl IntoResponse {
    let select = ctx.query.select.clone();
    match ctx.oquery::<Room>("rooms").fetch_all(&state).await {
        Ok(rooms) => match project(rooms, select.as_ref()) {
            Ok(v) => Json(v).into_response(),
            Err(e) => server_error(e.to_string()),
        },
        Err(e) => server_error(e.to_string()),
    }
}

async fn get_room(ctx: EntityContext, state: AppState) -> impl IntoResponse {
    let select = ctx.query.select.clone();
    match ctx.oquery::<Room>("rooms", "id").fetch_optional(&state).await {
        Ok(Some(room)) => match project(room, select.as_ref()) {
            Ok(v) => Json(v).into_response(),
            Err(e) => server_error(e.to_string()),
        },
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => server_error(e.to_string()),
    }
}

async fn create_room(ctx: CollectionContext, state: AppState) -> impl IntoResponse {
    let Some(body) = ctx.body else {
        return (StatusCode::BAD_REQUEST, "expected JSON body").into_response();
    };
    let Ok(room) = serde_json::from_value::<Room>(body) else {
        return (StatusCode::BAD_REQUEST, "invalid Room payload").into_response();
    };
    match sqlx::query("INSERT INTO rooms (id, name) VALUES (?, ?)")
        .bind(&room.id)
        .bind(&room.name)
        .execute(&*state)
        .await
    {
        Ok(_) => (StatusCode::CREATED, Json(room)).into_response(),
        Err(e) => server_error(e.to_string()),
    }
}

async fn delete_room(ctx: EntityContext, state: AppState) -> impl IntoResponse {
    match sqlx::query("DELETE FROM rooms WHERE id = ?")
        .bind(&ctx.key)
        .execute(&*state)
        .await
    {
        Ok(r) if r.rows_affected() == 0 => StatusCode::NOT_FOUND.into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => server_error(e.to_string()),
    }
}

async fn list_printers(ctx: ContainedCollectionContext, state: AppState) -> impl IntoResponse {
    match ctx
        .oquery_dynamic("printers", "room_id")
        .fetch_all(&state)
        .await
    {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => server_error(e.to_string()),
    }
}

async fn get_printer(ctx: ContainedEntityContext, state: AppState) -> impl IntoResponse {
    let select = ctx.query.select.clone();
    match ctx
        .oquery::<Printer>("printers", "room_id", "id")
        .fetch_optional(&state)
        .await
    {
        Ok(Some(printer)) => match project(printer, select.as_ref()) {
            Ok(v) => Json(v).into_response(),
            Err(e) => server_error(e.to_string()),
        },
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => server_error(e.to_string()),
    }
}

async fn create_printer(ctx: ContainedCollectionContext, state: AppState) -> impl IntoResponse {
    let Some(body) = ctx.body.clone() else {
        return (StatusCode::BAD_REQUEST, "expected JSON body").into_response();
    };
    let Ok(printer) = serde_json::from_value::<Printer>(body) else {
        return (StatusCode::BAD_REQUEST, "invalid Printer payload").into_response();
    };
    match sqlx::query("INSERT INTO printers (id, room_id, model) VALUES (?, ?, ?)")
        .bind(&printer.id)
        .bind(&ctx.parent_key)
        .bind(&printer.model)
        .execute(&*state)
        .await
    {
        Ok(_) => (StatusCode::CREATED, Json(printer)).into_response(),
        Err(e) => server_error(e.to_string()),
    }
}

async fn delete_printer(ctx: ContainedEntityContext, state: AppState) -> impl IntoResponse {
    match sqlx::query("DELETE FROM printers WHERE room_id = ? AND id = ?")
        .bind(&ctx.parent_key)
        .bind(&ctx.key)
        .execute(&*state)
        .await
    {
        Ok(r) if r.rows_affected() == 0 => StatusCode::NOT_FOUND.into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => server_error(e.to_string()),
    }
}

fn server_error(msg: String) -> axum::response::Response {
    (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
}

// ---------------------------------------------------------------------------
// Request helpers
// ---------------------------------------------------------------------------

/// Send one request through the router. Returns the status and the parsed JSON
/// body (or `Value::Null` for empty / non-JSON bodies such as 204/404/501).
async fn send(app: Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(v) => builder
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&v).unwrap()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };

    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

async fn get(app: Router, uri: &str) -> (StatusCode, Value) {
    send(app, "GET", uri, None).await
}

// ---------------------------------------------------------------------------
// Smoke: routes resolve and bodies have the expected shape
// ---------------------------------------------------------------------------

#[tokio::test]
async fn collection_get_returns_seeded_rooms() {
    let app = build_app().await;
    let (status, body) = get(app, "/rooms").await;

    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().expect("rooms collection is a JSON array");
    let ids: Vec<&str> = arr.iter().filter_map(|r| r["id"].as_str()).collect();
    assert!(ids.contains(&"oak-204"), "got: {ids:?}");
    assert!(ids.contains(&"redw-1002"), "got: {ids:?}");
}

#[tokio::test]
async fn entity_get_returns_single_object() {
    let app = build_app().await;
    let (status, body) = get(app, "/rooms/oak-204").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.is_object(), "expected a single object, got {body}");
    assert_eq!(body["name"], "Oak 204");
}

#[tokio::test]
async fn service_document_lists_entity_sets() {
    let app = build_app().await;
    let (status, body) = get(app, "/").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["@odata.context"], "$metadata");
    let entries = body["value"].as_array().expect("value is an array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["name"], "rooms");
    assert_eq!(entries[0]["kind"], "EntitySet");
}

// ---------------------------------------------------------------------------
// Context extraction: the key / parent key reach the handler and filter rows
// ---------------------------------------------------------------------------

#[tokio::test]
async fn entity_key_is_extracted_from_path() {
    let app = build_app().await;
    let (status, body) = get(app, "/rooms/oak-204").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["id"], "oak-204",
        "the entity key from the URL must select that exact row"
    );
}

#[tokio::test]
async fn contained_parent_key_filters_collection() {
    let app = build_app().await;
    let (status, body) = get(app, "/rooms/redw-1002/printers").await;

    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().expect("printers collection is an array");
    assert_eq!(arr.len(), 2, "redw-1002 has two seeded printers");
    assert!(
        arr.iter().all(|p| p["room_id"] == "redw-1002"),
        "every row must belong to the parent key from the URL: {body}"
    );
}

#[tokio::test]
async fn contained_entity_uses_parent_key_and_key() {
    let app = build_app().await;
    let (status, body) = get(app, "/rooms/oak-204/printers/prn0204-100").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], "prn0204-100");
    assert_eq!(body["model"], "Brother HL-L6400");
}

// ---------------------------------------------------------------------------
// Contained routes resolve at both collection and entity depth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn contained_routes_resolve() {
    let app = build_app().await;

    let (collection_status, _) = get(app.clone(), "/rooms/redw-1002/printers").await;
    assert_eq!(collection_status, StatusCode::OK);

    let (entity_status, _) = get(app, "/rooms/redw-1002/printers/prn1002-100").await;
    assert_eq!(entity_status, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Status matrix
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_returns_201() {
    let app = build_app().await;
    let (status, _) = send(
        app,
        "POST",
        "/rooms",
        Some(json!({ "id": "maple-7", "name": "Maple 7" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn unknown_entity_id_returns_404() {
    let app = build_app().await;
    let (status, _) = get(app, "/rooms/does-not-exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unregistered_operation_returns_501() {
    let app = build_app().await;
    // `update` (PATCH) was never registered → dispatch falls through to 501.
    let (status, _) = send(app, "PATCH", "/rooms/oak-204", None).await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let app = build_app().await;
    let (status, _) = get(app, "/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
