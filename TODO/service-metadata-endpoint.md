# `$metadata` endpoint

OData services expose a CSDL document at `GET /$metadata` describing the data
model the service offers (OData v4 §11.1.2). `ODataServiceBuilder` doesn't
implement that route today.

The interesting design tension is **what to serve**:

- The **input CSDL** the user handed the builder. Faithful to designed intent;
  silently lies when the implementation doesn't cover every declared entity
  set / nav prop / operation.
- An **as-implemented CSDL** derived from the builder's `configs` map.
  Honest, but requires a CSDL writer (we have a CSDL *reader* only).
- The **input CSDL + `Capabilities` annotations** that declare which ops are
  actually supported per entity set / nav prop. This is the spec-canonical
  answer — clients like PowerQuery / Excel / OData query tools read the
  `Org.OData.Capabilities.V1.*` vocabulary and adjust their UI accordingly.

The recommended target is the third option. The first option is a reasonable
stepping stone since it requires the least new machinery; the second is
strictly dominated by the third and not worth implementing on its own.

## Recommended near-term shape

Serve the input CSDL verbatim from `GET /$metadata`, plus emit a
`tracing::warn!` at build time when gaps exist so the user knows the served
document overstates the implementation. This is small and unblocks clients
that want any metadata at all; the canonical version follows when the
supporting pieces below land.

Concrete near-term work:

- Plumb the source CSDL bytes through to `ODataServiceBuilder`. Options:
  - `ODataServiceBuilder::new_with_csdl(schema, csdl: impl Into<Arc<str>>)`
    alongside the current `new(schema)`. Keeps the EDM crate untouched.
  - Have `Schema::from_csdl` retain a reference to the original input on the
    `Schema` itself. Smaller call-site change but pushes a service concern
    into the EDM crate.
- Register a `GET /{es_name_root}/$metadata` (or `/$metadata` at the service
  root — pick the route shape after looking at how the existing routes are
  rooted) returning the stored bytes with `Content-Type:
  application/xml;charset=utf-8`.
- One-time `tracing::warn!` during `.build()` when the configs map has any
  gap and a `$metadata` source is configured — e.g.
  `served $metadata declares 4 entity sets but only 3 have full CRUD
  registered; consider implementing the Capabilities annotations path to
  declare this honestly`.

## Requirements for the canonical (option 3) path

These are independent pieces. The first four belong in `csdl-edm`; the last
two are service-crate work that builds on top.

1. **CSDL writer** — `csdl_edm::serialization` already emits CSDL XML and
   JSON (see `crates/csdl-edm/src/serialization.rs`). Confirm it covers the
   round-trip cases we need before relying on it for `$metadata`.
2. **`Capabilities` vocabulary loading** — `csdl-edm`'s coverage matrix lists
   `Term` parse+serialize as done and resolver as partial. Confirm that the
   subset of `Org.OData.Capabilities.V1` we want to emit against (
   `UpdateRestrictions`, `DeleteRestrictions`, `InsertRestrictions`,
   `ReadRestrictions`) resolves end-to-end.
3. **`Annotation` reading on `EntitySet` / `Singleton` / `NavigationProperty`**
   — `csdl-edm`'s `csdl::EntitySet` already has `annotations: Vec<Annotation>`;
   verify the resolver carries them through to the semantic-graph form.
4. **`Annotation` writing** — included in (1), but worth verifying with a
   round-trip test for inline-attribute constants on a Capabilities
   annotation specifically.
5. **`CapabilityProfile` derivation** (service crate) — a helper that consumes
   the `configs` map and yields a `Vec<Annotation>` per entity set / nav
   prop, populating `UpdateRestrictions { Updatable: false }` etc. for the
   gaps. This is the bridge from "what's registered" to "what to emit".
6. **Composition** (service crate) — at `$metadata` request time, splice the
   `CapabilityProfile` annotations into the served `Schema` and serialize
   via the writer. Decide once whether splicing happens at build time (cache
   the rendered bytes) or per request (always reflect current state) — for
   a static schema, build-time is fine; for hot-reloadable schemas, lazy.

## Open questions

- Should the served metadata always be the schema as-built, or should we
  allow the user to supply their own pre-rendered CSDL bytes for cases where
  they want to customize what's exposed beyond what the framework derives?
- Route shape: `GET /$metadata` at service root, or per-namespace? OData
  defaults to service-root; this matches what most clients probe.
- Content negotiation: clients may ask for `application/xml` or
  `application/json`. CSDL JSON is an OData v4.01 thing — out of scope for
  v1 of this endpoint, but worth noting so we don't paint into a corner.
- Caching: `ETag` / `Last-Modified` headers on the metadata response are a
  small win for chatty clients. Add once the basic endpoint works.
