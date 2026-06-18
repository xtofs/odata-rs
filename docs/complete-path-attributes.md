# complete the path-attribute implementation

strongly type the `(not yet typed: String)` path attributes.

## Context

After this session's catalog work, eight path-attribute rows in [docs/csdl-attribute-catalog.toml](docs/csdl-attribute-catalog.toml) still carry `path_segment_domain = "(not yet typed: String)"`:

1. `ReferentialConstraint.Property`
2. `ReferentialConstraint.ReferencedProperty`
3. `Annotations.Target`
4. `Annotation.Target`
5. `Annotation.expression.Path`
6. `Annotation.expression.PropertyPath`
7. `Annotation.expression.NavigationPropertyPath`
8. `Annotation.expression.AnnotationPath`
9. `Annotation.expression.ModelElementPath` (in the catalog only; no Rust variant exists yet)

Two TODO docs in `crates/csdl-edm/TODO/` already frame this work:

- [`path-attribute-resolution-architecture.md`](crates/csdl-edm/TODO/path-attribute-resolution-architecture.md) — settled three-phase architecture (resolve / store / consume) and the `Arc<[Segment]>` storage shape. Treat as authoritative.
- [`graph-export-path-attribute-gaps.md`](crates/csdl-edm/TODO/graph-export-path-attribute-gaps.md) — same items, framed from the graph-exporter perspective.

What's missing is a concrete migration plan that pairs the catalog updates with the implementation work. This plan delivers that. Scope is deliberately the nine items above. Closing the `Unresolved(String)` fallback variants of the existing `BindingPathSegment` / `EntitySetPathSegment` enums (already raised in [`graph-export-path-attribute-gaps.md`](crates/csdl-edm/TODO/graph-export-path-attribute-gaps.md) §3) is a related but separate effort.

## Design choices made up front (push back if any are wrong)

**D1. `KeyPathSegment` becomes `PropertyPathSegment`** — a rename, not a new domain. The current name describes the _use_ (entity key); the shape is "structural-Property descent through ComplexType to a primitive." That shape is also what `ReferentialConstraint.Property/.ReferencedProperty` and `Annotation.expression.PropertyPath` need. One typed domain serves all three. Catalog entry on `PropertyRef.Name` keeps the same constraints; the use as a key continues to be documented in its `notes` and `resolution_context`. Net effect: fewer redundant domains, more honest naming.

**D2. Resolved annotations get a minimal EDM-level representation.** Today the resolver keeps annotations entirely in the syntactic layer (`csdl::Annotation`) and the EDM model has no Annotation node. Typed annotation expression paths need a home. Add `edm::Annotation { term: Arc<Term>, qualifier: Option<String>, expression: AnnotationExpression, target: Option<Arc<[ModelElementPathSegment]>> }` and `edm::AnnotationExpression` (the typed mirror of `csdl::CsdlAnnotationExpression`, with path variants storing `Arc<[PropertyPathSegment]>` / `Arc<[NavigationPathSegment]>` / `Arc<[AnnotationPathSegment]>` / `Arc<[ModelElementPathSegment]>`). Hang annotations off each annotated EDM node (EntityType, ComplexType, Property, NavigationProperty, Schema, etc.). This is structural work that the `$metadata` endpoint (see [`TODO/service-metadata-endpoint.md`](TODO/service-metadata-endpoint.md)) will need anyway.

**D3. `Annotation.expression.Path` (the catch-all) stays a String for now.** Its terminal kind is genuinely context-dependent (could be Property, NavigationProperty, EntityType, or deeper). Resolving it requires inferring what kind of target the surrounding expression expects, which is a typed-expression-evaluation concern, not a path-resolution concern. Catalog stays "(not yet typed: String)" for this one item with a note explaining why; this leaves seven catalog rows typed instead of nine. Future work: type once the annotation-expression resolver gains type inference.

## New segment domains

Add to the catalog's `[[segment_domain]]` table (matching enums added to `edm.rs`):

- **`PropertyPathSegment`** — `Property`. Used by `PropertyRef.Name` (renamed from `KeyPathSegment`), `ReferentialConstraint.Property`, `ReferentialConstraint.ReferencedProperty`, `Annotation.expression.PropertyPath`. Terminates at a primitive Property; descends through ComplexType-valued structural Properties. Single `Property` member like the old `KeyPathSegment`.
- **`NavigationPathSegment`** — `Property | NavigationProperty`. Used by `Annotation.expression.NavigationPropertyPath`. Terminates at a NavigationProperty; descends through ComplexType-valued Properties. Distinct from `BindingPathSegment` (no EntitySet/Singleton/EntityContainer/Cast heads — annotation contexts don't have container-rooted paths).
- **`AnnotationPathSegment`** — `Property | NavigationProperty | Annotation`. Used by `Annotation.expression.AnnotationPath`. Walks through structural+navigation segments, terminates at an Annotation host.
- **`ModelElementPathSegment`** — `Schema | EntityType | ComplexType | EnumType | TypeDefinition | Term | Function | Action | EntityContainer | EntitySet | Singleton | Property | NavigationProperty | EnumMember`. Used by `Annotations.Target`, `Annotation.Target`, `Annotation.expression.ModelElementPath`. The most permissive — any addressable model element.

## Per-attribute catalog updates

For each numbered item from §1, the catalog edit is mechanical: replace `path_segment_domain = "(not yet typed: String)"` with the chosen domain, drop the `notes = "resolver gap — stored as String today"` / `model-element path; resolver gap` prose, and keep the existing structured `terminal_kinds` / `descent_kinds` fields. The structured constraints already added this session match the new domains; no constraint changes are needed.

Item 5 (`Annotation.expression.Path`) keeps its current row per D3, with an updated note explaining the context-dependence.

## Implementation work in `crates/csdl-edm/`

Five tasks, ordered by dependency. Each task can land as its own commit. After all five, the graph exporter and validator stop holding any raw path strings from this list.

### Task A — introduce typed domains in `edm.rs`

In [`crates/csdl-edm/src/edm.rs`](crates/csdl-edm/src/edm.rs):

- Rename `KeyPathSegment` to `PropertyPathSegment`. Update every use site (six in-file references, plus the public re-exports and any external callers in service / validator / graph crates). The catalog rename ([D1](#design-choices-made-up-front-push-back-if-any-are-wrong)) and the doc/legend regenerate cleanly from this.
- Add the three new segment enums (`NavigationPathSegment`, `AnnotationPathSegment`, `ModelElementPathSegment`), each modeled on `BindingPathSegment` — variants hold `Weak<...>` to the resolved element, plus `Unresolved(String)` for migration tolerance, with `display_name()` and a module-level `*_path_to_string` formatter for symmetry with existing code.
- Add `edm::Annotation` and `edm::AnnotationExpression` per D2. Place `annotations: Vec<Arc<Annotation>>` (filled by resolver via `OnceLock` like other back-edges) on `EntityType`, `ComplexType`, `EnumType`, `TypeDefinition`, `Term`, `Property`, `NavigationProperty`, `Schema`, `EntityContainer`, `EntitySet`, `Singleton`, `EnumMember`, `Parameter`, `ReturnType` (the host kinds CSDL allows annotations on). Schema-level `<Annotations>` bulk-target wrappers resolve into the same `Annotation` shape, just with the resolved target path populated.

### Task B — type `ReferentialConstraint` (simplest; smallest blast radius)

Already has consumers, so this is the highest-value first move.

- In [`crates/csdl-edm/src/edm.rs`](crates/csdl-edm/src/edm.rs), change `ReferentialConstraint { property: String, referenced_property: String }` to hold `Arc<[PropertyPathSegment]>` on each field. Keep an inherent `display_name()` for both, for diagnostic and serialization use.
- In [`crates/csdl-edm/src/resolver.rs`](crates/csdl-edm/src/resolver.rs) (around `resolver.rs:1268-1271` per the inventory), resolve the two paths against the dependent and principal entity types respectively. Reuse the existing `KeyPathSegment`-style resolution (now `PropertyPathSegment`) — parse `'/' `-separated, look up each segment in the current type's properties, descend through ComplexType properties, require the terminal to be primitive.
- In [`crates/csdl-edm/src/validator.rs`](crates/csdl-edm/src/validator.rs) (the `ReferentialConstraintConsistencyRule` around `validator.rs:532-573`), switch from string-equality lookup against entity property names to typed `Arc<Property>` equality on the resolved segments. Equality compares by `Arc::ptr_eq` against the principal/dependent type's primitive properties.
- Drop the (now-redundant) string-name-compare codepaths from the validator. Resolver diagnostics replace late-binding validation errors.

### Task C — type the typed annotation expression paths

In [`crates/csdl-edm/src/expr.rs`](crates/csdl-edm/src/expr.rs):

- Leave `CsdlAnnotationExpression` as the syntactic form (still `String`-shaped paths). It's the reader/serializer's wire model.
- The corresponding `edm::AnnotationExpression` (added in Task A) carries typed segments: `PropertyPath(Arc<[PropertyPathSegment]>)`, `NavigationPropertyPath(Arc<[NavigationPathSegment]>)`, `AnnotationPath(Arc<[AnnotationPathSegment]>)`, `ModelElementPath(Arc<[ModelElementPathSegment]>)`, and a transitional `Path(String)` for D3.

In [`crates/csdl-edm/src/resolver.rs`](crates/csdl-edm/src/resolver.rs):

- For each annotation host, when materializing `edm::Annotation`, walk the syntactic expression tree and resolve each path variant against the annotation's _target type_ (or the annotation target's enclosing schema for `ModelElementPath`).
- Catch-all `Path(String)` passes through untyped per D3.

### Task D — type `Annotation.Target` (and the collapsed `Annotations.Target`)

In [`crates/csdl-edm/src/csdl.rs`](crates/csdl-edm/src/csdl.rs) the wire/syntactic field `Annotation { target: Option<String>, … }` stays as-is (reader/serializer form). The resolver populates `edm::Annotation.target: Option<Arc<[ModelElementPathSegment]>>` from it (added in Task A), resolving against the document's model. Per the agent inventory, the singular and plural forms are already collapsed at parse time, so this single resolution path covers both catalog rows.

### Task E — catalog edits + script verification

Edit `docs/csdl-attribute-catalog.toml`:

- Replace the four affected `path_segment_domain` values per the per-attribute mapping above (six of the nine items get a real domain; item 5 stays as documented).
- Replace `KeyPathSegment` with `PropertyPathSegment` everywhere (one `[[segment_domain]]` block, one `[[attribute]]` `path_segment_domain` field on `PropertyRef.Name`).
- Drop the now-redundant `notes` text on the typed rows; expand the `notes` on `Annotation.expression.Path` to explain the D3 deferral.
- Run `python3 scripts/sync_attribute_catalog.py` and `--check`.

## Reuse / existing utilities

- Resolver helpers in [`crates/csdl-edm/src/resolver.rs`](crates/csdl-edm/src/resolver.rs) already implement the property-path walk for `PropertyRef.Name` keys. Refactor into a single `resolve_property_path(root: &EntityType, raw: &str) -> Result<Arc<[PropertyPathSegment]>>` (or similar) so PropertyRef, ReferentialConstraint and Annotation.PropertyPath share one implementation.
- `BindingPathSegment::Unresolved(String)` and `EntitySetPathSegment::Unresolved(String)` patterns are the model for the new domains' migration-tolerant variants — copy the shape (`Unresolved(String)` plus `display_name()` plus `*_to_string` helper).
- `qb.push_bind` style and the `Arc + OnceLock` back-edge pattern in `edm.rs` is what `edm::Annotation` and its host-side annotation list should use.

## Verification

Run after each task; the workspace must stay green across all of them.

1. Build + tests:
   ```sh
   cargo build --workspace
   cargo test -p csdl-edm
   cargo test -p odata-rs-service
   cargo test --workspace
   ```
2. Catalog + doc sync:
   ```sh
   python3 scripts/sync_attribute_catalog.py
   python3 scripts/sync_attribute_catalog.py --check
   ```
3. Spec-faithfulness spot check:
   - Render the path-attribute table and segment-domain legend; confirm no row says `(not yet typed: String)` except `Annotation.expression.Path` (per D3).
   - Confirm the rendered domains include `PropertyPathSegment`, `NavigationPathSegment`, `AnnotationPathSegment`, `ModelElementPathSegment`, and the existing `BindingPathSegment` / `EntitySetPathSegment`.
4. End-to-end example:
   ```sh
   cargo run -p csdl-edm --example parse_resolve_validate -- data/inputs/extras_sample.csdl.xml
   cargo run -p csdl-edm --example parse_resolve_validate -- data/inputs/import_sample.csdl.xml
   cargo run -p csdl-edm --example url_expansion
   ```
   These exercise annotations and referential constraints; they should resolve cleanly with no new errors.
5. New tests to add (per task):
   - **Task B**: a `<ReferentialConstraint>` test that resolves both paths and a validator test that catches a dangling reference on either side.
   - **Task C**: per-variant tests for each typed path variant (PropertyPath terminates at primitive, NavigationPropertyPath terminates at navigation, AnnotationPath terminates at annotation host).
   - **Task D**: an `<Annotations Target="…">` test that resolves the bulk target.
   - For each task: an Unresolved-fallback test that exercises the migration-tolerant `Unresolved(String)` branch.
6. The graph exporter ([`crates/csdl-edm/src/graph.rs`](crates/csdl-edm/src/graph.rs)) should now emit the new path domains as first-class paths. Verify with a quick run of the visualizer demo if available.

## Out of scope

- Closing `BindingPathSegment::Unresolved` and `EntitySetPathSegment::Unresolved` fallback emissions in the resolver. Tracked in [`graph-export-path-attribute-gaps.md`](crates/csdl-edm/TODO/graph-export-path-attribute-gaps.md) §3 and worth a dedicated follow-up.
- Typing `Annotation.expression.Path` per D3.
- `$metadata` work (depends on edm::Annotation but lives in [`TODO/service-metadata-endpoint.md`](TODO/service-metadata-endpoint.md)).
