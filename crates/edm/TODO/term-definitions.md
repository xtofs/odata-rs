# Term definitions

CSDL 4.01 `<Term>` is a vocabulary definition — the schema element that *declares* an annotation term so other schemas can *use* it via `<Annotation Term="...">`. Currently the builder treats `<Term>` as an unknown element and drops it, so vocabulary schemas (e.g. `Org.OData.Core.V1`, `Org.OData.Capabilities.V1`) round-trip with empty Term lists.

Distinct from the `Annotation` struct already in `src/expr.rs` (which represents a *use* of a term). Don't confuse the two.

## Element shape

```xml
<Term Name="Description"
      Type="Edm.String"
      BaseTerm="..."
      DefaultValue="..."
      AppliesTo="Property EntityType"
      Nullable="true"
      MaxLength="..." Precision="..." Scale="..." SRID="..." Unicode="...">
    <Annotation .../>
</Term>
```

`AppliesTo` is a whitespace-separated list of CSDL element names the term targets.

## Model type to add

```rust
pub struct Term {
    pub name: String,
    pub type_: String,
    pub base_term: Option<String>,
    pub default_value: Option<String>,
    pub applies_to: Vec<String>,       // split AppliesTo on whitespace
    pub nullable: bool,                // default true
    pub facets: Facets,
    pub annotations: Vec<Annotation>,
}
```

Add `Schema::terms: Vec<Term>`.

## Builder work

`Frame::Term` with `Start`/`End` arms; parent validation `Term → Schema`; annotation attachment entry. The `applies_to` attribute needs `split_whitespace().map(str::to_string).collect()` at parse time.
