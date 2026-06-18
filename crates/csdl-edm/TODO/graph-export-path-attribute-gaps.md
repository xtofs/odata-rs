# Graph Export — Path-Attribute Resolution Gaps

## Context

The EDM → `graph.json` exporter (`src/graph.rs`, `examples/edm_graph_demo.rs`)
turns the resolved model into a semantic graph for the `edm-graph` visualizer:

- value-attributes → dropped
- reference-attributes → **edges**
- path-attributes → **paths**

The exporter discovers path-attributes structurally: every model field of shape
`Arc<[<Domain>PathSegment]>` (however wrapped in `Option` / `OnceLock` / `Vec`)
is a path. This is the same typed-segment representation described in
[`path-attribute-resolution-architecture.md`](./path-attribute-resolution-architecture.md).

For the visualizer to render *every* path-attribute uniformly, all of them must
reach the EDM store as `Arc<[Segment]>`. The following are not yet in that shape,
so the exporter currently cannot emit them as first-class paths.

## Gaps to close in the resolver

1. **`ReferentialConstraint.Property` / `.ReferencedProperty`**
   - Today: `ReferentialConstraint { property: String, referenced_property: String }`
     (raw strings in `edm.rs`).
   - Want: resolved property paths as `Arc<[<Segment>]>` (a property path on the
     dependent type, and one on the principal/target type), per the architecture
     inventory.

2. **Annotation path expressions**
   - Today: `CsdlAnnotationExpression::{Path, PropertyPath, NavigationPropertyPath,
     AnnotationPath, ModelElementPath}` hold raw `String`s in `expr.rs`.
   - Want: resolved against the annotation target into typed segment sequences.
   - Also `Annotations.Target` / `Annotation` target paths (architecture inventory).

3. **Best-effort segments → fully resolved**
   - `BindingPathSegment` / `EntitySetPathSegment` may currently contain
     `Unresolved(String)` fallbacks. The exporter renders these as best-effort
     (named segment + anchor, implicit nodes only where unambiguous).
   - Want: full resolution (containment ordering, type-casts, singleton/binding
     starts) so the exporter can reconstruct the complete alternating
     element→type thread for these domains too.

## Done-when

- All path-attributes in the inventory are stored as `Arc<[Segment]>`.
- `src/graph.rs` treats every path-attribute through one code path (segment walk
  + implicit-node reconstruction) with no per-domain string handling and no
  best-effort branches.

See [`path-attribute-resolution-architecture.md`](./path-attribute-resolution-architecture.md)
for the resolve/store/consume phase boundaries and migration plan.
