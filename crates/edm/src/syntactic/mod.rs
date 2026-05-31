use crate::expr::Annotation;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct EdmModel {
    pub schemas: Vec<Schema>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub namespace: String,
    pub alias: Option<String>,
    pub entity_types: Vec<EntityType>,
    pub complex_types: Vec<ComplexType>,
    pub enum_types: Vec<EnumType>,
    pub type_definitions: Vec<TypeDefinition>,
    pub entity_containers: Vec<EntityContainer>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityType {
    pub name: String,
    pub base_type: Option<String>,
    pub abstract_: bool,
    pub open_type: bool,
    pub has_stream: bool,
    pub key: Option<Key>,
    pub properties: Vec<Property>,
    pub navigation_properties: Vec<NavigationProperty>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComplexType {
    pub name: String,
    pub base_type: Option<String>,
    pub abstract_: bool,
    pub open_type: bool,
    pub properties: Vec<Property>,
    pub navigation_properties: Vec<NavigationProperty>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    pub property_refs: Vec<PropertyRef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyRef {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub name: String,
    pub type_: String,
    /// Defaults to true per CSDL 4.01.
    pub nullable: bool,
    pub facets: Facets,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavigationProperty {
    pub name: String,
    pub type_: String,
    /// Defaults to true per CSDL 4.01.
    pub nullable: bool,
    pub partner: Option<String>,
    pub contains_target: bool,
    pub referential_constraints: Vec<ReferentialConstraint>,
    pub on_delete: Option<OnDeleteAction>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferentialConstraint {
    pub property: String,
    pub referenced_property: String,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDeleteAction {
    Cascade,
    None,
    SetNull,
    SetDefault,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumType {
    pub name: String,
    pub underlying_type: Option<String>,
    pub is_flags: bool,
    pub members: Vec<EnumMember>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumMember {
    pub name: String,
    pub value: Option<i64>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeDefinition {
    pub name: String,
    pub underlying_type: String,
    pub facets: Facets,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityContainer {
    pub name: String,
    pub extends: Option<String>,
    pub entity_sets: Vec<EntitySet>,
    pub singletons: Vec<Singleton>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntitySet {
    pub name: String,
    pub entity_type: String,
    /// Defaults to true per CSDL 4.01.
    pub include_in_service_document: bool,
    pub navigation_property_bindings: Vec<NavigationPropertyBinding>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Singleton {
    pub name: String,
    pub type_: String,
    pub navigation_property_bindings: Vec<NavigationPropertyBinding>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavigationPropertyBinding {
    pub path: String,
    pub target: String,
}

/// Type facets shared by Property, TypeDefinition, Parameter, ReturnType, Term.
/// Most CSDL facets travel as a package; `nullable` does not — it applies to a
/// wider set of elements and carries different defaults, so it sits on the
/// owning struct directly rather than here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facets {
    pub max_length: Option<MaxLength>,
    pub precision: Option<u32>,
    pub scale: Option<Scale>,
    pub srid: Option<Srid>,
    pub unicode: Option<bool>,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxLength {
    Max,
    Fixed(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Variable,
    Floating,
    Fixed(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Srid {
    Variable,
    Value(u32),
}
