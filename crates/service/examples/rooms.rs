use axum::Json;
use axum::http::StatusCode;
/// Example: a building-management OData service.
///
/// Schema
///   EntityType  Room       { nav: Printers (contained) }
///   EntityType  Printer
///   EntitySet   Rooms      → Room
///
/// Registered:  Rooms/list, Rooms/get, Rooms/Printers/list, Rooms/Printers/get
/// Not registered (501):  Rooms/create, update, delete
///                        Rooms/Printers/create, update, delete
use axum::response::IntoResponse;
use std::fs;
use std::path::PathBuf;

use odata_edm::Schema;
use odata_service::{
    CollectionContext, ContainedCollectionContext, ContainedEntityContext, EntityContext,
    ODataServiceBuilder,
};

// ---------------------------------------------------------------------------
// Domain types and static fixture data used by this example
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct Room {
    pub id: &'static str,
    pub name: &'static str,
    pub printers: &'static [Printer],
}

#[derive(serde::Serialize)]
pub struct Printer {
    pub id: &'static str,
    pub model: &'static str,
}

static REDW_PRINTERS_ARR: [Printer; 2] = [
    Printer {
        id: "prn1002-100",
        model: "HP LaserJet",
    },
    Printer {
        id: "prn1002-200",
        model: "Canon ImageRunner",
    },
];

pub static REDW_PRINTERS: &[Printer] = &REDW_PRINTERS_ARR;

static OAK_PRINTERS_ARR: [Printer; 1] = [Printer {
    id: "prn0204-100",
    model: "Brother HL-L6400",
}];

pub static OAK_PRINTERS: &[Printer] = &OAK_PRINTERS_ARR;

static ROOM_DATA_ARR: [Room; 2] = [
    Room {
        id: "redw-1002",
        name: "Redwood 1002",
        printers: REDW_PRINTERS,
    },
    Room {
        id: "oak-204",
        name: "Oak 204",
        printers: OAK_PRINTERS,
    },
];

pub static ROOM_DATA: &[Room] = &ROOM_DATA_ARR;

// ---------------------------------------------------------------------------
// Handlers — plain async functions, no axum imports required
// ---------------------------------------------------------------------------

async fn list_rooms(_ctx: CollectionContext) -> impl IntoResponse {
    Json(ROOM_DATA)
}

async fn get_room(ctx: EntityContext) -> impl IntoResponse {
    match ROOM_DATA.iter().find(|r| r.id == ctx.key) {
        Some(room) => Json(room).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn list_printers(ctx: ContainedCollectionContext) -> impl IntoResponse {
    match ROOM_DATA.iter().find(|r| r.id == ctx.parent_key) {
        Some(room) => Json(room.printers).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_printer(ctx: ContainedEntityContext) -> impl IntoResponse {
    match ROOM_DATA.iter().find(|r| r.id == ctx.parent_key) {
        Some(room) => match room.printers.iter().find(|p| p.id == ctx.key) {
            Some(printer) => Json(printer).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Schema loading from CSDL
// ---------------------------------------------------------------------------

fn rooms_csdl_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/rooms.csdl.xml")
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
    let schema = build_schema().expect("rooms.csdl.xml should parse into a service schema");

    // Build the router. The library will:
    //   • warn for unregistered ops (create/update/delete on both levels)
    //   • wire 501 for those gaps automatically
    let app = ODataServiceBuilder::new(schema)
        .entity_set("Rooms", |es| {
            es.list(list_rooms)
                .get(get_room)
                .contained("Printers", |nav| {
                    nav.list(list_printers) // list printers in room
                        .get(get_printer) // get a specific printer in the room
                })
        })
        .build();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
