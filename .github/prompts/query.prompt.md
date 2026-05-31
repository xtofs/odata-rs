---
name: second crate
description: second crate and project organization for the odata-rs workspace
---

currently we have a single public EDM crate, but the project needs to grow to support whole OData services in rust. The prompt should produce a concrete workspace plan rather than open-ended brainstorming.

## project structure

on a macro level we need to think about the project structure. I am proposing

```
odata-rs/            # public crate
crates/
  edm/               # crate name: odata-edm     EDM model and CSDL parsing; migrate the existing EDM crate here with git mv
  url/               # crate name: odata-url     URL parsing for consumption by HTTP actions
  serde/             # crate name: odata-serde   payload serialization now; deserialization is out of scope for this iteration
  service/           # crate name: odata-service additional glue for service implementations
```

this gives us
• Clean internal organization
• Predictable crate names
• Freedom to publish them later without renaming
• A single public API surface (odata-rs)

Dependency direction must stay explicit: `odata-edm` has no internal deps; `odata-url` depends only on `url`; `odata-serde` depends on `odata-edm`; `odata-service` depends on `odata-edm`, `odata-url`, and `odata-serde`; `odata-rs` re-exports the public API surface and contains no domain logic.

The `odata-rs` crate must re-export the public API from the internal crates so existing consumers can adopt the workspace layout without breaking their import paths.

## url crate

We need a second crate that parses the request URL and hands a typed `ODataQuery` value to the service implementation. `ODataQuery` is pure data: a parsed, typed representation of the URL with no behavior attached.

It is what the library hands the service implementation in a similar way that a generic web server receives a URL and hands a parsed URL type to service actions. What the service has to do with this is of no concern to this crate.

This crate should depend on https://crates.io/crates/url and build `ODataQuery` on the parsed result of that crate.

On the surface, an OData query is relatively simple, "just" a URL with a path and well-defined query options. But the syntax https://github.com/oasis-tcs/odata-abnf/blob/main/abnf/odata-abnf-construction-rules.txt is relatively long, partially because it redefines parts of RFC 3986.

This requires a concrete initial scope: implement `resourcePath`, `$filter`, `$select`, `$expand`, `$top`, `$skip`, `$orderby`, and `$count`; defer `$search`, `$compute`, and lambda operators. The `ODataQuery` shape for this first pass should at minimum expose `resource_path`, `filter`, `select`, `expand`, `top`, `skip`, `orderby`, `count`, and `custom` query options, and the parser should return `Result<ODataQuery, ParseError>` without panicking.

The parser must treat percent encoding carefully. Document each divergence from RFC 3986 percent-encoding behavior, and add unit tests for every divergent case.

This analysis should be documented in `crates/url/docs/abnf-coverage.md` as a table mapping each ABNF rule name to its status (`implemented`, `deferred`, or `not-applicable`) and the Rust module or function responsible for it so the parser code's pedigree can be traced back to the ABNF.
