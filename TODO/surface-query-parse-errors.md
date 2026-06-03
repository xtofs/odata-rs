# Surface query-string parse errors as 400

`crates/service/src/builder.rs::parse_query` currently calls
`QueryOptions::parse(...).unwrap_or_default()`, which silently swallows any
`odata_url::ParseError` and hands the route a default (empty) `QueryOptions`.

The motivating bug: a URL like `/rooms?$top=2$skip=1` (missing the `&`
separator) parses as a single pair `$top = "2$skip=1"`, which then fails
`u64::parse` and produces `ParseError::InvalidInteger`. The current router
discards the error and returns the unpaged full list — making it look like
`.page` is broken when in fact the request was malformed.

## What "fix" looks like

1. Change `parse_query` to return `Result<QueryOptions, ParseError>`.
2. In each of the ten route closures in `builder.rs` (GET/POST on the
   collection route, GET/PATCH/DELETE on the entity route, and the four
   contained-nav variants), early-return a `400 Bad Request` response when
   parsing fails, *before* constructing the context and calling the
   dispatcher.

Response body should include the `ParseError` message so the client can see
which option was malformed (e.g. `"invalid integer for top: 2$skip=1"`).

## Why not done inline

The change is mechanical but touches every route closure (10 sites), each
already nested two deep. Worth a focused pass with a small helper or
middleware rather than scattering `match` arms.

## Possible alternative: middleware

A `tower::Layer` that runs `QueryOptions::parse` on the raw query upfront and
short-circuits to 400 on error, then stashes the parsed `QueryOptions` in
request extensions for the handler to pick up. That would centralize the
check in one place and let the route closures stay focused on context
construction — at the cost of a request-extension dance.

Pick whichever is cleaner once we look at the closures again.
