//! OData URL path rewriting middleware for axum.
//!
//! OData uses a *subsegment* key syntax — `EntitySet('key')` — where the key
//! is parenthesized inside the same path segment as the entity-set name.
//! Standard HTTP routers dispatch on `/`-delimited segments and cannot natively
//! match this pattern.
//!
//! This crate provides a middleware that rewrites OData subsegment paths into
//! segment paths *before* they reach the router:
//!
//! ```text
//! /Rooms('oak-204')/Printers('hp-42')
//!   →  /Rooms/__key__/oak-204/Printers/__key__/hp-42
//! ```
//!
//! The sentinel `__key__` is never a legal OData identifier, so there is no
//! collision with real path segments. The router registers routes using the
//! rewritten form (`/Rooms/__key__/{id}/Printers/__key__/{nav_id}`) and axum's
//! standard `{param}` extraction works as usual.

use axum::{
    Router,
    body::Body,
    http::{Request, Uri},
    middleware::{self, Next},
    response::Response,
};

/// The original OData URI before path rewriting.
///
/// Inserted into request extensions by [`odata_path_rewrite`] so that response
/// builders can reconstruct spec-compliant `@odata.id`, `@odata.editLink`, and
/// `@odata.nextLink` annotations using the canonical OData form.
#[derive(Clone, Debug)]
pub struct OriginalODataUri(pub Uri);

/// Extension trait that applies the OData path-rewrite middleware to an axum
/// [`Router`].
///
/// # Example
///
/// ```rust,no_run
/// use axum::Router;
/// use odata_routing::ODataRouterExt;
///
/// let app: Router = Router::new()
///     // ... register routes using __key__/{param} patterns ...
///     .with_odata_rewrite();
/// ```
pub trait ODataRouterExt<S> {
    fn with_odata_rewrite(self) -> Self;
}

impl<S> ODataRouterExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_odata_rewrite(self) -> Self {
        self.layer(middleware::from_fn(odata_path_rewrite))
    }
}

/// Axum middleware that rewrites OData subsegment key syntax into segment form.
///
/// Stashes the original URI in request extensions as [`OriginalODataUri`].
pub async fn odata_path_rewrite(mut req: Request<Body>, next: Next) -> Response {
    let original = req.uri().clone();
    req.extensions_mut()
        .insert(OriginalODataUri(original.clone()));

    let rewritten_path = rewrite_odata_path(original.path());

    if rewritten_path != original.path() {
        let rewritten_uri = if let Some(query) = original.query() {
            format!("{rewritten_path}?{query}")
        } else {
            rewritten_path
        };

        if let Ok(uri) = rewritten_uri.parse::<Uri>() {
            *req.uri_mut() = uri;
        }
    }

    next.run(req).await
}

/// The sentinel segment inserted between the entity-set name and the key value.
///
/// Exported so the service crate can build route patterns like
/// `format!("/{es_name}/{KEY_SEGMENT}/{{id}}")`.
pub const KEY_SEGMENT: &str = "__key__";

/// Rewrite an OData path from subsegment key form to segment form.
///
/// Each `Segment(key)` occurrence becomes `Segment/__key__/key`.
/// Segments without parenthesized keys pass through unchanged.
///
/// # Examples
///
/// ```
/// use odata_routing::rewrite_odata_path;
///
/// assert_eq!(
///     rewrite_odata_path("/Rooms('oak-204')/Printers('hp-42')"),
///     "/Rooms/__key__/oak-204/Printers/__key__/hp-42"
/// );
///
/// // Plain segments are unchanged
/// assert_eq!(rewrite_odata_path("/Rooms"), "/Rooms");
/// ```
pub fn rewrite_odata_path(path: &str) -> String {
    let mut result = Vec::<String>::new();

    for segment in path.trim_start_matches('/').split('/') {
        if let Some(open) = segment.find('(') {
            if segment.ends_with(')') {
                let prefix = &segment[..open];
                let key = &segment[open + 1..segment.len() - 1];

                if !prefix.is_empty() {
                    result.push(prefix.to_string());
                    result.push(KEY_SEGMENT.to_string());
                    // Strip OData string-literal quotes so the handler receives
                    // a clean key value matching what segment-style routes give.
                    let stripped = key
                        .strip_prefix('\'')
                        .and_then(|s| s.strip_suffix('\''))
                        .unwrap_or(key);
                    result.push(stripped.to_string());
                    continue;
                }
            }
        }

        result.push(segment.to_string());
    }

    format!("/{}", result.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_segment_unchanged() {
        assert_eq!(rewrite_odata_path("/Rooms"), "/Rooms");
    }

    #[test]
    fn collection_unchanged() {
        assert_eq!(rewrite_odata_path("/Rooms"), "/Rooms");
    }

    #[test]
    fn single_key() {
        assert_eq!(
            rewrite_odata_path("/Rooms('oak-204')"),
            "/Rooms/__key__/oak-204"
        );
    }

    #[test]
    fn integer_key() {
        assert_eq!(rewrite_odata_path("/Rooms(42)"), "/Rooms/__key__/42");
    }

    #[test]
    fn nested_contained_nav() {
        assert_eq!(
            rewrite_odata_path("/Rooms('oak-204')/Printers('hp-42')"),
            "/Rooms/__key__/oak-204/Printers/__key__/hp-42"
        );
    }

    #[test]
    fn key_then_plain_nav() {
        assert_eq!(
            rewrite_odata_path("/Rooms('oak-204')/Printers"),
            "/Rooms/__key__/oak-204/Printers"
        );
    }

    #[test]
    fn path_marker_after_key() {
        assert_eq!(
            rewrite_odata_path("/Rooms('oak-204')/$count"),
            "/Rooms/__key__/oak-204/$count"
        );
    }

    #[test]
    fn path_marker_after_collection() {
        assert_eq!(
            rewrite_odata_path("/Rooms/$count"),
            "/Rooms/$count"
        );
    }

    #[test]
    fn root_path() {
        assert_eq!(rewrite_odata_path("/"), "/");
    }

    #[test]
    fn empty_key() {
        // Degenerate case — `Rooms()` — preserve as-is since empty key is invalid
        assert_eq!(rewrite_odata_path("/Rooms()"), "/Rooms/__key__/");
    }

    #[test]
    fn quoted_key_with_escaped_quote() {
        assert_eq!(
            rewrite_odata_path("/Rooms('it''s')"),
            "/Rooms/__key__/it''s"
        );
    }

    #[test]
    fn deeply_nested() {
        assert_eq!(
            rewrite_odata_path("/A('1')/B('2')/C"),
            "/A/__key__/1/B/__key__/2/C"
        );
    }
}
