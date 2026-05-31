# Actions, Functions, and their imports

CSDL 4.01 behavior types — explicitly deferred from the structural model round. The builder currently swallows them via `Frame::Unknown`, so source files containing them parse without error but the operations are dropped from the resulting `EdmModel`.

## Elements to add

Schema children:
- `<Action Name="..." IsBound="..." EntitySetPath="..." />`
- `<Function Name="..." IsBound="..." IsComposable="..." EntitySetPath="..." />`

Children of `Action` / `Function`:
- `<Parameter Name="..." Type="..." Nullable="..." [facets] />` — parameters in source order
- `<ReturnType Type="..." Nullable="..." [facets] />` — at most one
- `<Annotation .../>`

EntityContainer children to add:
- `<ActionImport Name="..." Action="..." EntitySet="..." />`
- `<FunctionImport Name="..." Function="..." EntitySet="..." IncludeInServiceDocument="..." />`

## Model types to add (rough sketch)

```rust
pub struct Action {
    pub name: String,
    pub is_bound: bool,
    pub entity_set_path: Option<String>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<ReturnType>,
    pub annotations: Vec<Annotation>,
}

pub struct Function {
    pub name: String,
    pub is_bound: bool,
    pub is_composable: bool,
    pub entity_set_path: Option<String>,
    pub parameters: Vec<Parameter>,
    pub return_type: ReturnType,        // required for Function
    pub annotations: Vec<Annotation>,
}

pub struct Parameter {
    pub name: String,
    pub type_: String,
    pub nullable: bool,                 // default false for parameters
    pub facets: Facets,
    pub annotations: Vec<Annotation>,
}

pub struct ReturnType {
    pub type_: String,
    pub nullable: bool,                 // default true
    pub facets: Facets,
}

pub struct ActionImport {
    pub name: String,
    pub action: String,
    pub entity_set: Option<String>,
    pub annotations: Vec<Annotation>,
}

pub struct FunctionImport {
    pub name: String,
    pub function: String,
    pub entity_set: Option<String>,
    pub include_in_service_document: bool,  // default true
    pub annotations: Vec<Annotation>,
}
```

Both `Action` and `Function` can be overloaded (multiple with the same name, distinguished by parameter signature), so `Schema::actions: Vec<Action>` and `Schema::functions: Vec<Function>` is correct — no key-by-name.

## Builder work

Add `Frame::Action`, `Frame::Function`, `Frame::Parameter`, `Frame::ReturnType`, `Frame::ActionImport`, `Frame::FunctionImport` and matching `Start`/`End` arms. Parent validation:

- `Action`, `Function` → `Schema`
- `Parameter`, `ReturnType` → `Action` or `Function`
- `ActionImport`, `FunctionImport` → `EntityContainer`

Annotation attachment in `attach_annotation` needs entries for each of the above.
