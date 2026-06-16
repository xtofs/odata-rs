# OData in Rust Key Challenges

## OData constructs not found in all HTTP APIs

An implementation

- must treat $select as response shaping only, while still evaluating filters, ordering, and key logic on the full entity shape [§3.1](an-odata-library-in-rust.md#31-select-is-a-response-projection-not-a-query-constraint)
- must emit OData-compliant JSON envelopes and annotations, not plain JSON serialization output [§3.2](an-odata-library-in-rust.md#32-the-response-envelope-is-not-vanilla-json)
- should use structured metadata to validate service wiring before serving requests [§3.3](an-odata-library-in-rust.md#33-the-metadata-document-is-structured-not-free-form) / [§7.3](an-odata-library-in-rust.md#73-schema-first-checks-at-service-start)
- must handle inline $count as a separate count operation over the same filtered set, distinct from page retrieval [§11.3](an-odata-library-in-rust.md#113-count-inline-and-count-segment)
- must support recursive $expand materialization, including per-level nested query options [§11.2](an-odata-library-in-rust.md#112-expand-materialization)

## Rust as the implementation language vs C# (or other object oriented language)

An implementation

- should avoid trait-object erasure in core dispatch paths and preserve static handler contracts [§4.1](an-odata-library-in-rust.md#41-the-framework-is-the-dispatcher)
- should define handlers as ordinary async functions with explicit routing, rather than relying on runtime attribute reflection [§4.1](an-odata-library-in-rust.md#41-the-framework-is-the-dispatcher)
- should use distinct typed context shapes for distinct URL shapes to constrain handler access correctly [§9.1](an-odata-library-in-rust.md#91-context-types-per-url-shape)
- should pass application state as typed handler input, not resolve it through an ambient DI container [§4.3](an-odata-library-in-rust.md#43-no-ambient-dependency-injection-container) / [§5.1](an-odata-library-in-rust.md#51-state-is-a-typed-argument-not-an-ambient-lookup)
- should express query constraints as explicit function arguments instead of framework-interpreted attributes [§5.2](an-odata-library-in-rust.md#52-allowlists-are-first-class-arguments) / [§5.3](an-odata-library-in-rust.md#53-allowlists-exist-for-the-same-reason-attributes-did)
- Because Rust lacks a universal IQueryable/EF-style baseline, must provide an explicit translation layer [§4.2](an-odata-library-in-rust.md#42-no-iqueryable--ef-baseline)
- must deliberately choose between typed-row guarantees and dynamic-row projection flexibility per handler scenario [§10.3](an-odata-library-in-rust.md#103-a-prototype-for-the-general-problem-typed-vs-dynamic-rows)
