---
name: unify-csdl-model
description: converge model.rs and csdl.rs into one canonical CSDL model in the parse->resolve pipeline
---

# Goal

Unify the current dual CSDL model layers into one canonical CSDL model so the
pipeline is:

reader (json/xml) -> parser -> csdl model -> resolver -> edm model

# Current situation

- `src/model.rs` is used by parser/serialization and carries broad CSDL shape.
- `src/csdl.rs` is used by resolver and is a reduced structural model.
- `src/conversion.rs` maps `model` -> `csdl`, and currently drops or flattens
  some constructs.

# Desired architecture

- Keep exactly one canonical CSDL model between parser and resolver.
- Parser and writers should read/write that same model directly.
- Resolver should consume that same model directly.
- Any unsupported semantic features should fail explicitly or be documented as
  intentional deferrals, not silently lost in ad hoc conversion.

# Constraints

- Preserve existing JSON/XML round-trip behavior where possible.
- Keep resolver/validator responsibilities separated.
- Keep public API coherent (clear exported model type names).
- Avoid broad unrelated refactors.

# Deliverables

1. Design decision note:
   - whether to keep `src/csdl.rs` and absorb `src/model.rs`, or vice versa,
   - and naming/export implications.
2. Refactor implementation:
   - remove duplicated model structures,
   - update parser/serialization/resolver to one model,
   - delete or greatly reduce `src/conversion.rs`.
3. Migration safety:
   - tests for parse -> resolve -> validate on representative fixtures,
   - tests for XML<->JSON round-trip parity,
   - clear errors for unsupported semantics.
4. Documentation updates:
   - align docs and TODOs with the new single-model architecture.

# Acceptance criteria

- There is one canonical CSDL model type graph used by parser, serializer, and resolver.
- `parse_resolve_validate` no longer depends on a structural model-conversion shim.
- No silent dropping of model elements during the parse->resolve handoff.
- Existing supported samples continue to pass, and unsupported areas are explicit.
