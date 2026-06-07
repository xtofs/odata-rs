//! End-to-end demo: CSDL XML → token stream → syntactic model → semantic model.
//!
//! Run with:
//!     cargo run --example csdl_to_model
//!
//! Stage 1 prints the token stream produced by `CsdlReader`, showing how the
//! reader normalizes CSDL's inline-attribute vs nested-element forms.
//!
//! Stage 2 hands the same input to `odata_edm::builder::build_model` and pretty-
//! prints the resulting *syntactic* model — the concrete-tree form, faithful
//! to the XML but with all references still as strings.
//!
//! Stage 3 demonstrates the *surface API* — the resolved `odata_edm::EdmModel`.
//! Today it just shows the synthetic `Edm` schema; once
//! `EdmModel::from_parsed(syntactic)` is implemented it becomes the canonical
//! entry point most consumers will use.

use odata_edm::Result;
use odata_edm::builder::build_model;
use odata_edm::reader::{CsdlReader, SyntaxUnit};

/// Inlined at compile time from `examples/sample.csdl.xml` so that line/column
/// positions printed by Stage 1 line up with the file on disk — open the file
/// in an editor and compare.
// const SAMPLE_CSDL: &str = include_str!("sample.csdl.xml");
const SAMPLE_CSDL: &str = include_str!("example90-schema.csdl.xml");

fn main() -> Result<()> {
    println!("================================================================");
    println!(" Stage 1: XML  →  CsdlReader token stream");
    println!("================================================================");
    let mut reader = CsdlReader::from_reader(SAMPLE_CSDL.as_bytes());

    // Detach the token from `reader`'s borrow so we can call
    // `current_location` afterward to print the cursor position.
    while let Some(tok) = reader.next_token()? {
        let line = format_unit(tok);
        let loc = reader.current_location();
        println!(
            // "  [sample.csdl.xml({:>2}:{:>2})] {line}",
            "  sample.csdl.xml:{}:{} {line}",
            loc.line, loc.column
        );
    }

    println!();
    println!();
    println!("================================================================");
    println!(" Stage 2: token stream  →  syntactic EdmModel");
    println!("================================================================");
    let mut reader = CsdlReader::from_reader(SAMPLE_CSDL.as_bytes());
    let parsed: odata_edm::syntactic::EdmModel = build_model(&mut reader)?;
    println!("{parsed:#?}");

    println!();
    println!("================================================================");
    println!(" Stage 3: surface API — odata_edm::EdmModel (semantic / resolved)");
    println!("================================================================");
    let model = odata_edm::EdmModel::from_parsed(parsed).map_err(|errs| {
        // The resolver returns a batch of diagnostics; surface the first one
        // as our short-circuit error and let the example exit.
        odata_edm::Error::Csdl(format!(
            "{} resolution error(s); first: {}",
            errs.len(),
            errs[0]
        ))
    })?;

    println!("Schemas:");
    for s in model.schemas() {
        println!(
            "  - {} (alias: {:?}, builtin: {})",
            s.namespace, s.alias, s.is_builtin
        );
    }

    // Enumerate everything that belongs to the Sales schema. The model has
    // no per-schema index (those would be a future optimization), so we use
    // the global iterators with a namespace filter.
    println!();
    println!("Contents of schema 'Sales':");
    for (_, et) in model
        .entity_types()
        .filter(|(_, t)| t.qualified_name.namespace == "Sales")
    {
        println!("  EntityType  {}", et.qualified_name);
    }
    for (_, ct) in model
        .complex_types()
        .filter(|(_, t)| t.qualified_name.namespace == "Sales")
    {
        println!("  ComplexType {}", ct.qualified_name);
    }
    for (_, en) in model
        .enum_types()
        .filter(|(_, t)| t.qualified_name.namespace == "Sales")
    {
        println!("  EnumType    {}", en.qualified_name);
    }
    for (_, td) in model
        .type_definitions()
        .filter(|(_, t)| t.qualified_name.namespace == "Sales")
    {
        println!("  TypeDef     {}", td.qualified_name);
    }
    for (_, ec) in model
        .entity_containers()
        .filter(|(_, t)| t.qualified_name.namespace == "Sales")
    {
        println!("  Container   {}", ec.qualified_name);
    }

    // ==========================
    // Resolve a named type via the path API and walk it.
    use odata_edm::model::path::TargetPath;
    use odata_edm::model::{NamedElementRef, NamedTypeId, TypeRef};

    let path = TargetPath::parse("Sales.Country").unwrap();
    let country = match model.resolve_path(&path).unwrap() {
        NamedElementRef::EntityType(id) => id,
        _ => unreachable!(),
    };
    let et = model.entity_type(country);
    println!();
    println!("Resolved EntityType {}:", et.qualified_name);
    for p in &et.properties {
        let type_name = match &p.type_ {
            TypeRef::Named(NamedTypeId::Primitive(id)) => {
                model.primitive(*id).qualified_name.to_string()
            }
            TypeRef::Named(NamedTypeId::Entity(id)) => {
                model.entity_type(*id).qualified_name.to_string()
            }
            TypeRef::Named(NamedTypeId::Complex(id)) => {
                model.complex_type(*id).qualified_name.to_string()
            }
            TypeRef::Named(NamedTypeId::Enum(id)) => {
                model.enum_type(*id).qualified_name.to_string()
            }
            TypeRef::Named(NamedTypeId::TypeDef(id)) => {
                model.type_def(*id).qualified_name.to_string()
            }
            TypeRef::Collection(inner) => format!("Collection({inner:?})"),
        };
        println!(
            "  property {} : {}  (nullable: {})",
            p.name, type_name, p.nullable
        );
    }
    for np in &et.navigation_properties {
        println!("  nav      {} : {:?}", np.name, np.type_);
    }

    // ==========================
    // Resolve a slash-path into one of its properties.
    const PROP_PATH_STR: &str = "Sales.Address/Street";
    let prop_path = TargetPath::parse(PROP_PATH_STR).unwrap();
    println!();
    println!(
        "resolve_path(\"{:?}\") = {:?}",
        PROP_PATH_STR,
        model.resolve_path(&prop_path).unwrap()
    );

    Ok(())
}

fn format_unit(unit: SyntaxUnit) -> String {
    match unit {
        SyntaxUnit::StartElement { name, attributes } => {
            let attrs = attributes
                .iter()
                .map(|(k, v)| format!("{k}={v:?}"))
                .collect::<Vec<_>>()
                .join(", ");

            if attrs.is_empty() {
                format!("Start {name}")
            } else {
                format!("Start {name} [{attrs}]")
            }
        }
        SyntaxUnit::EndElement { name } => format!("End   {name}"),
        SyntaxUnit::AnnotationExpression(expr) => format!("Expr  {expr:?}"),
    }
}
