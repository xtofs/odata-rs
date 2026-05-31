# OData URL ABNF Analysis

This crate intentionally sits at the boundary between generic URI parsing and OData-specific interpretation.

The `url` crate handles RFC 3986 parsing, path splitting, query separation, fragment extraction, and percent-encoding normalization. This crate then maps the parsed URL into typed OData fields.

The useful distinction is three layers:

| Layer                 | Responsibility                                                                             |
| --------------------- | ------------------------------------------------------------------------------------------ |
| URL parsing           | Scheme, authority, path splitting, query separation, fragment extraction, percent-decoding |
| ODataQuery projection | Typed fields exposed to the service layer                                                  |
| Deferred ABNF         | Rules that are neither projected into `ODataQuery` nor interpreted by the URL parser yet   |

## URL Parsing Boundary

These ABNF concerns are handled by the `url` crate boundary rather than by OData-specific parsing here:

| ABNF rule or concern                 | Status                | Notes                                                            |
| ------------------------------------ | --------------------- | ---------------------------------------------------------------- |
| `query`                              | handled by dependency | Query string is split from the path before OData interpretation  |
| `fragment`                           | handled by dependency | Fragment is extracted before OData interpretation                |
| `pchar`, `pct-encoded`, `unreserved` | handled by dependency | The URL crate owns URI character normalization                   |
| hierarchical path splitting          | handled by dependency | Path segments are exposed for OData interpretation               |
| percent-encoding normalization       | handled by dependency | OData sees the decoded query values and normalized path segments |

## ODataQuery Projection

These ABNF rules are already projected into the public `ODataQuery` shape:

| ODataQuery property      | ABNF rule(s)             | Status                    |
| ------------------------ | ------------------------ | ------------------------- |
| `resource_path.segments` | `resourcePath`           | implemented               |
| `each`                   | `each`                   | implemented               |
| `count`                  | `count` path marker      | implemented               |
| `r#ref`                  | `ref`                    | implemented               |
| `value`                  | `value`                  | implemented               |
| `select`                 | `select`                 | partially implemented     |
| `filter`                 | `filter` / `commonExpr`  | partially implemented     |
| `expand`                 | `expand`                 | partially implemented     |
| `top`                    | `top`                    | implemented               |
| `skip`                   | `skip`                   | implemented               |
| `orderby`                | `orderby`                | partially implemented     |
| `inlinecount`            | `inlinecount`            | implemented               |
| `custom`                 | `customQueryOption`      | implemented               |
| `fragment`               | `context` / URL fragment | implemented by dependency |

`partially implemented` means the property is present and typed, but the internal grammar is still incomplete. For example, `select`, `expand`, and `orderby` are still coarse. `filter` now parses a structured `commonExpr` subset (logical, comparison, arithmetic, unary, function calls, literals, and grouped expressions), but deferred constructs like lambda operators are not yet supported.

## Deferred ABNF

These rules are not yet represented in `ODataQuery` and are not consumed by the current parser:

| ABNF rule                                                    | Status   | Reason it is deferred                            |
| ------------------------------------------------------------ | -------- | ------------------------------------------------ |
| key predicates, navigation, bound operations, function calls | deferred | resource-path semantics are still coarse-grained |
| lambda operators (`any`, `all`)                              | deferred | requires nested expression parsing               |
| `$search`                                                    | deferred | separate search grammar not yet implemented      |
| `$compute`                                                   | deferred | compute expression grammar not yet implemented   |

This document should stay aligned with `crates/url/docs/abnf-coverage.md`: if a rule is missing from both the projection table and the deferred table, it is a gap that has not been assigned yet.

Percent-encoding behavior remains owned by the `url` crate boundary; OData-specific parsing in this crate only consumes the decoded query pairs it receives.
