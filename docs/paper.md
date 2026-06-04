# An Architecture for a Production-Ready OData Service Library in Rust

*An architecture proposal.*

---

## 1. Abstract

OData is a mature standard for typed, queryable REST APIs. Its reference implementation — Microsoft's `Microsoft.AspNetCore.OData` — is deeply entwined with the .NET runtime: attribute-driven dispatch, dependency injection, and a tight coupling to LINQ / `IQueryable` / Entity Framework. None of those facilities exist in idiomatic Rust. This paper describes **Temper**, a proposed architecture for an OData service library on Rust that translates the protocol — not the framework — and is structured around a small number of compile-time-typed surfaces, an explicit translation layer between parsed query options and data access, and programmatic configuration in lieu of attributes and reflection.

The name *Temper* is a working code name and a placeholder for the architecture under discussion. It is used throughout this paper to avoid leaning on either the spec's vocabulary ("the OData server") or on any concrete implementation name.

## 2. Audience and Scope

This paper is intended for readers who already know what OData is and have at least passing familiarity with the .NET implementation. It is an architecture proposal, not a tutorial or an API reference. It enumerates the questions a serious port has to answer, the answers Temper proposes, and the open questions that remain.

URL examples in this paper use OData's *key-as-segment* notation — `/Rooms/oak-204/Printers` — rather than the parenthesized form `/Rooms('oak-204')/Printers`. The architecture must ultimately support both; the segment form is easier to read for an audience that should not have to learn OData's URL syntax to follow the argument.

## 3. OData Idiosyncrasies That Matter to a Rust Implementation

OData is often described as "REST with conventions," but several of its conventions interact poorly with naive ports. This section enumerates the ones with the largest architectural blast radius.

### 3.1 `$select` is a *response* projection, not a query constraint

OData v4 URL Conventions §5.1.4 defines `$select` as the option that controls which structural properties appear *in the response*. It does **not** restrict what other system query options may reference. `$filter` (§5.1.1) and `$orderby` (§5.1.5) operate against the full entity type regardless of `$select`; key access (`/Rooms/oak-204`) and parent-key matching for contained navigation (`/Rooms/oak-204/Printers`) likewise operate on the underlying row.

The implication for a SQL-backed implementation: the engine must always read enough columns to satisfy WHERE / ORDER BY / key matching, and `$select` is applied at *output shaping* time — not, in general, at the SQL projection. This is precisely backwards from how a casual reader of `$select` would translate it. An implementation that conflates the two — emitting `SELECT a, b` whenever a client says `$select=a,b` — will silently produce wrong filters and broken keys.

This single fact has structural consequences that ripple through the rest of the architecture: the data-access layer cannot use `$select` as its primary projection signal, the response builder must be able to shape output independently of the rows that came back, and the trade-off between "let the database do less work" and "let the application code stay simple" must be made explicit per call site rather than hidden behind a single rewrite rule.

### 3.2 The response envelope is not vanilla JSON

OData responses are JSON but they are not arbitrary JSON. Top-level collections are wrapped in `{"value": [...]}`. Annotations like `@odata.context`, `@odata.count`, `@odata.nextLink`, `@odata.id`, and `@odata.type` appear as sibling keys to structural properties, each carrying meaning that vanilla JSON parsers will happily strip if you reshape carelessly. Any "just throw it at a generic JSON serializer" pipeline will quietly emit a payload that other OData clients refuse to interpret.

This shapes the response layer of the architecture in two ways:

- **Output shaping is explicit and OData-aware.** The output side cannot be reduced to "call `serde_json::to_value` on the row and ship it." Envelope construction, annotation emission, and `$select` projection sit behind a single response builder so the OData-specific shaping is in one place.
- **Per-row shaping is a second pass over query results.** This is the same conclusion §3.1 forces from the input side; the two reinforce each other. Whatever the data-access layer returns, the response layer is responsible for shaping it into compliant OData JSON.

### 3.3 The metadata document is structured, not free-form

CSDL XML is what OData clients consume to learn the schema. It carries entity types, navigation properties (with containment markers), key cardinality, primitive type names, and annotations. The shape constrains the rest of the architecture: handlers can be type-checked against the schema *at service startup*, before any request arrives. This is the lever that makes "schema-first" a real strategy rather than a slogan (see §7.3).

## 4. What Doesn't Translate from .NET

The .NET OData stack is a sophisticated piece of engineering, but a large fraction of its surface area derives from runtime facilities that Rust deliberately does not provide. Calling out the non-translatable pieces up front prevents the architecture from drifting into Rust shapes that try to imitate framework behavior.

### 4.1 The framework is the dispatcher

ASP.NET Core OData is a "don't call me, I'll call you" framework: routes, parameter binding, model validation, content negotiation, and convention-based controllers are wired up by reflection and attributes (`[EnableQuery]`, `[ODataRouteComponent]`, `[FromODataUri]`, …). The runtime introspects attributes at startup, builds a routing table, and dispatches into controller methods whose signatures it has matched by name, type, and decoration.

Rust does not have runtime attribute introspection in this sense. Procedural macros can produce code at compile time, but they cannot weave dispatch logic at runtime against handlers a downstream user has declared with unknown shapes. Attempting to recreate the .NET model would mean either (a) requiring users to register handlers through an elaborate macro-based DSL, or (b) erasing handler types into trait objects and re-introducing dynamic dispatch costs that idiomatic Rust avoids. Both are dead ends for an architecture aimed at production use.

Temper takes the opposite stance. Handlers are ordinary `async` functions with a context argument and an application-state argument. Registration is fluent and explicit rather than reflective. The router assembled at the end of registration is a `Service`-shaped value in whatever HTTP-server ecosystem the integration targets, composing with the surrounding middleware the same way every other service does.

### 4.2 No `IQueryable` / EF baseline

The .NET implementation leans on `IQueryable<T>` and Entity Framework as its primary translation target: a parsed `$filter` expression tree is grafted onto a LINQ tree and EF turns the result into SQL. This is enormously productive in ecosystems that already have EF, and a tar pit in ecosystems that don't. There is no equivalent universal query abstraction in Rust — the available libraries (Diesel, SQLx, SeaORM, raw drivers) all sit at different abstraction levels with incompatible compile-time guarantees.

Rather than pick a winner, the architecture exposes the *parsed* query options as data and defines a clear interface between them and any data-access backend. Backend adapters — SQL builders, document-store adapters, in-memory closures, transport-level proxies — sit behind that interface. The rest of the architecture does not know or care which adapter is in use. This separation is the subject of §10.

### 4.3 No ambient dependency-injection container

.NET handlers receive their dependencies through constructor injection or `[FromServices]`. The DI container is part of the framework; users describe what services exist and the runtime wires them up.

Temper does not provide a DI container. Instead, a single application-state value `S` is attached to the service at construction time and passed to every handler as an explicit argument. Handlers see exactly what they were given; there is no implicit lookup, no hidden lifetime management, no "where did this service come from" question to answer when reading a handler. State that's expensive to clone is wrapped in a reference-counting pointer at the call site, and the per-request cost is one pointer copy.

## 5. Programmatic Configuration vs. Conventions and Attributes

The .NET implementation exposes most of its tunable behavior through attributes (`[Page(MaxTop = 100)]`, `[EnableQuery(MaxOrderByNodeCount = 5)]`, …) and routing conventions. These are forms of *declarative configuration interpreted by the framework at runtime*. They are powerful in ecosystems with rich reflection but they have well-known costs: behavior depends on attributes that may be far from the code they affect, defaults are hidden inside the framework, and what the service actually does is the composition of "what the user wrote" with "what conventions did" — a composition that is hard to read out of source.

Temper's principle is that all of this is *code that the developer writes*, in the same source file as the handler whose behavior it controls. The mechanism is uniform across the surface area; what follows are three examples of what it looks like.

### 5.1 State is a typed argument, not an ambient lookup

State attachment is a single typed step at service construction; handlers receive the state value as an argument. The type system enforces that state is attached before any handler is registered, so the handler's signature and the service's state type cannot drift apart. There is no "did I register this service with the container?" question to answer at runtime — the compiler answers it.

### 5.2 Allowlists are first-class arguments

Where a .NET controller would attribute-decorate an action with `[Page(MaxTop = 100)]` or `[OrderByModelConfiguration]`, a Temper handler passes the equivalent constraints as values to the query builder or response shaper it invokes. An orderby allowlist might be `Allowed::Only(&["id", "name"])`, a select allowlist similarly, a page size cap a small struct passed alongside. These are values, not annotations: they live next to the handler code, they are visible to the reader, and the type system enforces their shape.

### 5.3 Allowlists exist for the same reason attributes did

This is worth saying out loud: the *intent* of `[EnableQuery(MaxOrderByNodeCount=5)]` is to bound what an external client can demand of the service. Temper does the same job with the same intent — the only thing that changes is *where the bound is spelled*. Moving the constraint from a framework-interpreted attribute into a builder argument does not weaken the constraint; it makes the constraint readable from the handler code without consulting the framework's interpretation rules.

The general principle: anything a .NET implementation would express by decorating code, Temper expresses by *calling* code. The trade-off is verbosity in exchange for locality of reasoning, and the architecture takes the trade.

## 6. Crate Decomposition (Proposal)

Temper proposes a four-crate decomposition:

- **edm** — CSDL XML parser and resolved schema model. No HTTP, no SQL, no async. Loadable in any context — code generators, validators, documentation tools, IDE plugins.
- **url** — OData URL and query-string parser. Produces structured values for the parsed URL and for the system-query-option subset most handlers need (`$select`, `$filter`, `$expand`, `$orderby`, `$top`, `$skip`, `$count`, custom options). No HTTP, no schema knowledge.
- **service** — service composition: the router, handler context types, the builder API, and the integration to a chosen HTTP-server ecosystem. Backend-adapter crates plug in here.
- **umbrella** — re-exports the three under matching features for downstream users who want the full stack.

Three reasons for the split:

1. **Single-responsibility crates with minimal dependencies.** Each layer has a clear contract with the next; a downstream user who needs only CSDL parsing should not transitively pull HTTP and SQL dependencies.
2. **Integrations are swappable.** The service crate has an integration choice (which HTTP server, which middleware ecosystem); the data-access layer has another (which database, which query abstraction). Both choices are explicit, neither bleeds into the URL or EDM layers.
3. **Compile times stay tractable.** Optional features allow downstream binaries to include only what they use.

This is a proposal, not a commitment. The boundaries may shift as work on `$filter` lowering and `$expand` materialization clarifies which surfaces need to be shared across more than one crate.

## 7. The EDM Layer

CSDL is the foundation. Everything else in Temper depends on a resolved schema being available by the time the service starts. Parsing CSDL is therefore one of the load-bearing parts of the architecture, not a side concern.

### 7.1 Pass 1: streaming reader → syntactic model

A streaming XML reader produces a token stream. A small stack-machine builder consumes the stream and produces a *syntactic* model: every reference is still a string, every type is still spelled out by name, aliases are preserved as-written. This stage is intentionally loose — it accepts the CSDL exactly as written and does not enforce cross-schema consistency. Pragmatically it sits one step above a concrete syntax tree: it has structure but it has not resolved meaning.

The decision to expose this intermediate stage matters. Linting tools, documentation generators, and CSDL migration utilities all need access to the schema as it was written, including aliases and forward references. A library that only exposes the resolved view forces those tools to re-implement the parser.

### 7.2 Pass 2: resolver → resolved model

A multi-pass resolver turns the syntactic model into a *resolved* model:

1. Register all schemas; assign typed IDs to every named entity, complex type, function, action, and enumeration.
2. Resolve every string reference into a typed handle. Aliases canonicalize to their fully-qualified names; overloads pick their concrete signature; navigation properties resolve to their target entity types.
3. Validate the resulting graph — every reference points somewhere, every key cardinality is sane, every navigation property has a matching binding.

The result is the surface that the rest of the architecture consumes. The two-pass split keeps each pass simple and makes failure modes legible: an XML error is unambiguously a syntactic problem, a missing-reference error is unambiguously a semantic problem, and the user sees exactly which pass produced the failure.

### 7.3 Schema-first checks at service start

Because the schema is fully resolved before any service is constructed, the service builder validates handler registrations against the model *before any request arrives*:

- Every entity set named in a handler registration must exist in the schema.
- Every contained navigation property registered with a handler must in fact be marked `Contained="true"` on the parent entity type.
- Entity sets present in the schema that have *no* registered handlers produce warnings — not errors, because partial coverage is a legitimate state during development.
- Operations (functions and actions) referenced by handlers must match their CSDL signatures.

These checks fire at construction time. There is no class of "I deployed and only discovered at first request that I'd typoed a navigation property name" failure mode. This is the substantive payoff of schema-first: most categories of registration bug become impossible to ship.

## 8. URL Parsing as a Typed Surface

The URL layer parses OData URLs into structured values. Two surfaces matter to downstream consumers:

- A full parsed URL value, carrying the resource path, path markers (the `$value`, `$count`, `$ref` segments), the system query options, and any non-`$`-prefixed custom options.
- A reduced view containing only the system query options most handlers need.

`$filter` deserves its own paragraph. It is parsed into a typed expression tree, but parsing is separated from *lowering* the expression to any particular backend. This separation follows directly from §4.2's non-translation point: any future lowering pass — to SQL, to a document-store query language, to an in-memory closure — consumes the same parsed AST. Parsing is universal; lowering is backend-specific. The architecture commits to that separation up front.

## 9. Service Composition

Three structural choices shape what handlers and registration look like.

### 9.1 Context types per URL shape

Every handler takes a context value that names *the shape of the URL position it was called from*. The four shapes are:

- A collection-level context for `GET /EntitySet` and `POST /EntitySet`.
- An entity-level context for `GET /EntitySet/{key}`, `PATCH`, `DELETE`.
- A contained-collection context for `GET /EntitySet/{parent_key}/NavProp`.
- A contained-entity context for `GET /EntitySet/{parent_key}/NavProp/{key}`.

Each shape carries exactly the context-specific data — key, parent key, navigation property name — alongside the always-present parsed query options and optional request body. Handlers cannot accidentally read a parent key that the route shape didn't actually provide; the type checker enforces it.

### 9.2 Fluent registration with state attached up front

The service is constructed through a builder. State is attached once at the start; handlers are registered for entity sets and their contained navigation properties; the build step validates the registration against the schema and produces the runtime router. The type system enforces ordering: state-attachment is only callable on a fresh builder, so a handler signature is fixed in terms of the state type it will receive.

### 9.3 Handler signatures stay small

A handler is a function from `(Context, State)` to a future. The library does not impose its own error type, response wrapper, or extractor traits. Handlers compose with whatever the chosen HTTP-server ecosystem already provides for response types. This keeps the integration boring on purpose: a Temper handler that returns JSON looks like a handler in that ecosystem returning JSON, not like a Temper-specific shape.

## 10. The Data-Access Translation Layer

This is the architecturally interesting part, because it is where most of the work of porting OData to a non-`IQueryable` world lives.

### 10.1 The problem

Parsed `QueryOptions` describe what the client wants. The client speaks at the level of *the resource*: "give me rooms whose name starts with 'Oak', sorted by name, the first twenty, with only `id` and `name` in the response." A database speaks at the level of *the data store*: tables (or collections, or graphs), columns, indexes, join paths. The job of the translation layer is to turn the former into a sequence of operations on the latter, *honoring the spec* — including the spec's wrinkles like §3.1's distinction between response projection and query constraint.

This is exactly what `IQueryable` + EF does in .NET, and exactly what Temper cannot get for free. The architecture's commitment is to make this layer a *named, swappable component* with a clearly defined input (parsed query options + a handler-supplied target descriptor) and output (whatever shape the chosen data-access library produces).

### 10.2 Multiple realizations of the same layer

Temper does not specify a single translation strategy. A SQL builder targeting one specific driver is one realization. A query builder for a document store is another. A pass-through to an existing service-layer query API (proxying OData onto an internal RPC) is a third. What they share is:

- The same input — parsed query options, with their semantics fully spelled out by the URL layer.
- The same shape of contract — they expose builder verbs (select, where, orderby, page) and a terminal "execute against the data store" step.
- The same response-shaping contract on the way out — they hand the response layer rows that can be shaped into an OData JSON envelope.

Concrete implementations may differ in important ways. A relational implementation will surface column names and bound parameters; a document implementation may surface field paths and a different parameter convention; a remote-API implementation may carry only a serializable query value. The translation layer's *interface* is what the architecture pins down; the implementations choose how to honor it.

### 10.3 A prototype for the general problem: typed vs. dynamic rows

A prototype SQL builder implementation forced an instructive choice: how should rows come back from the data store? Two row representations turn out to be necessary, with different cost profiles and different invariants. They generalize to any data-access backend, so the trade-off is worth describing here as a general design pattern rather than a SQL detail.

**Typed rows.** Rows are deserialized into a Rust struct whose fields name the columns. The handler can read fields directly, in code, with full type safety. The cost is rigidity: the row's deserializer expects *every* column to be present, so the data-access layer cannot skip columns. `$select` cannot drive the storage projection here — the storage projection has to fetch the full structural set, and `$select` is honored at output-shaping time by retaining only the keys the client asked for.

**Dynamic rows.** Rows come back as ordered maps of column name to value (a `serde_json::Map` for JSON-bound backends; an analogous shape for other backends). The handler cannot read typed fields, but the data-access layer is free to shrink its projection to exactly what `$select` asked for. The cost is that any field-by-field logic in the handler is stringly-typed.

A single unified type that lets `$select` shrink the storage projection *and* still produces typed structs would silently break the typed deserializer's contract — every row that arrives missing a field would fail to deserialize. The only way to make a unified type work is to weaken every row type with optional or default-bearing fields, which pushes the awkwardness onto every typed call site even when the handler does not want `$select` to shrink columns.

The architecture exposes both representations as distinct entry points sharing the same builder surface. Handlers pick the representation that matches what they actually need. The four-way trade-off:

| Handler reads typed fields? | Wants storage projection to shrink with `$select`? | Path |
|---|---|---|
| No — forwards rows to output | No  | Typed — full read, simplest |
| No — forwards rows to output | Yes | Dynamic — `$select` drives the storage projection |
| Yes — reads typed fields in code | No  | Typed — full read; output shaping happens after |
| Yes — reads typed fields in code | Yes | **Inconsistent**: the handler is asking for a known struct shape *and* a possibly-sparse row. The architecture does not paper over this; it expects the implementor to choose one or the other, or to use a backend escape hatch (e.g. optional fields with explicit defaults). |

The pros and cons of each:

- **Typed pros** — type safety in the handler, no stringly-typed access, IDE help, refactors caught by the compiler. **Typed cons** — storage always reads the full structural set; the handler is coupled to the schema's columns in code, not just at runtime.
- **Dynamic pros** — minimum-cost storage queries when `$select` is narrow; no Rust schema to keep in sync with the table; new columns become visible without code changes. **Dynamic cons** — stringly-typed access in the handler; refactors require runtime testing; type errors surface as missing-key errors rather than compile errors.

The architectural commitment is to expose both paths, document the trade-off, and refuse to hide the fourth-quadrant inconsistency behind a runtime quirk.

### 10.4 Ergonomic helpers as one shape of this layer

A translation-layer implementation can offer ergonomic helpers that wrap the common shapes of mapping a context to a query. For example, the typical mapping from a contained-collection context is "apply `$select`, add a `where_eq` for the parent key, apply `$orderby`, apply paging." A helper that takes the context and the table+foreign-key names and produces a fully-formed query in one call collapses that boilerplate.

Helpers like this are an *example* of how a translation-layer implementation can offer affordances, not a part of the architecture itself. A different translation-layer implementation might offer a completely different set of helpers tuned to its backend. What matters at the architecture level is that the contract — parsed query options in, executable query out — is uniform.

## 11. Open Architectural Challenges

The remainder of this paper is a section per architectural challenge that the current proposal acknowledges but does not yet fully resolve. Each is presented with the problem it poses, the constraints from the spec, and the design direction (or "TBD" where a direction has not yet been chosen).

### 11.1 `$filter` lowering

**The challenge.** `$filter` is parsed into a typed expression tree today. Producing useful behavior from that tree requires *lowering* it onto whatever the data-access backend understands: SQL `WHERE` clauses, document-store query predicates, in-memory closures over Rust values. The lowering is not a single decision but a family of decisions: literal type coercions, null semantics, string-function support, the `cast` and `isof` operators, navigation-property traversal inside the filter.

**Constraints from the spec.** `$filter` operates against the *entity type*, not the projection (§3.1), which means lowering happens on the full row's set of accessible columns. Many of the spec's functions (`startswith`, `contains`, `indexof`, date arithmetic, geo predicates) have no portable database equivalent, so the lowering pass needs an extension mechanism for backend-specific function support.

**Direction.** The architecture commits to a single parsed `$filter` AST in the URL layer and a backend-specific lowering pass living in the translation layer. The lowering pass is a visitor over the AST that emits backend-native pieces. Functions the backend cannot lower produce a clear error rather than silently falling back to in-memory filtering, because silent fallback hides performance cliffs.

### 11.2 `$expand` materialization

**The challenge.** `$expand` asks the server to return navigation-property targets inline with the parent entity. This is straightforward for non-cyclic single-valued navigation but quickly becomes complex: multiple expand levels, expanded collections with their own `$select` / `$filter` / `$orderby` / `$top`, transitive expand (`$expand=Manager($expand=Manager)`), and the choice between joined queries vs. multiple round-trips per expanded level.

**Constraints from the spec.** Each `$expand` segment carries its own nested system query options. Expanded entities appear in the response JSON either nested under the parent or, in some bindings, as separate `@odata.context`-referenced documents. The spec does not mandate a query strategy; an implementation may join, may issue multiple queries, may stream.

**Direction.** TBD. Two strategies are on the table: a join-based pass for single-valued non-cyclic expansions, with a per-level secondary-query pass for collection-valued ones; and a strategy-pattern approach where the translation-layer implementation declares which expand shapes it can fold into a single query and which it cannot. The architecture commits to making the response-shaping layer independent of the strategy — i.e. the response builder receives an already-materialized tree of rows, however the translation layer chose to materialize it.

### 11.3 `$count` inline and `/$count` segment

**The challenge.** `$count=true` asks the server to include a total-count annotation alongside a paginated response, computed over the full filtered set. The `/$count` path segment asks for *only* the count as a bare integer. Computing the count efficiently requires a separate query (typically `SELECT COUNT(*) FROM ... WHERE ...`) with the same filter but without the limit/offset.

**Constraints from the spec.** The count must reflect the filter, not the page. Skipping the count is an explicit client request; producing it is non-trivial extra work.

**Direction.** Treat the count as a second, named operation on the translation layer: `count_matching(query_options)` alongside `fetch_all(query_options)`. The service composes them when `$count=true` is set, and exposes the bare count when the route ends in `/$count`. Whether the two queries are issued sequentially or concurrently is a backend-specific optimization the translation layer may choose.

### 11.4 Composite keys

**The challenge.** OData entities may be keyed by more than one property. The URL syntax supports this (`/OrderLines/OrderId=42,LineNumber=3`); the type-safe handler API must support it without forcing single-keyed entities to pay any complexity tax.

**Constraints from the spec.** Composite keys are an entity-type-level decision in the schema. The URL parser must accept both syntactic forms (segment-with-parens and the segment form). Handler signatures must vary by arity.

**Direction.** TBD. Two options: (a) replace `key: String` on entity-shape contexts with `key: KeyValues`, a small type carrying named values, and provide ergonomic accessors for the common single-key case; (b) make the key type generic on the context, so single-keyed entities continue to see `key: String` and composite-keyed entities see a tuple or struct. Option (a) is simpler to type; option (b) is more zero-cost. The decision is pending discussion of which mistake is more recoverable.

### 11.5 Functions and actions

**The challenge.** OData *functions* are side-effect-free callable operations bound to entity types or namespaces; *actions* are side-effecting. Both can take parameters and return entity instances, collections, or primitives. They are first-class citizens in CSDL and in the URL grammar (`/Rooms/Temper.Reservations.AvailableSlots()`), and a production OData service is expected to expose them as part of its surface.

**Constraints from the spec.** Functions and actions appear in CSDL with typed parameter and return shapes. Handlers for them have to type-check against those signatures at startup, the same way entity-set handlers do today.

**Direction.** Extend the schema-first registration model: a builder verb for registering a function or action handler, with the same compile-time and startup-time checks as entity-set registration. The handler receives a function-call context whose shape is generated from the operation's CSDL signature. The translation layer is uninvolved (functions and actions are not query options); the response layer shapes the return value the same way as any entity result.

### 11.6 `$batch`

**The challenge.** `/$batch` accepts a multipart request body containing multiple sub-requests and dispatches them as a single transactional or non-transactional unit. Sub-requests may reference earlier sub-requests' results by ID. The format has two encodings: multipart/mixed (older) and JSON batch (newer).

**Constraints from the spec.** Atomicity semantics within a "changeset" are required; ordering and result correlation are specified. The implementation must be able to dispatch sub-requests through its own router without re-entering the network stack.

**Direction.** TBD. The architectural commitment is that the batch dispatcher must be able to *invoke handlers internally* without an outbound HTTP round-trip; this implies the router's dispatch surface needs to be accessible programmatically, not only as an HTTP service. The shape of that internal-dispatch API is open.

### 11.7 Streams, deltas, and open types

**The challenge.** OData supports three further classes of entity behavior that production services may rely on:

- **Streams** — entities or properties that carry large binary payloads accessed via separate URLs.
- **Deltas** — change-tracking responses where the client periodically asks "what changed since the last token?"
- **Open types** — entities that may carry properties not declared in the schema.

Each has its own architectural implications: streams require an out-of-band binary transfer path that bypasses JSON shaping; deltas require server-side change tracking with persistence; open types require the response shaper to handle untyped properties without losing the typed ones.

**Direction.** TBD. None of these are part of the v0 architecture. They are listed here so the omission is explicit and so the architectural surfaces they touch — the response shaper, the entity context, the schema model — can be designed with the eventual support in mind. The principle is that the v0 design should not foreclose any of them; for example, the response shaper should not assume that all properties of a response are known to the schema, because open types will violate that assumption.

## 12. What Makes the Design Rust-Shaped

A short list to close on, contrasting the proposal with what a literal port of the .NET implementation would produce:

1. **No reflection, no ambient runtime.** Handler signatures are checked at compile time. State is a typed value, not a container lookup. Registration is fluent and explicit, not attribute-driven. The user reads the source and sees what dispatches to what.
2. **Schema is data, dispatch is code.** The CSDL parse produces values; the service builder consumes them. Neither layer leaks the other's shape into user-facing types.
3. **Two row representations, not one.** Typed and dynamic data access are honest about what each guarantees; the architecture refuses to paper over the contract conflict that a unified type would create.
4. **`$select` shapes responses, not queries.** The architecture follows the spec rather than the casual reading.
5. **Crates are small and independently useful.** CSDL parsing and URL parsing are usable outside any HTTP context, by tools that have nothing to do with serving requests.
6. **Failures are loud and specific.** Schema errors, missing handlers, signature mismatches, and unsupported `$filter` constructs all surface at well-defined points — most of them at service-construction time — with messages that name the thing that went wrong.

The .NET implementation set a high bar for what an OData service can do. Temper's proposal aims at the same bar through different means: fewer moving parts, stricter compile-time checks, programmatic configuration in place of declarative annotation, and an honest separation between what's universal (the protocol) and what's integration-specific (the data access, the JSON shape, the HTTP dispatch).
