//! Fluent builder over `sqlx::QueryBuilder` that buffers SELECT / WHERE /
//! ORDER BY / LIMIT pieces independently of the order they're appended in
//! handler code, then emits them in correct SQL order at terminal-method time.
//!
//! Identifiers (columns, sort direction) are filtered through caller-supplied
//! allowlists; values are always bound via `push_bind`.

use std::marker::PhantomData;

use odata_url::{OrderByClause, QueryOptions, SelectClause};
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};

pub struct OQuery<T> {
    table: String,
    columns: Vec<String>,
    wheres: Vec<(String, String)>,
    orderby: Vec<(String, &'static str)>,
    limit: Option<i64>,
    offset: Option<i64>,
    // `fn() -> T` keeps `OQuery<T>` Send+Sync regardless of T.
    _row: PhantomData<fn() -> T>,
}

impl<T> OQuery<T>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
{
    /// Build a query targeting `table`, returning rows of type `T`.
    /// Use as `OQuery::<Room>::from("rooms")`.
    pub fn from(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            columns: Vec::new(),
            wheres: Vec::new(),
            orderby: Vec::new(),
            limit: None,
            offset: None,
            _row: PhantomData,
        }
    }

    /// Apply `$select` against an allowlist. If `$select` is absent (or no
    /// item matches the allowlist) the full allowlist is used.
    pub fn select(mut self, sel: Option<&SelectClause>, allowed: &[&str]) -> Self {
        self.columns = match sel {
            Some(s) => s
                .items
                .iter()
                .filter(|item| allowed.contains(&item.as_str()))
                .cloned()
                .collect(),
            None => Vec::new(),
        };
        if self.columns.is_empty() {
            self.columns = allowed.iter().map(|s| s.to_string()).collect();
        }
        self
    }

    /// Append a `col = ?` predicate with the value as a bound parameter.
    /// The column name is NOT validated — only call with trusted strings.
    pub fn where_eq(mut self, col: &str, value: impl Into<String>) -> Self {
        self.wheres.push((col.to_string(), value.into()));
        self
    }

    /// Apply `$orderby` against an allowlist of sortable columns.
    pub fn orderby(mut self, ob: Option<&OrderByClause>, allowed: &[&str]) -> Self {
        let Some(clause) = ob else { return self };
        for item in clause.expression.split(',') {
            let mut it = item.trim().split_whitespace();
            let Some(col) = it.next() else { continue };
            if !allowed.contains(&col) {
                continue;
            }
            let dir = match it.next().map(str::to_ascii_lowercase).as_deref() {
                Some("desc") => "DESC",
                _ => "ASC",
            };
            self.orderby.push((col.to_string(), dir));
        }
        self
    }

    /// Apply `$top` and `$skip` as LIMIT/OFFSET.
    pub fn top_skip(mut self, q: &QueryOptions) -> Self {
        self.limit = q.top.map(|n| n as i64);
        self.offset = q.skip.map(|n| n as i64);
        self
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

    pub async fn fetch_all(&self, pool: &SqlitePool) -> Result<Vec<T>, sqlx::Error> {
        let mut qb = self.build_qb();
        qb.build_query_as::<T>().fetch_all(pool).await
    }

    pub async fn fetch_optional(&self, pool: &SqlitePool) -> Result<Option<T>, sqlx::Error> {
        let mut qb = self.build_qb();
        qb.build_query_as::<T>().fetch_optional(pool).await
    }
}
