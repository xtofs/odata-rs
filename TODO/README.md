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
- [structured-warnings](structured-warnings.md) — emit reader/parser warnings through a structured `Diagnostic` channel instead of silently dropping suspect input.
- [utf8-aware-column-counting](utf8-aware-column-counting.md) — `Location::column` counts bytes, not Unicode characters; matters for non-ASCII CSDL.
- [zero-copy-token-strings](zero-copy-token-strings.md) — `CsdlToken` claims `Cow<'a, str>` but currently always allocates; route reads through the borrowed input buffer.

## Service / router

- [surface-query-parse-errors](surface-query-parse-errors.md) — return `400 Bad Request` for malformed query strings instead of silently falling back to default `QueryOptions`.
