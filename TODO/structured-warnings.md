# Structured warnings

The reader and the (future) model parser silently discard information in a couple of cases where the input is well-formed XML but semantically suspect:

1. **PropertyValue dual form** — inline attribute AND nested element expressions on the same `<PropertyValue>` (e.g. `<PropertyValue Property="P" String="a"><Int>7</Int></PropertyValue>`). The reader's recursive expression parser keeps the inline form and drops the nested body. See `pick_first_value` in [src/reader/mod.rs](src/reader/mod.rs).

2. **Annotation dual form / multiple expressions** — same situation on `<Annotation>`. The reader emits ALL expressions as separate `AnnotationExpression` tokens (correct token-stream behavior); the model parser keeps the first and drops the rest. The example's `build_model` in [examples/csdl_to_model.rs](examples/csdl_to_model.rs) `eprintln!`s a warning; production code needs something structured.

What "structured" should look like:

- A `Diagnostic` type with severity, source location (XML line/col from quick-xml), and a stable diagnostic code.
- An emission channel on `CsdlReader` (e.g. `reader.diagnostics() -> &[Diagnostic]`).
- A matching channel on the model builder once it exists.
- Optionally, a callback API for streaming diagnostics during parsing.

Open questions:
- Should the model parser's dropped-expression warning be emitted by the reader instead (since the reader knows the source XML)? That would centralize diagnostics in one place but couples the reader to a semantic decision.
- Default behavior when diagnostics are emitted: collect-and-continue, or fail-fast? Probably collect-and-continue with a `strict` mode toggle.
