# Claude Code Prompt — Scaffold a Rust OData Library

## Context

Scaffold a Rust library crate for an OData server. The library is explicitly **not** modelled on
ASP.NET’s OData stack. It does not assume IQueryable-style deferred execution, an ORM, or any
particular async runtime beyond `async_trait`. The design separates three concerns:

- **Parsing and EDM** — caller-supplied; the library exposes trait boundaries for these, not
  implementations.
- **Query representation and composition** — owned by the library.
- **Execution and serialization** — split: the caller owns execution (via a trait impl), the
  library owns OData wire-format serialization.

-----

## What to scaffold

### 1. Crate structure

```
odata/
  Cargo.toml
  src/
    lib.rs
    query/
      mod.rs          # ODataQuery struct and sub-types
      filter.rs       # FilterExpr AST + combinators
      order_by.rs     # OrderByItem
      select.rs       # SelectClause
      expand.rs       # ExpandItem
      transform.rs    # QueryTransform type alias + pipe() helper
    source/
      mod.rs          # ODataSource trait + ODataResponse
      expand.rs       # Expandable<Relation> trait
    serial/
      mod.rs          # OData JSON envelope serialization (serde_json)
    edm/
      mod.rs          # EdmModel trait — caller implements this
    parser/
      mod.rs          # ODataUrlParser trait — caller implements this
    error.rs          # ODataError enum
```

### 2. Core types

#### `query/mod.rs` — `ODataQuery`

```rust
/// A fully parsed OData request. Pure data; no behavior.
pub struct ODataQuery {
    pub filter:   Option<FilterExpr>,
    pub select:   Option<SelectClause>,
    pub order_by: Vec<OrderByItem>,
    pub top:      Option<usize>,
    pub skip:     Option<usize>,
    pub count:    bool,
    pub expand:   Vec<ExpandItem>,
}
```

#### `query/filter.rs` — `FilterExpr`

Provide a recursive enum covering at minimum:

- `And(Box<FilterExpr>, Box<FilterExpr>)`
- `Or(Box<FilterExpr>, Box<FilterExpr>)`
- `Not(Box<FilterExpr>)`
- `Eq(String, FilterValue)` / `Ne` / `Lt` / `Le` / `Gt` / `Ge`
- `In(String, Vec<FilterValue>)`

Include combinator methods `and()`, `or()`, `not()` on `FilterExpr` so callers can build
expressions programmatically (used by query transforms for permission filtering etc.).

`FilterValue` should cover `String`, `i64`, `f64`, `bool`, `Uuid`, `chrono::DateTime<Utc>`,
and `Null`.

#### `query/transform.rs` — `QueryTransform`

```rust
pub type QueryTransform = Box<dyn Fn(ODataQuery) -> ODataQuery + Send + Sync>;

pub trait QueryPipe: Sized {
    fn pipe(self, f: impl Fn(Self) -> Self + Send + Sync + 'static) -> Self;
}

impl QueryPipe for ODataQuery { ... }
```

Provide two example transforms as free functions:

- `and_filter(expr: FilterExpr) -> QueryTransform` — appends a filter with AND
- `limit_top(max: usize) -> QueryTransform` — clamps `top` to a maximum

### 3. Execution trait

#### `source/mod.rs` — `ODataSource`

```rust
use async_trait::async_trait;

pub struct ODataResponse<T> {
    pub value:     Vec<T>,
    pub count:     Option<usize>,   // populated when $count=true
    pub next_link: Option<String>,  // server-side paging
}

#[async_trait]
pub trait ODataSource: Send + Sync {
    type Entity: serde::Serialize + Send;
    type Error: std::error::Error + Send;

    async fn execute(
        &self,
        query: ODataQuery,
    ) -> Result<ODataResponse<Self::Entity>, Self::Error>;
}
```

#### `source/expand.rs` — `Expandable<Relation>`

```rust
#[async_trait]
pub trait Expandable<Relation>: ODataSource {
    async fn expand(
        &self,
        entities: &mut [Self::Entity],
        relation: &Relation,
    ) -> Result<(), Self::Error>;
}
```

### 4. Caller-supplied trait boundaries

#### `edm/mod.rs` — `EdmModel`

```rust
/// Caller implements this to expose $metadata.
pub trait EdmModel: Send + Sync {
    /// Returns the CSDL JSON or XML string for the $metadata endpoint.
    fn metadata_document(&self) -> String;

    /// Maps an entity set name to its Rust type name (used by serializer).
    fn entity_set_type(&self, entity_set: &str) -> Option<&str>;
}
```

#### `parser/mod.rs` — `ODataUrlParser`

```rust
/// Caller implements this. The library never parses OData URLs itself.
pub trait ODataUrlParser: Send + Sync {
    type Error: std::error::Error + Send;

    fn parse(&self, url: &str) -> Result<ODataQuery, Self::Error>;
}
```

### 5. Serialization

#### `serial/mod.rs`

Provide a `serialize_response` free function:

```rust
pub fn serialize_response<T: serde::Serialize>(
    response: ODataResponse<T>,
    context_url: &str,
) -> serde_json::Value
```

Output must conform to OData JSON format v4:

- Top-level `@odata.context`
- `@odata.count` (if `count` is `Some`)
- `@odata.nextLink` (if `next_link` is `Some`)
- `value` array

### 6. Error type

#### `error.rs`

```rust
#[derive(Debug, thiserror::Error)]
pub enum ODataError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("source error: {0}")]
    Source(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unsupported feature: {0}")]
    Unsupported(String),
}
```

### 7. `Cargo.toml` dependencies

```toml
[dependencies]
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
async-trait = "0.1"
thiserror   = "1"
uuid        = { version = "1", features = ["v4", "serde"] }
chrono      = { version = "0.4", features = ["serde"] }
```

-----

## What NOT to scaffold

- Any OData URL parser implementation (caller provides `ODataUrlParser` impl)
- Any EDM/CSDL document generation (caller provides `EdmModel` impl)
- Any HTTP framework integration (axum, actix, etc.) — that is a separate crate
- Any database access layer

-----

## Constraints

- No `unwrap()` anywhere in library code; propagate errors through `Result`
- All public types derive `Debug`
- `ODataQuery` and `FilterExpr` derive `Clone`
- Use `async_trait` for all async traits
- No global state; all types are instantiated by the caller

-----

## Deliverable

A compilable `cargo build` with `cargo test` passing on a minimal test:

```rust
#[tokio::test]
async fn smoke_test_transform_pipeline() {
    use odata::query::{ODataQuery, FilterExpr, FilterValue};
    use odata::query::transform::{QueryPipe, and_filter, limit_top};

    let base = ODataQuery::default();
    let q = base
        .pipe(and_filter(FilterExpr::Eq("tenantId".into(), FilterValue::String("acme".into()))))
        .pipe(limit_top(100));

    assert!(q.filter.is_some());
    assert_eq!(q.top, Some(100));
}
```