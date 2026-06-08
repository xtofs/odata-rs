# TODO

One file per open item, so each is a focused context unit for both humans and agents. Add new entries by dropping a `kebab-case.md` file alongside the others and linking it below.

CSDL / EDM modeling work happens in the `csdl-edm` crate (see [crates/csdl-edm/TODO/csdl-coverage-gaps.md](../crates/csdl-edm/TODO/csdl-coverage-gaps.md)). This list covers the rest of the workspace.

## Filter expressions

- [filter-function-signatures](filter-function-signatures.md) — validate `$filter` function calls against their signatures.
- [filter-lambda-any-all](filter-lambda-any-all.md) — support `any`/`all` lambdas in `$filter` expressions.

## Service / router

- [service-metadata-endpoint](service-metadata-endpoint.md) — implement `GET /$metadata`; the canonical version emits the full schema plus `Org.OData.Capabilities.V1.*` annotations derived from registered handlers.
- [surface-query-parse-errors](surface-query-parse-errors.md) — return `400 Bad Request` for malformed query strings instead of silently falling back to default `QueryOptions`.
