# AGENTS.md

Conventions for agents (and people) working in this workspace. Architectural defaults that are easy to violate by accident; please respect or argue explicitly before deviating.

## Crate responsibilities (don't blur the boundaries)

- **`csdl-edm`** owns CSDL parsing (XML and JSON), serialization, the syntactic CSDL model (`csdl::*`), the resolver, and the resolved semantic EDM model graph (`edm::*`). It is the **sole** owner of those responsibilities in this workspace. The crate is maintained independently and evolves **additively** — its API is settled.
- **`odata-rs-url`** owns OData URL and query-string parsing.
- **`odata-rs-service`** owns the axum-based router, handler contexts, and the builder API.

If something belongs in csdl-edm (a new CSDL element, a resolver capability, a serialization concern), it is added in csdl-edm, not shadowed in another crate.

## Service-side projections are *internal* to the service crate

The service crate maintains an internal projection of the EDM model in `crates/service/src/schema_view.rs`. It exists for one reason: it is the **router's working set** — just the slice of the model the router needs to assemble routes and emit gap warnings (entity sets, target entity-type short name per set, contained navigation properties on those targets).

Rules:

1. **`schema_view` is `pub(crate)`.** It is not re-exported. Public consumers see `csdl_edm::edm::Model` (or, for the static-CSDL convenience case, `ODataServiceBuilder::from_csdl(&str)`).
2. **`schema_view` is built from `csdl_edm::edm::Model`** — *never* by re-parsing CSDL strings, walking `csdl::*` syntactic nodes, splitting `"Collection(X)"` or `"Namespace.X"`, or otherwise re-implementing work the resolver has already done. The semantic graph delivers short names and Arc-resolved references; use them.
3. **`schema_view` does not validate.** If a resolver check belongs anywhere in the workspace, it belongs in csdl-edm. The projection trusts its input.
4. **`schema_view` adds no new concepts.** Every field is a renaming or projection of an `edm::*` field. If you're tempted to add a concept that isn't already in the EDM model, that's a signal it probably belongs in csdl-edm — or that the router's needs have changed enough that the projection should grow.

## Public API surface of the service crate

`ODataServiceBuilder` is the entry point. The two constructors are:

- `ODataServiceBuilder::new(&csdl_edm::edm::Model)` — when the caller has the EDM model in hand.
- `ODataServiceBuilder::from_csdl(&str)` — convenience: parse → resolve → project, in one call. Returns `crate::Result<Self>`.

`crate::{Error, Result}` are re-exports from `csdl_edm`. Errors that originate from the resolver are folded into `Error::Csdl(String)` for now (until csdl-edm exposes a richer error union, or we decide we need one).

## When the projection should grow vs. when consumers should reach for `edm::Model`

The projection grows when the *router* gains a new structural concern (e.g. singletons, function imports, multi-namespace schemas). When other features need access to richer model information — e.g. `$metadata` emission, `$select` validation against properties, `$filter` translation — those features consume `csdl_edm::edm::Model` directly. They do not extend the projection.

A useful sniff test: if the new field would never be referenced inside `crates/service/src/builder.rs::assemble_router`, it doesn't belong in `schema_view`.

## TODO files

One file per open item under `TODO/`, with `TODO/README.md` as the index. CSDL/EDM modeling work belongs in `crates/csdl-edm/TODO/`, not here.

## Linking changes

When you change the projection or the public builder surface, also update [ARCHITECTURE.md](ARCHITECTURE.md). The architecture doc is the durable design record; this file is the operating-rules cheat sheet.
