//! Example: a building-management OData service backed by an in-memory
//! SQLite database (via `sqlx`).
//!
//! Schema
//!   EntityType  Room       { nav: Printers (contained) }
//!   EntityType  Printer
//!   EntitySet   Rooms      → Room
//!
//! Registered:
//!   - Rooms          GET list, GET get, POST create, DELETE
//!   - Rooms/Printers GET list, GET get, POST create, DELETE
//!
//! The `list` handlers honor the OData system query options that map cleanly
//! to SQL today: `$top`, `$skip`, and `$orderby` (allowlisted columns).
//! `$filter`, `$select`, `$expand`, and `$count` are received but ignored —
//! wiring those is a separate translation step.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{FromRow, SqlitePool};

use odata_edm::Schema;
use odata_service::{
    CollectionContext, ContainedCollectionContext, ContainedEntityContext, EntityContext,
    ODataServiceBuilder,
};

// `Allowed` is re-exported from `odata_service::oquery` for handlers that want
// to pass `Allowed::All`. The example sticks to slice literals, which auto-
// convert to `Allowed::Only` via the `From` impl.
use odata_service::oquery::{OQuery, OQueryDynamic};

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------
//
// The state value is cloned per request and passed as the handler's second
// argument. We wrap the pool in `Arc` so cloning is cheap.

type AppState = Arc<SqlitePool>;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Room {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Printer {
    pub id: String,
    pub model: String,
    #[serde(skip)]
    #[sqlx(default)]
    pub room_id: String,
}

// ---------------------------------------------------------------------------
// Room handlers
// ---------------------------------------------------------------------------

// NOTE: `$select` is currently passed as `None` because the typed row structs
// (`Room`, `Printer`) require all columns to be present for `FromRow`.
// Honoring `$select` end-to-end needs either `Option<T>` fields, a
// `serde_json::Value` row type, or output-side JSON projection — a separate
// change.

async fn list_rooms(ctx: CollectionContext, pool: AppState) -> impl IntoResponse {
    let query = OQuery::<Room>::from("rooms")
        .select(None, &["id", "name"])
        .orderby(ctx.query.orderby.as_ref(), &["id", "name"])
        .page(&ctx.query.page);

    match query.fetch_all(&pool).await {
        Ok(rooms) => Json(rooms).into_response(),
        Err(e) => server_error(e),
    }
}

async fn get_room(ctx: EntityContext, pool: AppState) -> impl IntoResponse {
    let query = OQuery::<Room>::from("rooms")
        .select(None, &["id", "name"])
        .where_eq("id", ctx.key);

    match query.fetch_optional(&pool).await {
        Ok(Some(room)) => Json(room).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => server_error(e),
    }
}

async fn create_room(ctx: CollectionContext, pool: AppState) -> impl IntoResponse {
    let Some(body) = ctx.body else {
        return (StatusCode::BAD_REQUEST, "expected JSON body").into_response();
    };
    let Ok(room) = serde_json::from_value::<Room>(body) else {
        return (StatusCode::BAD_REQUEST, "invalid Room payload").into_response();
    };
    match sqlx::query("INSERT INTO rooms (id, name) VALUES (?, ?)")
        .bind(&room.id)
        .bind(&room.name)
        .execute(&*pool)
        .await
    {
        Ok(_) => (StatusCode::CREATED, Json(room)).into_response(),
        Err(e) => server_error(e),
    }
}

async fn delete_room(ctx: EntityContext, pool: AppState) -> impl IntoResponse {
    match sqlx::query("DELETE FROM rooms WHERE id = ?")
        .bind(&ctx.key)
        .execute(&*pool)
        .await
    {
        Ok(r) if r.rows_affected() == 0 => StatusCode::NOT_FOUND.into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => server_error(e),
    }
}

// ---------------------------------------------------------------------------
// Printer (contained) handlers
// ---------------------------------------------------------------------------

// Demonstrates the dynamic OQuery path: rows come back as JSON maps, and
// `$select` shrinks the SQL projection directly.
async fn list_printers(ctx: ContainedCollectionContext, pool: AppState) -> impl IntoResponse {
    let query = OQueryDynamic::from("printers")
        .select(ctx.query.select.as_ref(), &["id", "model", "room_id"])
        .where_eq("room_id", ctx.parent_key)
        .orderby(ctx.query.orderby.as_ref(), &["id", "model"])
        .page(&ctx.query.page);

    match query.fetch_all(&pool).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => server_error(e),
    }
}

async fn get_printer(ctx: ContainedEntityContext, pool: AppState) -> impl IntoResponse {
    let query = OQuery::<Printer>::from("printers")
        .select(None, &["id", "model", "room_id"])
        .where_eq("room_id", ctx.parent_key)
        .where_eq("id", ctx.key);

    match query.fetch_optional(&pool).await {
        Ok(Some(printer)) => Json(printer).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => server_error(e),
    }
}

async fn create_printer(ctx: ContainedCollectionContext, pool: AppState) -> impl IntoResponse {
    let Some(body) = ctx.body else {
        return (StatusCode::BAD_REQUEST, "expected JSON body").into_response();
    };
    let Ok(printer) = serde_json::from_value::<Printer>(body) else {
        return (StatusCode::BAD_REQUEST, "invalid Printer payload").into_response();
    };
    match sqlx::query("INSERT INTO printers (id, room_id, model) VALUES (?, ?, ?)")
        .bind(&printer.id)
        .bind(&ctx.parent_key)
        .bind(&printer.model)
        .execute(&*pool)
        .await
    {
        Ok(_) => (StatusCode::CREATED, Json(printer)).into_response(),
        Err(e) => server_error(e),
    }
}

async fn delete_printer(ctx: ContainedEntityContext, pool: AppState) -> impl IntoResponse {
    match sqlx::query("DELETE FROM printers WHERE room_id = ? AND id = ?")
        .bind(&ctx.parent_key)
        .bind(&ctx.key)
        .execute(&*pool)
        .await
    {
        Ok(r) if r.rows_affected() == 0 => StatusCode::NOT_FOUND.into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => server_error(e),
    }
}

fn server_error(err: sqlx::Error) -> axum::response::Response {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
}

// ---------------------------------------------------------------------------
// Database setup
// ---------------------------------------------------------------------------

async fn init_db() -> SqlitePool {
    // `:memory:` databases are private to each connection. To share one
    // in-memory DB across the pool we use a named, shared-cache URI and keep
    // one connection alive so the DB isn't dropped between requests.
    let opts: SqliteConnectOptions = "sqlite:file:odata_rooms?mode=memory&cache=shared"
        .parse()
        .expect("invalid sqlite connect string");

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .min_connections(1)
        .connect_with(opts)
        .await
        .expect("failed to open in-memory sqlite");

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
    .expect("failed to create schema");

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

// ---------------------------------------------------------------------------
// Schema loading from CSDL
// ---------------------------------------------------------------------------

fn rooms_csdl_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/rooms/rooms.csdl.xml")
}

fn build_schema() -> odata_edm::Result<Schema> {
    let path = rooms_csdl_path();
    let csdl = fs::read_to_string(&path).map_err(|error| {
        odata_edm::Error::Csdl(format!(
            "failed to read CSDL file '{}': {error}",
            path.display()
        ))
    })?;

    Schema::from_csdl(&csdl)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let schema = build_schema().expect("cannot parse rooms.csdl.xml into a service schema");
    let pool: AppState = Arc::new(init_db().await);

    let app = ODataServiceBuilder::new(schema)
        .with_state(pool)
        .entity_set("Rooms", |es| {
            es.list(list_rooms)
                .get(get_room)
                .create(create_room)
                .delete(delete_room)
                .contained("Printers", |nav| {
                    nav.list(list_printers)
                        .get(get_printer)
                        .create(create_printer)
                        .delete(delete_printer)
                })
        })
        .build();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
