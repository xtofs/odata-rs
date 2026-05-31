# Annotations on annotation-bearing expression elements

## The mental model (to anchor the discussion)

CSDL annotations are **not a kind of expression** — they're an
**extension-property mechanism** that attaches to certain *hosts*. Each
annotation is a key/value pair:

- **Key**: the annotation Term's qualified name, optionally narrowed by a Qualifier (`Term [+ Qualifier]`).
- **Value**: an annotation expression (constant, path, or dynamic expression — exactly what `CsdlAnnotationExpression` already models).

The host is *what is being annotated*. Hosts come in two flavors:

1. **CSDL model elements** — Schema, EntityType, Property, EntityContainer, EntitySet, NavigationProperty, PropertyValue, Member, etc.
2. **Value-bearing expression constructs** that the CSDL spec lets carry their own annotations — Record, Collection, the dynamic-expression elements (If, Apply, Cast, IsOf, LabeledElement, UrlRef), and others per CSDL 4.01 §14.

For category 1, our model already handles things correctly: each element struct has an `annotations: Vec<Annotation>` field.

For category 2, the reader and the model both have gaps. That's what this TODO is about.

## The concrete gap

1. **The recursive expression parser in `src/reader/mod.rs` errors on `<Annotation>` children inside expression elements.** A perfectly valid input like

   ```xml
   <Record>
     <Annotation Term="Core.Description" String="this record describes ..."/>
     <PropertyValue Property="A" String="x"/>
   </Record>
   ```

   currently fails with "unknown expression element `<Annotation>`" — the parser only knows how to handle expression-element children.

2. **`PropertyValue::annotations` exists in the model but is never populated.** The builder always writes an empty `Vec`.

3. **`CsdlAnnotationExpression::Record` and `::Collection` have no `annotations` field at all**, so even if the reader parsed them there's nowhere to put them.

## What NOT to do (and what an earlier version of this TODO got wrong)

The earlier draft proposed `CsdlAnnotationExpression::Annotated { inner, annotations }` — a syntactic wrapper variant slapped around any expression to make it annotation-bearing.

That's the wrong layer:

- It would let the type system pretend *any* expression carries annotations, when in fact only specific spec-sanctioned ones do.
- It would force consumers to "unwrap" trivial values to get at them.
- It mixes the "what is this value" question with the "what's annotated about it" question, which the rest of the model keeps separate.

## What to do instead

Give each spec-sanctioned annotation-bearing expression variant **its own `annotations` field**, exactly the way `Property`, `EntityType`, `Schema` etc. already do. The candidate list, per CSDL 4.01 §14:

| Expression element | Current shape | Proposed |
|---|---|---|
| `Record` | `Record { type_, properties }` | `Record { type_, properties, annotations }` |
| `Collection` | `Collection(Vec<...>)` | `Collection { items, annotations }` |
| `PropertyValue` | already has `annotations` | populate it from the reader |
| `If`, `Apply`, `Cast`, `IsOf`, `LabeledElement`, `UrlRef` | no annotations field | add `annotations` to each variant |

(Verify the precise list against CSDL 4.01 §14.5 before implementing — most dynamic-expression elements carry annotations per spec, but the constants like `Binary`/`Bool`/`Int`/`String`/path elements are *rare* hosts in practice; see below.)

This:

- Matches how the rest of the model treats annotations (a field on the host).
- Doesn't add a meta-variant to the expression enum.
- Makes "this construct can carry annotations" visible at the type-system level — variants that don't admit annotations stay annotation-free.

## Bare constants and paths

The spec technically allows annotations on bare constant expressions
(`<String>x<Annotation .../></String>`) and on path expressions. In real
CSDL these are essentially never used.

Two acceptable handling strategies — pick one when implementing:

1. **Parse and discard** with a `// TODO` so the reader doesn't error on
   the edge case but doesn't grow the model surface for a vanishingly rare
   feature.
2. **Promote selected leaf variants** the same way as Record/Collection
   (e.g. `String { value, annotations }`). Cleaner but adds noise to the
   most common expression variants for almost no gain.

(1) is the better starting point.

## Reader-side changes that come with this

When the model gains `annotations` fields on those expression variants,
the recursive parser in `src/reader/mod.rs` needs to:

- Recognize `<Annotation>` child elements in `build_expr_from_start`
  for Record, Collection, PropertyValue, and the dynamic-expression
  arms.
- Parse them recursively (Term/Qualifier from attributes + nested annotation
  expression as the value) and collect into the parent's `annotations` Vec.
- Stop emitting the misleading "unknown expression element" error in
  those cases.

The existing `<Annotation>` handling in the main `next_token` loop is the
right reference for how a single annotation is structured; the recursive
version just calls into the same Term/Qualifier extraction and expression
parsing, with the result attached to the enclosing expression's
`annotations` field instead of being emitted as a top-level token.
