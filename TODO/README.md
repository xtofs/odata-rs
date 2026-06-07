# TODO

One file per open item, so each is a focused context unit for both humans and agents. Add new entries by dropping a `kebab-case.md` file alongside the others and linking it below.

## CSDL / EDM model

- [actions-and-functions](actions-and-functions.md) — wire CSDL 4.01 Actions, Functions, and their imports into the model; today they parse without error but are dropped.
- [annotations-on-expressions](annotations-on-expressions.md) — handle CSDL annotations attached to annotation-bearing expression elements.
- [edmx-support](edmx-support.md) — parser currently handles only `edm:Schema` and children; extend to `edmx:Edmx` envelopes.
- [element-metadata](element-metadata.md) — extend the element `meta` table beyond parent rules to cover required/allowed attributes and annotation acceptance.
- [term-definitions](term-definitions.md) — handle `<Term>` vocabulary declarations so schemas like `Org.OData.Core.V1` round-trip with non-empty Term lists.
- [typed-constant-variants](typed-constant-variants.md) — replace `String`-stored CSDL constant expressions with lossless typed variants where std lacks a native type.

## Filter expressions

- [filter-function-signatures](filter-function-signatures.md) — validate `$filter` function calls against their signatures.
- [filter-lambda-any-all](filter-lambda-any-all.md) — support `any`/`all` lambdas in `$filter` expressions.

## Reader / diagnostics

- [semantic-graph](semantic-graph.md) — semantic graph vs syntactic tree representation.
- [edm-structured-warnings](edm-structured-warnings.md) — emit reader/parser warnings through a structured `Diagnostic` channel instead of silently dropping suspect input.
- [utf8-aware-column-counting](utf8-aware-column-counting.md) — `Location::column` counts bytes, not Unicode characters; matters for non-ASCII CSDL.

## Service / router

- [service-metadata-endpoint](service-metadata-endpoint.md) — implement `GET /$metadata`; canonical target is full schema + `Org.OData.Capabilities.V1.*` annotations derived from registered handlers. Lists the supporting work (CSDL writer, capabilities vocabulary, annotation emission, profile derivation).
- [surface-query-parse-errors](surface-query-parse-errors.md) — return `400 Bad Request` for malformed query strings instead of silently falling back to default `QueryOptions`.
