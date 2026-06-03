# Architecture

## Crate layout

- **`odata-rs-edm`** — CSDL parser, `Schema`, `EntityType`, `EntitySet`, `NavigationProperty`.
- **`odata-rs-url`** — URL / query-string parser. Two public types:
  - `ODataQuery` — full parsed URL (resource path, path markers, system query options, custom options).
  - `QueryOptions` — the system-query-option subset a handler actually needs (`select`, `filter`, `expand`, `page`, `orderby`, `count`, `custom`). Built from `ODataQuery` or from a raw `?...` string.
- **`odata-rs-service`** — axum-based router, handler contexts (`CollectionContext`, `EntityContext`, contained variants), and the builder API. Optionally exposes `odata_service::oquery` (sqlx/SQLite query builder) behind the `sqlx-sqlite` feature.

The umbrella crate `odata-rs` re-exports the three under matching features (`edm`, `url`, `service`, `sqlx-sqlite`, `full`).

## Handler signatures and application state

Every handler is registered as `Fn(Context, S) -> impl Future`, where `Context` is one of `CollectionContext` / `EntityContext` / `ContainedCollectionContext` / `ContainedEntityContext` and `S` is an application-state type chosen by the builder.

The builder is generic: `ODataServiceBuilder<S = ()>`. There are two entry shapes:

```rust
// Stateless — handlers are `Fn(Context, ())`. The trailing `()` argument is
// usually written `_` in the closure.
ODataServiceBuilder::new(schema)
    .entity_set("Rooms", |es| es.list(|ctx, _| async move { ... }))
    .build();

// Stateful — handlers are `Fn(Context, S)` with `S: Clone + Send + Sync + 'static`.
ODataServiceBuilder::new(schema)
    .with_state(Arc::new(pool))
    .entity_set("Rooms", |es| es.list(|ctx, pool: Arc<SqlitePool>| async move { ... }))
    .build();
```

`with_state` is only callable on `ODataServiceBuilder<()>`. The type system enforces that state is attached before any handler registration: once you've registered a handler, the builder's `S` is fixed and `with_state` is not in scope.

State is cloned per request — wrap large or non-`Clone` state in `Arc<T>` so the per-call clone is cheap. This mirrors `axum::Router::with_state`. The state value is not magically injected as an extractor; it is a normal second function argument the handler always sees.

## OData spec notes

### `$select` is a response projection, not a query constraint

OData v4 URL Conventions §5.1.4 defines `$select` as the system query option that controls which *structural properties of the resource* appear in the response. It does **not** restrict what other query options can reference.

Concretely:

- `$filter` (§5.1.1) and `$orderby` (§5.1.5) operate against the full entity type. They may reference any property of the resource regardless of whether it appears in `$select`.
- Key access (`/Rooms('id')`) and parent-key matching for contained navigation (`/Rooms('id')/Printers`) likewise operate on the underlying entity, not the projection.

So at SQL level the engine must always read enough columns to satisfy WHERE / ORDER BY / key matching. `$select` is then applied to the *response shape*, not the SELECT list (unless the row representation can tolerate a sparse column set — see below).

## Row representation: typed vs dynamic

Handler authors choose how to materialize rows. Each row type has its own SQL contract and its own ergonomics; the two combinations that are awkward are awkward because the handler is asking for two incompatible things at once.

| Handler reads `row.field` in Rust? | Wants `$select` to prune SQL? | Path |
|---|---|---|
| No — just forwards the row to JSON | No | Typed `OQuery<T>` — always-full SELECT from the allowlist. Cheapest, simplest. |
| No — just forwards the row to JSON | Yes | Dynamic `OQueryDynamic` — `$select` drives the SQL projection; output is the row as-is. |
| Yes — reads typed fields in Rust | No | Typed `OQuery<T>` — always-full SELECT; optional response-side JSON projection. |
| Yes — reads typed fields in Rust | Yes | **Awkward.** The handler wants a known struct shape *and* a row that may be missing fields. The wrapper does not paper over this. Use one of sqlx's own escape hatches: `#[sqlx(default)]` on each optional field, a hand-written `FromRow` impl, or drop to the dynamic path. |

The wrapper exposes both paths as distinct entry points (`OQuery::<T>::from(...)` vs `OQueryDynamic::from(...)`) sharing the same fluent surface. Only the terminal row type differs.

### Why this split rather than a single unified type

A single `OQuery<T>` that lets `$select` shrink the SQL projection would silently break the strict-`FromRow` contract — sqlx's derived `FromRow` calls `try_get(name)` for every struct field and errors on a missing column. The only way to make that work is to weaken the struct (`Option<T>` fields, `#[sqlx(default)]`, manual impl), which pushes the awkwardness onto every typed row even when the handler doesn't want `$select` pruning. Splitting the paths keeps each one honest about what it guarantees.

## `OQuery` fluent surface

Both variants accept the same chain of clause builders; only construction and the terminal call differ:

```rust
// Typed — Vec<Room>
let query = OQuery::<Room>::from("rooms")
    .select(None, &["id", "name"])      // $select ignored — always full SELECT
    .orderby(ctx.query.orderby.as_ref(), &["id", "name"])
    .page(&ctx.query.page);
let rooms: Vec<Room> = query.fetch_all(db).await?;

// Dynamic — Vec<serde_json::Map<String, Value>>
let query = OQueryDynamic::from("printers")
    .select(ctx.query.select.as_ref(), &["id", "model", "room_id"])  // $select drives SQL
    .where_eq("room_id", ctx.parent_key)
    .page(&ctx.query.page);
let rows = query.fetch_all(db).await?;
```

Identifiers (column names, sort direction) are always filtered through caller-supplied allowlists. Values always go through `push_bind`. The wrapper buffers clause pieces independently and emits them in correct SQL order at terminal-method time, so the fluent chain is order-independent at the handler level.

### Allowlist shape: `Allowed`

`select` and `orderby` on both `OQuery<T>` and `OQueryDynamic` take an
`impl Into<Allowed<'_>>` for the column allowlist:

```rust
pub enum Allowed<'a> {
    All,
    Only(&'a [&'a str]),
}
```

`From<&[&str]>` and `From<&[&str; N]>` are provided, so existing call sites
keep passing slice literals (`&["id", "name"]`) unchanged. To opt out of
allowlisting, pass `Allowed::All`. Per-path semantics:

| Method                          | `Allowed::Only(cols)`                          | `Allowed::All`                                       |
|---------------------------------|------------------------------------------------|------------------------------------------------------|
| `OQuery<T>::select`             | SQL `SELECT cols`                              | SQL `SELECT *` — caller asserts `T` matches the table |
| `OQueryDynamic::select`         | `sel ∩ cols`, fallback to `cols` when empty    | `sel` verbatim; falls back to `*` when no `$select`   |
| `*::orderby`                    | only `cols` may sort                           | any column may sort                                   |

The typed `Allowed::All` is brittle to schema evolution (any new column the
`FromRow` doesn't know about will mismatch). Prefer `Allowed::Only` when you
can enumerate.

## What is intentionally *not* in scope (yet)

- `$filter` translation to SQL `WHERE`. The expression tree is parsed but not lowered.
- `$expand` materialization.
- `$count` inline / `/$count`.
- Composite keys.
- Output-side response-projection helper for `$select` on the typed path (open follow-up).
