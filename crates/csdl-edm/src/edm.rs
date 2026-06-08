//! ---------------------------------------------------------------------------
//! 2. Semantic model
//! ---------------------------------------------------------------------------

use std::sync::{Arc, OnceLock, Weak};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    Binary,
    Byte,
    Date,
    DateTimeOffset,
    Decimal,
    Double,
    Duration,
    Guid,
    Int16,
    Int64,
    SByte,
    Single,
    String,
    TimeOfDay,
    Int32,
    Boolean,
}

#[derive(Debug, Clone)]
pub enum ResolvedType {
    Primitive(PrimitiveType),
    Enum(Arc<EnumType>),
    Complex(Arc<ComplexType>),
    TypeDefinition(Arc<TypeDefinition>),
}

#[derive(Debug, Clone)]
pub enum TermType {
    Primitive(PrimitiveType),
    TypeDefinition(Arc<TypeDefinition>),
    Enum(Arc<EnumType>),
    Complex(Arc<ComplexType>),
    Entity(Arc<EntityType>),
}

#[derive(Debug)]
pub struct DocumentModel {
    pub version: String,
    pub references: Vec<Reference>,
    pub schemas: Vec<Arc<Model>>,
}

#[derive(Debug)]
pub struct Reference {
    pub uri: String,
    pub includes: Vec<Include>,
    pub include_annotations: Vec<IncludeAnnotations>,
}

#[derive(Debug)]
pub struct Include {
    pub namespace: String,
    pub alias: Option<String>,
}

#[derive(Debug)]
pub struct IncludeAnnotations {
    pub term_namespace: String,
    pub target_namespace: Option<String>,
    pub qualifier: Option<String>,
}

#[derive(Debug)]
pub struct Model {
    pub namespace: String,
    pub alias: Option<String>,
    pub elements: Vec<Arc<SchemaElement>>,
    pub entity_container: Option<Arc<EntityContainer>>,
}

#[derive(Debug)]
pub enum SchemaElement {
    EntityType(Arc<EntityType>),
    ComplexType(Arc<ComplexType>),
    EnumType(Arc<EnumType>),
    TypeDefinition(Arc<TypeDefinition>),
    Term(Arc<Term>),
    Function(Arc<Function>),
    Action(Arc<Action>),
}

/// Entity type. `properties` and `navigation_properties` are wrapped in
/// `OnceLock` so the resolver can fill them in after every entity/complex
/// `Arc` exists. After resolution they are effectively immutable.
#[derive(Debug)]
pub struct EntityType {
    pub name: String,
    pub is_abstract: bool,
    pub keys: Vec<String>,
    pub properties: OnceLock<Vec<Arc<Property>>>,
    pub navigation_properties: OnceLock<Vec<Arc<NavigationProperty>>>,
}

impl EntityType {
    pub fn properties(&self) -> &[Arc<Property>] {
        self.properties.get().map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn navigation_properties(&self) -> &[Arc<NavigationProperty>] {
        self.navigation_properties
            .get()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

#[derive(Debug)]
pub struct ComplexType {
    pub name: String,
    pub is_abstract: bool,
    pub properties: OnceLock<Vec<Arc<Property>>>,
    pub navigation_properties: OnceLock<Vec<Arc<NavigationProperty>>>,
}

impl ComplexType {
    pub fn properties(&self) -> &[Arc<Property>] {
        self.properties.get().map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn navigation_properties(&self) -> &[Arc<NavigationProperty>] {
        self.navigation_properties
            .get()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

#[derive(Debug)]
pub struct EnumType {
    pub name: String,
    pub members: Vec<Arc<EnumMember>>,
}

#[derive(Debug)]
pub struct EnumMember {
    pub name: String,
    pub value: Option<i64>,
}

#[derive(Debug)]
pub struct TypeDefinition {
    pub name: String,
    pub underlying_type: PrimitiveType,
}

#[derive(Debug)]
pub struct Term {
    pub name: String,
    pub is_collection: bool,
    pub ty: OnceLock<TermType>,
    pub base_term: OnceLock<Option<Weak<Term>>>,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub is_bound: bool,
    pub is_composable: bool,
    pub entity_set_path: Option<String>,
    pub parameters: Vec<OperationParameter>,
    pub return_type: Option<OperationReturnType>,
}

#[derive(Debug)]
pub struct Action {
    pub name: String,
    pub is_bound: bool,
    pub entity_set_path: Option<String>,
    pub parameters: Vec<OperationParameter>,
    pub return_type: Option<OperationReturnType>,
}

#[derive(Debug)]
pub struct OperationParameter {
    pub name: String,
    pub ty: TermType,
    pub is_collection: bool,
}

#[derive(Debug)]
pub struct OperationReturnType {
    pub ty: TermType,
    pub is_collection: bool,
}

impl Term {
    pub fn ty(&self) -> Option<&TermType> {
        self.ty.get()
    }
}

#[derive(Debug)]
pub struct Property {
    pub name: String,
    pub ty: ResolvedType,
    pub is_collection: bool,
}

#[derive(Debug)]
pub struct NavigationProperty {
    pub name: String,
    /// Weak to break entity <-> entity cycles.
    pub target: Weak<EntityType>,
    pub is_collection: bool,
    pub partner: Option<String>,
    pub contains_target: Option<bool>,
    pub on_delete: Option<OnDeleteAction>,
    pub referential_constraints: Vec<ReferentialConstraint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDeleteAction {
    Cascade,
    None,
    SetNull,
    SetDefault,
}

impl OnDeleteAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cascade => "Cascade",
            Self::None => "None",
            Self::SetNull => "SetNull",
            Self::SetDefault => "SetDefault",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReferentialConstraint {
    pub property: String,
    pub referenced_property: String,
}

#[derive(Debug)]
pub struct EntityContainer {
    pub name: String,
    pub elements: Vec<Arc<EntityContainerElement>>,
}

#[derive(Debug)]
pub enum EntityContainerElement {
    EntitySet(Arc<EntitySet>),
    Singleton(Arc<Singleton>),
    FunctionImport(Arc<FunctionImport>),
    ActionImport(Arc<ActionImport>),
}

#[derive(Debug)]
pub struct EntitySet {
    pub name: String,
    pub target: Arc<EntityType>,
    pub navigation_property_bindings: Vec<NavigationPropertyBinding>,
}

#[derive(Debug)]
pub struct Singleton {
    pub name: String,
    pub target: Arc<EntityType>,
    pub navigation_property_bindings: Vec<NavigationPropertyBinding>,
}

#[derive(Debug)]
pub struct FunctionImport {
    pub name: String,
    pub function: String,
    pub entity_set: Option<String>,
}

#[derive(Debug)]
pub struct ActionImport {
    pub name: String,
    pub action: String,
    pub entity_set: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NavigationPropertyBinding {
    pub path: String,
    pub target: String,
}
