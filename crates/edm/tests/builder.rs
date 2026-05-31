//! Data-driven tests for [`edm::builder::build_model`].
//!
//! Add new cases by appending more `#[test]` functions. Each test stands on
//! its own; failures are reported individually.

use edm::builder::build_model;
use edm::expr::CsdlAnnotationExpression;
use edm::reader::CsdlReader;
use edm::syntactic::*;

fn build(xml: &str) -> EdmModel {
    let mut r = CsdlReader::from_reader(xml.as_bytes());
    build_model(&mut r).expect("build_model failed")
}

// ----- Schema + EntityType + Property + Key --------------------------------

#[test]
fn schema_with_entity_type_and_key() {
    let xml = r#"<Schema Namespace="N"><EntityType Name="T"><Key><PropertyRef Name="Id"/></Key><Property Name="Id" Type="Edm.Int32" Nullable="false"/></EntityType></Schema>"#;
    let m = build(xml);
    assert_eq!(m.schemas.len(), 1);
    let s = &m.schemas[0];
    assert_eq!(s.namespace, "N");
    assert_eq!(s.entity_types.len(), 1);
    let et = &s.entity_types[0];
    assert_eq!(et.name, "T");
    assert_eq!(et.properties.len(), 1);
    assert_eq!(et.properties[0].name, "Id");
    assert_eq!(et.properties[0].type_, "Edm.Int32");
    assert!(!et.properties[0].nullable);
    let k = et.key.as_ref().expect("missing key");
    assert_eq!(k.property_refs.len(), 1);
    assert_eq!(k.property_refs[0].name, "Id");
}

#[test]
fn property_facets_parsed() {
    let xml = r#"<Schema Namespace="N"><EntityType Name="T">
        <Property Name="A" Type="Edm.String" MaxLength="100" Unicode="true"/>
        <Property Name="B" Type="Edm.String" MaxLength="max"/>
        <Property Name="C" Type="Edm.Decimal" Precision="18" Scale="2"/>
        <Property Name="D" Type="Edm.Decimal" Scale="variable"/>
        <Property Name="E" Type="Edm.Decimal" Scale="floating"/>
        <Property Name="F" Type="Edm.Geography" SRID="4326"/>
        <Property Name="G" Type="Edm.Geography" SRID="variable"/>
        <Property Name="H" Type="Edm.Int32" DefaultValue="42"/>
    </EntityType></Schema>"#;
    let m = build(xml);
    let ps = &m.schemas[0].entity_types[0].properties;
    assert_eq!(ps[0].facets.max_length, Some(MaxLength::Fixed(100)));
    assert_eq!(ps[0].facets.unicode, Some(true));
    assert_eq!(ps[1].facets.max_length, Some(MaxLength::Max));
    assert_eq!(ps[2].facets.precision, Some(18));
    assert_eq!(ps[2].facets.scale, Some(Scale::Fixed(2)));
    assert_eq!(ps[3].facets.scale, Some(Scale::Variable));
    assert_eq!(ps[4].facets.scale, Some(Scale::Floating));
    assert_eq!(ps[5].facets.srid, Some(Srid::Value(4326)));
    assert_eq!(ps[6].facets.srid, Some(Srid::Variable));
    assert_eq!(ps[7].facets.default_value, Some("42".to_string()));
}

#[test]
fn property_nullable_defaults_to_true() {
    let xml = r#"<Schema Namespace="N"><EntityType Name="T"><Property Name="P" Type="Edm.String"/></EntityType></Schema>"#;
    let m = build(xml);
    assert!(m.schemas[0].entity_types[0].properties[0].nullable);
}

// ----- ComplexType + base type + abstract/open -----------------------------

#[test]
fn complex_type_with_base_and_flags() {
    let xml = r#"<Schema Namespace="N"><ComplexType Name="Address" BaseType="N.LocationBase" Abstract="true" OpenType="true"><Property Name="City" Type="Edm.String"/></ComplexType></Schema>"#;
    let m = build(xml);
    let ct = &m.schemas[0].complex_types[0];
    assert_eq!(ct.name, "Address");
    assert_eq!(ct.base_type.as_deref(), Some("N.LocationBase"));
    assert!(ct.abstract_);
    assert!(ct.open_type);
    assert_eq!(ct.properties.len(), 1);
}

// ----- NavigationProperty + ReferentialConstraint + OnDelete ---------------

#[test]
fn navigation_property_with_constraint_and_on_delete() {
    let xml = r#"<Schema Namespace="N"><EntityType Name="Order">
        <NavigationProperty Name="Customer" Type="N.Customer" Nullable="false" Partner="Orders" ContainsTarget="false">
            <ReferentialConstraint Property="CustomerId" ReferencedProperty="Id"/>
            <OnDelete Action="Cascade"/>
        </NavigationProperty>
    </EntityType></Schema>"#;
    let m = build(xml);
    let np = &m.schemas[0].entity_types[0].navigation_properties[0];
    assert_eq!(np.name, "Customer");
    assert_eq!(np.type_, "N.Customer");
    assert!(!np.nullable);
    assert_eq!(np.partner.as_deref(), Some("Orders"));
    assert!(!np.contains_target);
    assert_eq!(np.referential_constraints.len(), 1);
    assert_eq!(np.referential_constraints[0].property, "CustomerId");
    assert_eq!(np.referential_constraints[0].referenced_property, "Id");
    assert_eq!(np.on_delete, Some(OnDeleteAction::Cascade));
}

// ----- EnumType ------------------------------------------------------------

#[test]
fn enum_type_with_members() {
    let xml = r#"<Schema Namespace="N"><EnumType Name="Color" UnderlyingType="Edm.Int32" IsFlags="true">
        <Member Name="Red" Value="1"/>
        <Member Name="Green" Value="2"/>
        <Member Name="Blue" Value="4"/>
    </EnumType></Schema>"#;
    let m = build(xml);
    let et = &m.schemas[0].enum_types[0];
    assert_eq!(et.name, "Color");
    assert_eq!(et.underlying_type.as_deref(), Some("Edm.Int32"));
    assert!(et.is_flags);
    assert_eq!(et.members.len(), 3);
    assert_eq!(et.members[1].name, "Green");
    assert_eq!(et.members[1].value, Some(2));
}

// ----- TypeDefinition ------------------------------------------------------

#[test]
fn type_definition_with_facets() {
    let xml = r#"<Schema Namespace="N"><TypeDefinition Name="ShortString" UnderlyingType="Edm.String" MaxLength="20" Unicode="true"/></Schema>"#;
    let m = build(xml);
    let td = &m.schemas[0].type_definitions[0];
    assert_eq!(td.name, "ShortString");
    assert_eq!(td.underlying_type, "Edm.String");
    assert_eq!(td.facets.max_length, Some(MaxLength::Fixed(20)));
    assert_eq!(td.facets.unicode, Some(true));
}

// ----- EntityContainer + EntitySet + Singleton + NavigationPropertyBinding -

#[test]
fn entity_container_with_sets_and_singleton() {
    let xml = r#"<Schema Namespace="N">
        <EntityContainer Name="Container">
            <EntitySet Name="Orders" EntityType="N.Order" IncludeInServiceDocument="false">
                <NavigationPropertyBinding Path="Customer" Target="Customers"/>
            </EntitySet>
            <Singleton Name="Me" Type="N.User"/>
        </EntityContainer>
    </Schema>"#;
    let m = build(xml);
    let ec = &m.schemas[0].entity_containers[0];
    assert_eq!(ec.name, "Container");
    assert_eq!(ec.entity_sets.len(), 1);
    assert_eq!(ec.entity_sets[0].name, "Orders");
    assert_eq!(ec.entity_sets[0].entity_type, "N.Order");
    assert!(!ec.entity_sets[0].include_in_service_document);
    assert_eq!(ec.entity_sets[0].navigation_property_bindings.len(), 1);
    assert_eq!(
        ec.entity_sets[0].navigation_property_bindings[0].path,
        "Customer"
    );
    assert_eq!(
        ec.entity_sets[0].navigation_property_bindings[0].target,
        "Customers"
    );
    assert_eq!(ec.singletons.len(), 1);
    assert_eq!(ec.singletons[0].name, "Me");
    assert_eq!(ec.singletons[0].type_, "N.User");
}

// ----- Annotations attach to their parents ---------------------------------

#[test]
fn annotations_attach_to_their_parents() {
    let xml = r#"<Schema Namespace="N">
        <Annotation Term="Core.Description" String="schema"/>
        <EntityType Name="T">
            <Annotation Term="Core.Description" String="entity"/>
            <Property Name="P" Type="Edm.String">
                <Annotation Term="Core.Description" String="property"/>
            </Property>
        </EntityType>
    </Schema>"#;
    let m = build(xml);
    let s = &m.schemas[0];
    assert_eq!(s.annotations.len(), 1);
    assert!(matches!(
        s.annotations[0].expression,
        Some(CsdlAnnotationExpression::String(ref x)) if x == "schema"
    ));
    let et = &s.entity_types[0];
    assert_eq!(et.annotations.len(), 1);
    assert!(matches!(
        et.annotations[0].expression,
        Some(CsdlAnnotationExpression::String(ref x)) if x == "entity"
    ));
    let p = &et.properties[0];
    assert_eq!(p.annotations.len(), 1);
    assert!(matches!(
        p.annotations[0].expression,
        Some(CsdlAnnotationExpression::String(ref x)) if x == "property"
    ));
}

// ----- Forward-compat: unknown wrapper elements are ignored ----------------

#[test]
fn unknown_wrapper_elements_are_tolerated() {
    // Mimics an `edmx:Edmx` / `edmx:DataServices` wrapper. The builder
    // should still produce the inner Schema.
    let xml = r#"<Edmx><DataServices><Schema Namespace="N"/></DataServices></Edmx>"#;
    let m = build(xml);
    assert_eq!(m.schemas.len(), 1);
    assert_eq!(m.schemas[0].namespace, "N");
}

// ----- Error reporting -----------------------------------------------------

#[test]
fn entity_type_outside_schema_errors_with_line() {
    // <EntityType> on line 2; <Schema> never opens, so the builder rejects.
    let xml = "<Bogus>\n  <EntityType Name=\"X\"/>\n</Bogus>";
    let mut r = CsdlReader::from_reader(xml.as_bytes());
    let err = build_model(&mut r).expect_err("expected error");
    let msg = err.to_string();
    assert!(msg.contains("line 2"), "missing line in error: {msg}");
    assert!(msg.contains("EntityType"), "missing element in error: {msg}");
    assert!(
        msg.contains("Bogus"),
        "expected parent name in error: {msg}"
    );
}

#[test]
fn unknown_on_delete_action_errors() {
    let xml = r#"<Schema Namespace="N"><EntityType Name="T"><NavigationProperty Name="P" Type="X"><OnDelete Action="Nope"/></NavigationProperty></EntityType></Schema>"#;
    let mut r = CsdlReader::from_reader(xml.as_bytes());
    let err = build_model(&mut r).expect_err("expected error");
    assert!(err.to_string().contains("Nope"), "got: {err}");
}
