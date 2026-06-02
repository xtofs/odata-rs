//! End-to-end tests for the semantic resolver (`EdmModel::from_parsed`).
//!
//! These exercise the full pipeline: CSDL XML → reader tokens → syntactic
//! builder → semantic resolver. Failures here usually mean a regression in
//! the resolver itself, but they double as smoke tests for the whole stack.

use odata_edm::EdmModel;
use odata_edm::builder::build_model;
use odata_edm::model::{
    NamedElementRef, NamedTypeId, PrimitiveKind, StructuralTypeId, TypeRef,
    path::TargetPath,
};
use odata_edm::reader::CsdlReader;

fn resolve(xml: &str) -> EdmModel {
    let mut r = CsdlReader::from_reader(xml.as_bytes());
    let parsed = build_model(&mut r).expect("build_model");
    EdmModel::from_parsed(parsed).expect("from_parsed")
}

// ============================================================================
// Basic structure
// ============================================================================

#[test]
fn schema_registered_with_namespace_and_alias() {
    let xml = r#"<Schema Namespace="Sales" Alias="S"/>"#;
    let m = resolve(xml);
    // Edm is always first (builtin); user schema comes second.
    let schemas = m.schemas();
    assert_eq!(schemas.len(), 2);
    assert_eq!(schemas[0].namespace, "Edm");
    assert!(schemas[0].is_builtin);
    assert_eq!(schemas[1].namespace, "Sales");
    assert_eq!(schemas[1].alias.as_deref(), Some("S"));
    assert!(!schemas[1].is_builtin);
}

// ============================================================================
// EntityType + Property with primitive type
// ============================================================================

#[test]
fn entity_type_with_primitive_property_resolves() {
    let xml = r#"<Schema Namespace="N">
        <EntityType Name="T">
            <Key><PropertyRef Name="Id"/></Key>
            <Property Name="Id" Type="Edm.Int32" Nullable="false"/>
            <Property Name="Name" Type="Edm.String"/>
        </EntityType>
    </Schema>"#;
    let m = resolve(xml);
    let qname = "N.T".parse().unwrap();
    let id = match m.lookup_type(&qname).unwrap() {
        NamedTypeId::Entity(id) => id,
        other => panic!("expected EntityType, got {other:?}"),
    };
    let et = m.entity_type(id);
    assert_eq!(et.qualified_name.namespace, "N");
    assert_eq!(et.qualified_name.name, "T");
    assert_eq!(et.properties.len(), 2);
    assert_eq!(et.properties[0].name, "Id");
    assert_eq!(et.properties[1].name, "Name");
    // The property types should be primitive IDs into the Edm schema.
    let id_type = match &et.properties[0].type_ {
        TypeRef::Named(NamedTypeId::Primitive(p)) => *p,
        other => panic!("expected primitive, got {other:?}"),
    };
    let name_type = match &et.properties[1].type_ {
        TypeRef::Named(NamedTypeId::Primitive(p)) => *p,
        other => panic!("expected primitive, got {other:?}"),
    };
    assert_eq!(m.primitive(id_type).qualified_name.name, "Int32");
    assert_eq!(m.primitive(name_type).qualified_name.name, "String");
    assert_eq!(m.primitive(name_type).kind, PrimitiveKind::Primitive);
}

// ============================================================================
// Cross-reference: Property pointing at another user-defined type
// ============================================================================

#[test]
fn property_type_can_reference_another_user_type() {
    let xml = r#"<Schema Namespace="N">
        <ComplexType Name="Address">
            <Property Name="City" Type="Edm.String"/>
        </ComplexType>
        <EntityType Name="Customer">
            <Property Name="Home" Type="N.Address"/>
        </EntityType>
    </Schema>"#;
    let m = resolve(xml);
    let cust_qn = "N.Customer".parse().unwrap();
    let cust_id = match m.lookup_type(&cust_qn).unwrap() {
        NamedTypeId::Entity(id) => id,
        _ => unreachable!(),
    };
    let cust = m.entity_type(cust_id);
    let home_type = match &cust.properties[0].type_ {
        TypeRef::Named(NamedTypeId::Complex(c)) => *c,
        other => panic!("expected complex type, got {other:?}"),
    };
    assert_eq!(m.complex_type(home_type).qualified_name.name, "Address");
}

// ============================================================================
// Collection types
// ============================================================================

#[test]
fn collection_property_resolves_through_collection_wrapper() {
    let xml = r#"<Schema Namespace="N">
        <EntityType Name="Order">
            <Property Name="Tags" Type="Collection(Edm.String)"/>
        </EntityType>
    </Schema>"#;
    let m = resolve(xml);
    let qn = "N.Order".parse().unwrap();
    let id = match m.lookup_type(&qn).unwrap() {
        NamedTypeId::Entity(id) => id,
        _ => unreachable!(),
    };
    let inner = match &m.entity_type(id).properties[0].type_ {
        TypeRef::Collection(inner) => &**inner,
        other => panic!("expected Collection, got {other:?}"),
    };
    let prim = match inner {
        TypeRef::Named(NamedTypeId::Primitive(p)) => *p,
        other => panic!("expected primitive inside collection, got {other:?}"),
    };
    assert_eq!(m.primitive(prim).qualified_name.name, "String");
}

// ============================================================================
// Base type resolution (forward references)
// ============================================================================

#[test]
fn entity_type_base_type_resolves_even_when_declared_later() {
    let xml = r#"<Schema Namespace="N">
        <EntityType Name="Child" BaseType="N.Parent"/>
        <EntityType Name="Parent"/>
    </Schema>"#;
    let m = resolve(xml);
    let child_id = match m.lookup_type(&"N.Child".parse().unwrap()).unwrap() {
        NamedTypeId::Entity(id) => id,
        _ => unreachable!(),
    };
    let parent_id = match m.lookup_type(&"N.Parent".parse().unwrap()).unwrap() {
        NamedTypeId::Entity(id) => id,
        _ => unreachable!(),
    };
    assert_eq!(m.entity_type(child_id).base_type, Some(parent_id));
}

// ============================================================================
// Alias canonicalization on type references
// ============================================================================

#[test]
fn alias_qualified_type_references_resolve_against_full_namespace() {
    let xml = r#"<Schema Namespace="Org.Sales" Alias="S">
        <ComplexType Name="Address">
            <Property Name="City" Type="Edm.String"/>
        </ComplexType>
        <EntityType Name="Customer">
            <Property Name="Home" Type="S.Address"/>
        </EntityType>
    </Schema>"#;
    let m = resolve(xml);
    let cust_id = match m
        .lookup_type(&"Org.Sales.Customer".parse().unwrap())
        .unwrap()
    {
        NamedTypeId::Entity(id) => id,
        _ => unreachable!(),
    };
    let home_type = match &m.entity_type(cust_id).properties[0].type_ {
        TypeRef::Named(NamedTypeId::Complex(c)) => *c,
        _ => unreachable!(),
    };
    assert_eq!(
        m.complex_type(home_type).qualified_name.namespace,
        "Org.Sales"
    );
}

// ============================================================================
// EnumType + TypeDefinition
// ============================================================================

#[test]
fn enum_type_with_underlying_primitive_resolves() {
    let xml = r#"<Schema Namespace="N">
        <EnumType Name="Color" UnderlyingType="Edm.Int32" IsFlags="true">
            <Member Name="Red" Value="1"/>
            <Member Name="Green" Value="2"/>
        </EnumType>
    </Schema>"#;
    let m = resolve(xml);
    let id = match m.lookup_type(&"N.Color".parse().unwrap()).unwrap() {
        NamedTypeId::Enum(id) => id,
        _ => unreachable!(),
    };
    let et = m.enum_type(id);
    assert!(et.is_flags);
    let prim = et.underlying_type.unwrap();
    assert_eq!(m.primitive(prim).qualified_name.name, "Int32");
    assert_eq!(et.members.len(), 2);
    assert_eq!(et.members[0].name, "Red");
}

#[test]
fn type_definition_resolves_underlying_type() {
    let xml = r#"<Schema Namespace="N">
        <TypeDefinition Name="ShortString" UnderlyingType="Edm.String" MaxLength="20"/>
    </Schema>"#;
    let m = resolve(xml);
    let id = match m.lookup_type(&"N.ShortString".parse().unwrap()).unwrap() {
        NamedTypeId::TypeDef(id) => id,
        _ => unreachable!(),
    };
    let td = m.type_def(id);
    let prim = match td.underlying_type {
        NamedTypeId::Primitive(p) => p,
        _ => unreachable!(),
    };
    assert_eq!(m.primitive(prim).qualified_name.name, "String");
}

// ============================================================================
// EntityContainer + EntitySet + NavigationPropertyBinding
// ============================================================================

#[test]
fn entity_container_with_sets_resolves_and_bindings_link_correctly() {
    let xml = r#"<Schema Namespace="N">
        <EntityType Name="Customer">
            <NavigationProperty Name="Orders" Type="Collection(N.Order)"/>
        </EntityType>
        <EntityType Name="Order"/>
        <EntityContainer Name="C">
            <EntitySet Name="Customers" EntityType="N.Customer">
                <NavigationPropertyBinding Path="Orders" Target="Orders"/>
            </EntitySet>
            <EntitySet Name="Orders" EntityType="N.Order"/>
        </EntityContainer>
    </Schema>"#;
    let m = resolve(xml);
    let cid = m.lookup_container(&"N.C".parse().unwrap()).unwrap();
    let c = m.entity_container(cid);
    assert_eq!(c.entity_sets.len(), 2);

    let customers = m.entity_set(c.entity_sets[0]);
    assert_eq!(customers.name, "Customers");
    let cust_type_qn = m.entity_type(customers.entity_type).qualified_name.clone();
    assert_eq!(cust_type_qn.to_string(), "N.Customer");

    assert_eq!(customers.navigation_property_bindings.len(), 1);
    let binding = &customers.navigation_property_bindings[0];
    let target_es = match &binding.target {
        NamedElementRef::EntitySet(id) => *id,
        other => panic!("expected EntitySet target, got {other:?}"),
    };
    assert_eq!(m.entity_set(target_es).name, "Orders");
}

// ============================================================================
// Annotations carried through
// ============================================================================

#[test]
fn annotations_are_carried_from_syntactic_to_semantic() {
    use odata_edm::expr::CsdlAnnotationExpression;
    let xml = r#"<Schema Namespace="N">
        <Annotation Term="Core.Description" String="schema-level"/>
        <EntityType Name="T">
            <Annotation Term="Core.Description" String="entity-level"/>
            <Property Name="P" Type="Edm.String">
                <Annotation Term="Core.Description" String="property-level"/>
            </Property>
        </EntityType>
    </Schema>"#;
    let m = resolve(xml);
    let user_schema = &m.schemas()[1];
    assert_eq!(user_schema.annotations.len(), 1);
    assert!(matches!(
        user_schema.annotations[0].expression,
        Some(CsdlAnnotationExpression::String(ref s)) if s == "schema-level"
    ));

    let id = match m.lookup_type(&"N.T".parse().unwrap()).unwrap() {
        NamedTypeId::Entity(id) => id,
        _ => unreachable!(),
    };
    let et = m.entity_type(id);
    assert_eq!(et.annotations.len(), 1);
    assert_eq!(et.properties[0].annotations.len(), 1);
}

// ============================================================================
// Error batching
// ============================================================================

#[test]
fn unknown_type_references_are_collected_not_fatal() {
    let xml = r#"<Schema Namespace="N">
        <EntityType Name="T">
            <Property Name="A" Type="N.Missing"/>
            <Property Name="B" Type="N.AlsoMissing"/>
        </EntityType>
    </Schema>"#;
    let mut r = CsdlReader::from_reader(xml.as_bytes());
    let parsed = build_model(&mut r).expect("build_model");
    let result = EdmModel::from_parsed(parsed);
    let errors = result.expect_err("expected errors");
    assert!(
        errors.len() >= 2,
        "expected both missing types to error, got {} errors: {errors:#?}",
        errors.len()
    );
}

#[test]
fn duplicate_type_name_is_reported() {
    let xml = r#"<Schema Namespace="N">
        <EntityType Name="T"/>
        <EntityType Name="T"/>
    </Schema>"#;
    let mut r = CsdlReader::from_reader(xml.as_bytes());
    let parsed = build_model(&mut r).expect("build_model");
    let result = EdmModel::from_parsed(parsed);
    let errors = result.expect_err("expected duplicate-name error");
    assert!(
        errors
            .iter()
            .any(|e| matches!(e.kind, odata_edm::model::ResolutionErrorKind::DuplicateName(_))),
        "got errors: {errors:#?}"
    );
}

// ============================================================================
// resolve_path
// ============================================================================

#[test]
fn resolve_path_for_a_named_entity_type() {
    let xml = r#"<Schema Namespace="N"><EntityType Name="T"/></Schema>"#;
    let m = resolve(xml);
    let path = TargetPath::parse("N.T").unwrap();
    let r = m.resolve_path(&path).unwrap();
    let id = match r {
        NamedElementRef::EntityType(id) => id,
        other => panic!("expected EntityType, got {other:?}"),
    };
    assert_eq!(m.entity_type(id).qualified_name.to_string(), "N.T");
}

#[test]
fn resolve_path_for_property_segment_into_entity_type() {
    let xml = r#"<Schema Namespace="N">
        <EntityType Name="T">
            <Property Name="P" Type="Edm.String"/>
        </EntityType>
    </Schema>"#;
    let m = resolve(xml);
    let path = TargetPath::parse("N.T/P").unwrap();
    let r = m.resolve_path(&path).unwrap();
    match r {
        NamedElementRef::Property {
            owner: StructuralTypeId::Entity(_),
            index: 0,
        } => {}
        other => panic!("expected Property{{owner: Entity, index: 0}}, got {other:?}"),
    }
}

#[test]
fn resolve_path_with_alias_canonicalizes() {
    let xml = r#"<Schema Namespace="Org.Sales" Alias="S">
        <EntityType Name="Customer"/>
    </Schema>"#;
    let m = resolve(xml);
    let by_alias = TargetPath::parse("S.Customer").unwrap();
    let by_full = TargetPath::parse("Org.Sales.Customer").unwrap();
    assert_eq!(
        m.resolve_path(&by_alias).unwrap(),
        m.resolve_path(&by_full).unwrap()
    );
}

#[test]
fn resolve_path_returns_unknown_type_for_missing_name() {
    let xml = r#"<Schema Namespace="N"><EntityType Name="T"/></Schema>"#;
    let m = resolve(xml);
    let path = TargetPath::parse("N.Nope").unwrap();
    let err = m.resolve_path(&path).unwrap_err();
    assert!(matches!(
        err.kind,
        odata_edm::model::ResolutionErrorKind::UnknownType(_)
    ));
}

// ============================================================================
// Global enumeration
// ============================================================================

#[test]
fn entity_types_iter_yields_all_user_types() {
    let xml = r#"<Schema Namespace="N">
        <EntityType Name="A"/>
        <EntityType Name="B"/>
        <EntityType Name="C"/>
    </Schema>"#;
    let m = resolve(xml);
    let names: Vec<String> = m
        .entity_types()
        .map(|(_, et)| et.qualified_name.to_string())
        .collect();
    assert_eq!(names, vec!["N.A", "N.B", "N.C"]);
}

#[test]
fn iter_filter_gives_per_schema_view() {
    let xml = r#"<Schema Namespace="A">
        <EntityType Name="One"/>
    </Schema>
    <Schema Namespace="B">
        <EntityType Name="Two"/>
        <EntityType Name="Three"/>
    </Schema>"#;
    // Two top-level Schemas requires wrapping for the reader, but our
    // sample's reader treats anything that isn't <Schema> as Unknown so
    // we wrap in a dummy outer element:
    let xml = format!("<Outer>{xml}</Outer>");
    let m = resolve(&xml);
    let in_a: Vec<_> = m
        .entity_types()
        .filter(|(_, et)| et.qualified_name.namespace == "A")
        .map(|(_, et)| et.qualified_name.name.clone())
        .collect();
    let in_b: Vec<_> = m
        .entity_types()
        .filter(|(_, et)| et.qualified_name.namespace == "B")
        .map(|(_, et)| et.qualified_name.name.clone())
        .collect();
    assert_eq!(in_a, vec!["One"]);
    assert_eq!(in_b, vec!["Two", "Three"]);
}

#[test]
fn primitive_types_iter_covers_builtin_edm_schema() {
    let m = resolve(r#"<Schema Namespace="N"/>"#);
    let names: Vec<String> = m
        .primitive_types()
        .map(|(_, p)| p.qualified_name.to_string())
        .collect();
    assert!(names.contains(&"Edm.String".to_string()));
    assert!(names.contains(&"Edm.Int32".to_string()));
    assert!(names.contains(&"Edm.Geography".to_string()));
    assert!(names.contains(&"Edm.PrimitiveType".to_string()));
}

#[test]
fn iter_returns_ids_that_round_trip_through_accessor() {
    let xml = r#"<Schema Namespace="N">
        <EntityType Name="A"/>
        <EntityType Name="B"/>
    </Schema>"#;
    let m = resolve(xml);
    for (id, et) in m.entity_types() {
        assert_eq!(m.entity_type(id).qualified_name, et.qualified_name);
    }
}

#[test]
fn iter_returns_empty_for_unused_categories() {
    let xml = r#"<Schema Namespace="N"><EntityType Name="T"/></Schema>"#;
    let m = resolve(xml);
    assert_eq!(m.actions().count(), 0);
    assert_eq!(m.functions().count(), 0);
    assert_eq!(m.terms().count(), 0);
}

#[test]
fn resolve_path_for_container_child() {
    let xml = r#"<Schema Namespace="N">
        <EntityType Name="T"/>
        <EntityContainer Name="C">
            <EntitySet Name="Things" EntityType="N.T"/>
        </EntityContainer>
    </Schema>"#;
    let m = resolve(xml);
    let path = TargetPath::parse("N.C/Things").unwrap();
    let r = m.resolve_path(&path).unwrap();
    let es_id = match r {
        NamedElementRef::EntitySet(id) => id,
        other => panic!("expected EntitySet, got {other:?}"),
    };
    assert_eq!(m.entity_set(es_id).name, "Things");
}
