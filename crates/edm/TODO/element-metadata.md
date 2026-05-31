# Element meta-table: required/allowed attributes and annotation acceptance

The `meta` module inside [src/builder.rs](src/builder.rs) currently captures only **parent rules** (`ParentSpec`). The same table is the natural home for the rest of CSDL's element-level constraints, which today either live implicitly in the construction arms (silent ignore of unknown attrs, default values, …) or aren't enforced at all.

## What to add

Extend `ElementRule` with:

```rust
pub(super) struct ElementRule {
    pub name: &'static str,
    pub parents: ParentSpec,
    pub required_attrs: &'static [&'static str],
    pub optional_attrs: &'static [&'static str],
    pub accepts_annotations: bool,
}
```

- `required_attrs` — fail with a clear diagnostic if any are missing. Example: `Property` requires `Name` and `Type`; `EntityType` requires `Name`; `Annotation` requires `Term`.
- `optional_attrs` — combined with `required_attrs` defines the full attribute domain. Anything not in either list is an *unknown attribute* — at minimum a diagnostic, possibly an error in strict mode.
- `accepts_annotations` — whether `<Annotation>` child elements are valid on this parent. Today the builder accepts annotations on any frame that has an `annotations` field and silently drops them otherwise; declaring the rule explicitly lets the builder validate (and lets a writer know which elements need an annotations slot).

## Driving downstream consumers

The same table is the basis for:

1. **Required-attribute validation**: replaces today's `.unwrap_or_default()` for `Name`/`Type` etc. with a hard error pointing at the offending element.
2. **Unknown-attribute diagnostics**: surfaces typos like `Nulllable="true"` instead of silently ignoring them.
3. **A future CSDL writer**: the writer needs the same attribute-domain knowledge to emit valid XML.
4. **A future validator** that checks an `EdmModel` against the CSDL grammar without re-parsing.

## When to do this

Once the first consumer that needs strict attribute validation lands — likely either:
- The semantic-model resolver (it'll choke on missing `Name`/`Type` anyway, and a builder-level error is friendlier), or
- A CSDL writer (needs to know which attributes are valid on which elements).

Until then, the parent rules are the only enforcement the builder needs.
