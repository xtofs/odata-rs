//! Fluent SQL builders over `sqlx::QueryBuilder<Sqlite>`.
//!
//! Two row representations live here, sharing the same fluent surface:
//!
//! - [`OQuery<T>`] — typed rows via `sqlx::FromRow`. The SQL projection is
//!   always the full allowlist; `$select` is **not** threaded into the SQL
//!   SELECT list because that would break `FromRow`'s strict column check.
//!   Use this when the handler reads typed fields off the row in Rust.
//!
//! - [`OQueryDynamic`] — rows as `serde_json::Map<String, Value>`. Here
//!   `$select` *does* drive the SQL projection: only the columns that survive
//!   `sel ∩ allowed` are read. Use this when the handler forwards the row
//!   straight to JSON.
//!
//! See `ARCHITECTURE.md` for the tradeoff matrix and OData spec notes.

use std::marker::PhantomData;

use odata_url::{OrderByClause, Page, SelectClause};
use serde_json::{Map as JsonMap, Value as JsonValue};
use sqlx::sqlite::SqliteRow;
use sqlx::{Column, FromRow, QueryBuilder, Row, Sqlite, SqlitePool, ValueRef};

// ---------------------------------------------------------------------------
// Allowed: column allowlist with an explicit "any" mode
// ---------------------------------------------------------------------------

/// Column allowlist passed to [`OQuery::select`] / [`OQueryDynamic::select`]
/// and the matching `orderby` methods.
///
/// - [`Allowed::All`] — no constraint. The typed `select` emits `SELECT *`;
///   the dynamic `select` uses `$select` verbatim (or `*` if no `$select`).
///   `orderby` accepts any column the client requests.
/// - [`Allowed::Only`] — only the listed columns are valid.
///
/// `From<&[&str]>` and `From<&[&str; N]>` are provided so most call sites can
/// keep passing `&["id", "name"]` directly via `impl Into<Allowed<'_>>`.
#[derive(Debug, Clone, Copy)]
pub enum Allowed<'a> {
    All,
    Only(&'a [&'a str]),
}

impl<'a> Allowed<'a> {
    fn contains(&self, name: &str) -> bool {
        match self {
            Allowed::All => true,
            Allowed::Only(cols) => cols.iter().any(|c| *c == name),
        }
    }

    /// Yields the explicit column list, or `None` for [`Allowed::All`].
    fn explicit(&self) -> Option<&'a [&'a str]> {
        match self {
            Allowed::All => None,
            Allowed::Only(cols) => Some(*cols),
        }
    }
}

impl<'a> From<&'a [&'a str]> for Allowed<'a> {
    fn from(s: &'a [&'a str]) -> Self {
        Allowed::Only(s)
    }
}

impl<'a, const N: usize> From<&'a [&'a str; N]> for Allowed<'a> {
    fn from(s: &'a [&'a str; N]) -> Self {
        Allowed::Only(s.as_slice())
    }
}

// ---------------------------------------------------------------------------
// Shared SQL pieces (private)
// ---------------------------------------------------------------------------

/// Buffered clause pieces, emitted in correct SQL order by [`Pieces::build_qb`].
///
/// An empty `columns` vector renders as `SELECT *`.
struct Pieces {
    table: String,
    columns: Vec<String>,
    wheres: Vec<(String, String)>,
    orderby: Vec<(String, &'static str)>,
    limit: Option<i64>,
    offset: Option<i64>,
}

impl Pieces {
    fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            columns: Vec::new(),
            wheres: Vec::new(),
            orderby: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    fn push_where_eq(&mut self, col: &str, value: impl Into<String>) {
        self.wheres.push((col.to_string(), value.into()));
    }

    fn push_orderby(&mut self, ob: Option<&OrderByClause>, allowed: Allowed<'_>) {
        let Some(clause) = ob else { return };
        for item in clause.expression.split(',') {
            let mut it = item.trim().split_whitespace();
            let Some(col) = it.next() else { continue };
            if !allowed.contains(col) {
                continue;
            }
            let dir = match it.next().map(str::to_ascii_lowercase).as_deref() {
                Some("desc") => "DESC",
                _ => "ASC",
            };
            self.orderby.push((col.to_string(), dir));
        }
    }

    fn apply_page(&mut self, p: &Page) {
        self.limit = p.top.map(|n| n as i64);
        self.offset = p.skip.map(|n| n as i64);
    }

    fn build_qb(&self) -> QueryBuilder<'_, Sqlite> {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT ");
        if self.columns.is_empty() {
            qb.push("*");
        } else {
            qb.push(self.columns.join(", "));
        }
        qb.push(" FROM ");
        qb.push(&self.table);
        for (i, (col, val)) in self.wheres.iter().enumerate() {
            qb.push(if i == 0 { " WHERE " } else { " AND " });
            qb.push(col);
            qb.push(" = ");
            qb.push_bind(val.clone());
        }
        if !self.orderby.is_empty() {
            qb.push(" ORDER BY ");
            let parts: Vec<String> = self
                .orderby
                .iter()
                .map(|(c, d)| format!("{c} {d}"))
                .collect();
            qb.push(parts.join(", "));
        }
        if self.limit.is_some() || self.offset.is_some() {
            // SQLite requires LIMIT when OFFSET is present.
            qb.push(" LIMIT ");
            qb.push(self.limit.unwrap_or(-1).to_string());
            qb.push(" OFFSET ");
            qb.push(self.offset.unwrap_or(0).to_string());
        }
        qb
    }
}

fn cols_to_strings(cols: &[&str]) -> Vec<String> {
    cols.iter().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// Typed path: OQuery<T>
// ---------------------------------------------------------------------------

/// Typed query: rows are decoded via `sqlx::FromRow` into `T`.
///
/// The SQL projection is always driven by the allowlist passed to
/// [`Self::select`]; `$select` is intentionally ignored at SQL level because
/// shrinking the projection would break the strict-`FromRow` contract on `T`.
/// Honor `$select` at response-serialization time instead.
pub struct OQuery<T> {
    pieces: Pieces,
    // `fn() -> T` keeps `OQuery<T>` Send+Sync regardless of T.
    _row: PhantomData<fn() -> T>,
}

impl<T> OQuery<T>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
{
    /// Build a typed query targeting `table`. Use as `OQuery::<Room>::from("rooms")`.
    pub fn from(table: impl Into<String>) -> Self {
        Self {
            pieces: Pieces::new(table),
            _row: PhantomData,
        }
    }

    /// Locks the SQL SELECT list.
    ///
    /// The `_sel` argument is accepted for API symmetry with
    /// [`OQueryDynamic::select`] but is **ignored** here: `T`'s `FromRow`
    /// requires every struct field's column to be present in the row, so the
    /// projection cannot be shrunk by `$select`.
    ///
    /// - `Allowed::Only(cols)` → SQL `SELECT cols`. Default for most call
    ///   sites; pass a slice literal and let the `From` impl convert.
    /// - `Allowed::All` → SQL `SELECT *`. Caller asserts that `T`'s
    ///   `FromRow` matches the table's full column set. Brittle to schema
    ///   evolution — prefer `Allowed::Only` when you can enumerate.
    pub fn select<'a>(
        mut self,
        _sel: Option<&SelectClause>,
        allowed: impl Into<Allowed<'a>>,
    ) -> Self {
        self.pieces.columns = match allowed.into() {
            Allowed::All => Vec::new(), // → SELECT *
            Allowed::Only(cols) => cols_to_strings(cols),
        };
        self
    }

    pub fn where_eq(mut self, col: &str, value: impl Into<String>) -> Self {
        self.pieces.push_where_eq(col, value);
        self
    }

    /// Apply `$orderby`. Columns are validated against `allowed`.
    /// Pass [`Allowed::All`] to accept any column the client sorts by.
    pub fn orderby<'a>(
        mut self,
        ob: Option<&OrderByClause>,
        allowed: impl Into<Allowed<'a>>,
    ) -> Self {
        self.pieces.push_orderby(ob, allowed.into());
        self
    }

    pub fn page(mut self, p: &Page) -> Self {
        self.pieces.apply_page(p);
        self
    }

    pub async fn fetch_all(&self, pool: &SqlitePool) -> Result<Vec<T>, sqlx::Error> {
        let mut qb = self.pieces.build_qb();
        qb.build_query_as::<T>().fetch_all(pool).await
    }

    pub async fn fetch_optional(&self, pool: &SqlitePool) -> Result<Option<T>, sqlx::Error> {
        let mut qb = self.pieces.build_qb();
        qb.build_query_as::<T>().fetch_optional(pool).await
    }
}

// ---------------------------------------------------------------------------
// Dynamic path: OQueryDynamic
// ---------------------------------------------------------------------------

/// Dynamic query: rows are returned as `serde_json::Map<String, Value>`.
///
/// `$select` drives the SQL projection, optionally filtered by an allowlist.
pub struct OQueryDynamic {
    pieces: Pieces,
}

impl OQueryDynamic {
    pub fn from(table: impl Into<String>) -> Self {
        Self {
            pieces: Pieces::new(table),
        }
    }

    /// Sets the SQL SELECT list.
    ///
    /// | `sel`   | `allowed`             | SQL SELECT becomes              |
    /// |---------|-----------------------|---------------------------------|
    /// | `Some`  | `Allowed::Only(cols)` | `sel ∩ cols` — fallback to cols |
    /// | `Some`  | `Allowed::All`        | `sel` verbatim                  |
    /// | `None`  | `Allowed::Only(cols)` | `cols`                          |
    /// | `None`  | `Allowed::All`        | `*`                             |
    pub fn select<'a>(
        mut self,
        sel: Option<&SelectClause>,
        allowed: impl Into<Allowed<'a>>,
    ) -> Self {
        let allowed = allowed.into();
        self.pieces.columns = match (sel, allowed.explicit()) {
            (Some(s), Some(cols)) => {
                let filtered: Vec<String> = s
                    .items
                    .iter()
                    .filter(|item| cols.iter().any(|c| *c == item.as_str()))
                    .cloned()
                    .collect();
                if filtered.is_empty() {
                    cols_to_strings(cols)
                } else {
                    filtered
                }
            }
            (Some(s), None) => s.items.clone(),
            (None, Some(cols)) => cols_to_strings(cols),
            (None, None) => Vec::new(), // → SELECT *
        };
        self
    }

    pub fn where_eq(mut self, col: &str, value: impl Into<String>) -> Self {
        self.pieces.push_where_eq(col, value);
        self
    }

    /// Apply `$orderby`. Columns are validated against `allowed`.
    /// Pass [`Allowed::All`] to accept any column the client sorts by.
    pub fn orderby<'a>(
        mut self,
        ob: Option<&OrderByClause>,
        allowed: impl Into<Allowed<'a>>,
    ) -> Self {
        self.pieces.push_orderby(ob, allowed.into());
        self
    }

    pub fn page(mut self, p: &Page) -> Self {
        self.pieces.apply_page(p);
        self
    }

    pub async fn fetch_all(
        &self,
        pool: &SqlitePool,
    ) -> Result<Vec<JsonMap<String, JsonValue>>, sqlx::Error> {
        let mut qb = self.pieces.build_qb();
        let rows = qb.build().fetch_all(pool).await?;
        Ok(rows.iter().map(row_to_json_map).collect())
    }

    pub async fn fetch_optional(
        &self,
        pool: &SqlitePool,
    ) -> Result<Option<JsonMap<String, JsonValue>>, sqlx::Error> {
        let mut qb = self.pieces.build_qb();
        let row = qb.build().fetch_optional(pool).await?;
        Ok(row.as_ref().map(row_to_json_map))
    }
}

/// Convert one SQLite row into a JSON object. NULLs become `Value::Null`;
/// INTEGER / REAL / TEXT are decoded by trying the corresponding sqlx
/// `try_get` in turn. BLOB and anything unrecognized fall through to
/// `Value::Null`.
fn row_to_json_map(row: &SqliteRow) -> JsonMap<String, JsonValue> {
    let mut out = JsonMap::new();
    for (i, col) in row.columns().iter().enumerate() {
        let name = col.name().to_string();
        let raw = match row.try_get_raw(i) {
            Ok(r) => r,
            Err(_) => {
                out.insert(name, JsonValue::Null);
                continue;
            }
        };
        let value = if raw.is_null() {
            JsonValue::Null
        } else if let Ok(v) = row.try_get::<i64, _>(i) {
            JsonValue::Number(v.into())
        } else if let Ok(v) = row.try_get::<f64, _>(i) {
            serde_json::Number::from_f64(v)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null)
        } else if let Ok(v) = row.try_get::<String, _>(i) {
            JsonValue::String(v)
        } else {
            JsonValue::Null
        };
        out.insert(name, value);
    }
    out
}
