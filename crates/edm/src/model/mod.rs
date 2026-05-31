//! The Edm model — the surface API of the crate.
//!
//! This is the abstract / resolved view of one or more parsed CSDL schemas:
//! symbolic names have been resolved to typed IDs, overloads picked, aliases
//! canonicalized. It's built from the **syntactic** model in
//! [`crate::syntactic`] (a concrete tree faithful to the CSDL XML) via
//! [`EdmModel::from_parsed`].
//!
//! Consumers of the crate normally interact with this module. The syntactic
//! model is transient — it exists between parsing and resolution and is
//! consumed (by value) by the resolver.
//!
//! Built around per-category arenas with newtype IDs. IDs from different
//! categories don't compare (`EntityTypeId` ≠ `ComplexTypeId`), and cycles in
//! the graph (EntityType → Property → EntityType → NavigationProperty → …)
//! are trivially fine because we store IDs, never references.
//!
//! The synthetic `Edm` schema is always populated. Primitives (`Edm.String`,
//! `Edm.Int32`, …) resolve through the same `type_by_qname` table as any
//! user-defined type — they're just entries in a schema that nobody parsed.
//!
//! Status: scaffold. The [`EdmModel::from_parsed`] resolver body and
//! [`EdmModel::resolve_path`] are `todo!()`. The ID newtypes, arena shape,
//! name AST, and [`TargetPath`] parser surface are in place.

use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU32;

use crate::expr::Annotation;
use crate::syntactic::Facets;

pub mod builtins;
pub mod names;
pub mod path;
mod resolver;

pub use names::QualifiedName;
pub use path::{ParsePathError, SignatureArg, TargetPath};

// ============================================================================
// ID newtypes — one per arena. Each is `Copy` and namespaces its own values
// so unrelated IDs can't be mixed up.
// ============================================================================

macro_rules! arena_id {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(NonZeroU32);

        impl $name {
            #[allow(dead_code)]
            pub(crate) fn from_index(idx: usize) -> Self {
                Self(NonZeroU32::new((idx as u32).wrapping_add(1))
                    .expect("arena index overflow"))
            }

            #[allow(dead_code)]
            pub(crate) fn index(self) -> usize {
                (self.0.get() - 1) as usize
            }
        }
    };
}

arena_id!(EntityTypeId);
arena_id!(ComplexTypeId);
arena_id!(EnumTypeId);
arena_id!(TypeDefId);
arena_id!(PrimitiveTypeId);
arena_id!(ActionId);
arena_id!(FunctionId);
arena_id!(TermId);
arena_id!(EntityContainerId);
arena_id!(EntitySetId);
arena_id!(SingletonId);
arena_id!(ActionImportId);
arena_id!(FunctionImportId);

/// Any reference-able type. Used wherever the model needs to refer to "a
/// type" without caring which specific category it is. Primitives live here
/// too, via the synthetic Edm schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamedTypeId {
    Primitive(PrimitiveTypeId),
    Entity(EntityTypeId),
    Complex(ComplexTypeId),
    Enum(EnumTypeId),
    TypeDef(TypeDefId),
}

/// Either an `EntityType` or a `ComplexType`. Used for path-style references
/// like `Sales.Customer/Name` where the parent is one of the structural types
/// but consumers care about the parent uniformly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructuralTypeId {
    Entity(EntityTypeId),
    Complex(ComplexTypeId),
}

/// Either an `Action` or a `Function`. Used for `Parameter` and `ReturnType`
/// back-references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallableId {
    Action(ActionId),
    Function(FunctionId),
}

// ============================================================================
// NamedElementRef — the unified "reference to any nameable thing" enum.
// What the path resolver returns and what external annotation targets point at.
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NamedElementRef {
    EntityType(EntityTypeId),
    ComplexType(ComplexTypeId),
    EnumType(EnumTypeId),
    TypeDef(TypeDefId),
    Primitive(PrimitiveTypeId),
    Action(ActionId),
    Function(FunctionId),
    Term(TermId),
    EntityContainer(EntityContainerId),
    EntitySet(EntitySetId),
    Singleton(SingletonId),
    ActionImport(ActionImportId),
    FunctionImport(FunctionImportId),

    /// A property on a structural type. `index` is the position in the
    /// owner's `properties` vec.
    Property {
        owner: StructuralTypeId,
        index: u32,
    },
    /// A navigation property on a structural type.
    NavigationProperty {
        owner: StructuralTypeId,
        index: u32,
    },
    /// A member of an enum type.
    EnumMember {
        owner: EnumTypeId,
        index: u32,
    },
    /// A parameter of an action or function.
    Parameter {
        owner: CallableId,
        index: u32,
    },
    /// The return type of an action or function.
    ReturnType {
        owner: CallableId,
    },

    /// An annotation usage on a target. The target may itself be any of the
    /// above (including another annotation usage, in deeply-nested cases).
    AnnotationUsage {
        target: Box<NamedElementRef>,
        term: TermId,
        qualifier: Option<String>,
    },
}

// ============================================================================
// Type expressions — what a CSDL `Type="..."` attribute resolves to.
// ============================================================================

/// A type expression as it appears in a CSDL `Type` attribute, resolved.
/// Pre-resolution form (a raw string) lives in [`crate::syntactic`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeRef {
    Named(NamedTypeId),
    Collection(Box<TypeRef>),
}

// ============================================================================
// Schema metadata
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct SchemaInfo {
    pub namespace: String,
    pub alias: Option<String>,
    /// `true` for the synthetic Edm schema, `false` for user-parsed schemas.
    pub is_builtin: bool,
    pub annotations: Vec<Annotation>,
}

// ============================================================================
// Per-category structs. These are the resolved counterparts of the structs
// in `crate::syntactic` — same shape, but with typed IDs instead of strings.
//
// During scaffolding many fields are kept identical to the parsed form so the
// types compile; resolution will swap strings for IDs.
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct EntityType {
    pub qualified_name: QualifiedName,
    pub base_type: Option<EntityTypeId>,
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
    pub qualified_name: QualifiedName,
    pub base_type: Option<ComplexTypeId>,
    pub abstract_: bool,
    pub open_type: bool,
    pub properties: Vec<Property>,
    pub navigation_properties: Vec<NavigationProperty>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumType {
    pub qualified_name: QualifiedName,
    pub underlying_type: Option<PrimitiveTypeId>,
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
pub struct TypeDef {
    pub qualified_name: QualifiedName,
    pub underlying_type: NamedTypeId,
    pub facets: Facets,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimitiveType {
    pub qualified_name: QualifiedName,
    /// Human-readable category: "primitive", "abstract", "geography", …
    pub kind: PrimitiveKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveKind {
    /// Concrete primitive value type (Edm.String, Edm.Int32, Edm.Decimal, …).
    Primitive,
    /// Geographic / geometric primitive (Edm.Geography, Edm.GeometryPoint, …).
    Spatial,
    /// Abstract base type used in Term/Property type slots
    /// (Edm.PrimitiveType, Edm.EntityType, Edm.AnnotationPath, …).
    Abstract,
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
    pub type_: TypeRef,
    pub nullable: bool,
    pub facets: Facets,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavigationProperty {
    pub name: String,
    pub type_: TypeRef,
    pub nullable: bool,
    pub partner: Option<String>,
    pub contains_target: bool,
    pub referential_constraints: Vec<ReferentialConstraint>,
    pub on_delete: Option<crate::syntactic::OnDeleteAction>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferentialConstraint {
    pub property: String,
    pub referenced_property: String,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub qualified_name: QualifiedName,
    pub is_bound: bool,
    pub entity_set_path: Option<String>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<ReturnType>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub qualified_name: QualifiedName,
    pub is_bound: bool,
    pub is_composable: bool,
    pub entity_set_path: Option<String>,
    pub parameters: Vec<Parameter>,
    pub return_type: ReturnType,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub type_: TypeRef,
    pub nullable: bool,
    pub facets: Facets,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnType {
    pub type_: TypeRef,
    pub nullable: bool,
    pub facets: Facets,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    pub qualified_name: QualifiedName,
    pub type_: TypeRef,
    pub base_term: Option<TermId>,
    pub default_value: Option<String>,
    pub applies_to: Vec<String>,
    pub nullable: bool,
    pub facets: Facets,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityContainer {
    pub qualified_name: QualifiedName,
    pub extends: Option<EntityContainerId>,
    pub entity_sets: Vec<EntitySetId>,
    pub singletons: Vec<SingletonId>,
    pub action_imports: Vec<ActionImportId>,
    pub function_imports: Vec<FunctionImportId>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntitySet {
    pub container: EntityContainerId,
    pub name: String,
    pub entity_type: EntityTypeId,
    pub include_in_service_document: bool,
    pub navigation_property_bindings: Vec<NavigationPropertyBinding>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Singleton {
    pub container: EntityContainerId,
    pub name: String,
    pub type_: TypeRef,
    pub navigation_property_bindings: Vec<NavigationPropertyBinding>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavigationPropertyBinding {
    pub path: String,
    /// `EntitySet` or `Singleton`; resolution stores it as a [`NamedElementRef`]
    /// so consumers can branch on the kind.
    pub target: NamedElementRef,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionImport {
    pub container: EntityContainerId,
    pub name: String,
    pub action: ActionId,
    pub entity_set: Option<NamedElementRef>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionImport {
    pub container: EntityContainerId,
    pub name: String,
    pub function: FunctionId,
    pub entity_set: Option<NamedElementRef>,
    pub include_in_service_document: bool,
    pub annotations: Vec<Annotation>,
}

// ============================================================================
// The arena container
// ============================================================================

/// The resolved graph view of one or more parsed CSDL schemas.
///
/// Always contains the synthetic `Edm` schema (primitive and abstract types)
/// as the first schema. User schemas are appended in source order.
#[derive(Debug, Clone)]
pub struct EdmModel {
    schemas: Vec<SchemaInfo>,

    primitive_types: Vec<PrimitiveType>,
    entity_types: Vec<EntityType>,
    complex_types: Vec<ComplexType>,
    enum_types: Vec<EnumType>,
    type_definitions: Vec<TypeDef>,
    actions: Vec<Action>,
    functions: Vec<Function>,
    terms: Vec<Term>,
    entity_containers: Vec<EntityContainer>,
    entity_sets: Vec<EntitySet>,
    singletons: Vec<Singleton>,
    action_imports: Vec<ActionImport>,
    function_imports: Vec<FunctionImport>,

    // Lookup tables. Keys are always canonical (full-namespace) qualified
    // names. Alias-form names are canonicalized at lookup time via `aliases`.
    // Populated by the resolver; unused until then — hence the allow.
    type_by_qname: HashMap<QualifiedName, NamedTypeId>,
    #[allow(dead_code)]
    actions_by_qname: HashMap<QualifiedName, Vec<ActionId>>,
    #[allow(dead_code)]
    functions_by_qname: HashMap<QualifiedName, Vec<FunctionId>>,
    #[allow(dead_code)]
    terms_by_qname: HashMap<QualifiedName, TermId>,
    #[allow(dead_code)]
    containers_by_qname: HashMap<QualifiedName, EntityContainerId>,

    /// Maps every defined alias to its full namespace. Both ends of the map
    /// only ever hold full namespace strings; rewriting `Alias.Name` →
    /// `Full.Namespace.Name` happens at the entry point of resolution.
    aliases: HashMap<String, String>,
}

impl EdmModel {
    /// Build a model containing only the synthetic Edm schema. Useful as a
    /// base for tests and as the starting point of [`Self::from_parsed`].
    pub fn new() -> Self {
        let mut m = Self {
            schemas: Vec::new(),
            primitive_types: Vec::new(),
            entity_types: Vec::new(),
            complex_types: Vec::new(),
            enum_types: Vec::new(),
            type_definitions: Vec::new(),
            actions: Vec::new(),
            functions: Vec::new(),
            terms: Vec::new(),
            entity_containers: Vec::new(),
            entity_sets: Vec::new(),
            singletons: Vec::new(),
            action_imports: Vec::new(),
            function_imports: Vec::new(),
            type_by_qname: HashMap::new(),
            actions_by_qname: HashMap::new(),
            functions_by_qname: HashMap::new(),
            terms_by_qname: HashMap::new(),
            containers_by_qname: HashMap::new(),
            aliases: HashMap::new(),
        };
        builtins::populate(&mut m);
        m
    }

    /// Build an [`EdmModel`] from a parsed [`crate::syntactic::EdmModel`].
    ///
    /// Takes the parsed model **by value** so the resolver can move strings
    /// (and other owned fields) directly into the arenas — no allocations
    /// duplicated between the two layers. The syntactic model is consumed at
    /// the moment its job is done.
    ///
    /// On success returns the populated model. On failure returns the full
    /// batch of resolution errors collected during the pass — never panics on
    /// bad input.
    pub fn from_parsed(parsed: crate::syntactic::EdmModel) -> Result<Self, Vec<ResolutionError>> {
        resolver::run(parsed)
    }

    /// Resolve a parsed [`TargetPath`] against this model. Walks the path's
    /// `base` (with alias canonicalization), then any `/`-separated segments,
    /// then an optional `@Term#Qualifier` suffix.
    ///
    /// Limitations: overload signatures on the base are not yet matched
    /// (Action/Function arenas are populated separately when supported);
    /// segments are followed only into structural types
    /// (`EntityType`/`ComplexType`/`EnumType`).
    pub fn resolve_path(&self, path: &TargetPath) -> Result<NamedElementRef, ResolutionError> {
        resolver::resolve_path(self, path)
    }

    /// Find a named type by its canonical or aliased qualified name.
    pub fn lookup_type(&self, qname: &QualifiedName) -> Option<NamedTypeId> {
        let canonical = self.canonicalize(qname);
        self.type_by_qname.get(&canonical).copied()
    }

    /// Find an entity container by its canonical or aliased qualified name.
    pub fn lookup_container(&self, qname: &QualifiedName) -> Option<EntityContainerId> {
        let canonical = self.canonicalize(qname);
        self.containers_by_qname.get(&canonical).copied()
    }

    /// Canonicalize a possibly-alias-qualified name to its full-namespace form.
    pub fn canonicalize(&self, qname: &QualifiedName) -> QualifiedName {
        match self.aliases.get(&qname.namespace) {
            Some(ns) => QualifiedName {
                namespace: ns.clone(),
                name: qname.name.clone(),
            },
            None => qname.clone(),
        }
    }

    pub fn schemas(&self) -> &[SchemaInfo] {
        &self.schemas
    }

    // ----- Arena accessors -------------------------------------------------

    pub fn primitive(&self, id: PrimitiveTypeId) -> &PrimitiveType {
        &self.primitive_types[id.index()]
    }
    pub fn entity_type(&self, id: EntityTypeId) -> &EntityType {
        &self.entity_types[id.index()]
    }
    pub fn complex_type(&self, id: ComplexTypeId) -> &ComplexType {
        &self.complex_types[id.index()]
    }
    pub fn enum_type(&self, id: EnumTypeId) -> &EnumType {
        &self.enum_types[id.index()]
    }
    pub fn type_def(&self, id: TypeDefId) -> &TypeDef {
        &self.type_definitions[id.index()]
    }
    pub fn action(&self, id: ActionId) -> &Action {
        &self.actions[id.index()]
    }
    pub fn function(&self, id: FunctionId) -> &Function {
        &self.functions[id.index()]
    }
    pub fn term(&self, id: TermId) -> &Term {
        &self.terms[id.index()]
    }
    pub fn entity_container(&self, id: EntityContainerId) -> &EntityContainer {
        &self.entity_containers[id.index()]
    }
    pub fn entity_set(&self, id: EntitySetId) -> &EntitySet {
        &self.entity_sets[id.index()]
    }
    pub fn singleton(&self, id: SingletonId) -> &Singleton {
        &self.singletons[id.index()]
    }
    pub fn action_import(&self, id: ActionImportId) -> &ActionImport {
        &self.action_imports[id.index()]
    }
    pub fn function_import(&self, id: FunctionImportId) -> &FunctionImport {
        &self.function_imports[id.index()]
    }

    // ----- Global iterators -----------------------------------------------
    //
    // Per-category traversal over the whole model. Each iterator yields
    // `(Id, &Struct)` so consumers can both use the value and remember its
    // handle for cross-references.
    //
    // For "elements of one schema" use these with a `.filter()`:
    //     model.entity_types().filter(|(_, et)| et.qualified_name.namespace == "Sales")
    // A per-schema index on `SchemaInfo` is an optional future optimization
    // (constant-time per-schema iteration); deferred to keep storage costs
    // proportional to actual access patterns.

    pub fn primitive_types(
        &self,
    ) -> impl Iterator<Item = (PrimitiveTypeId, &PrimitiveType)> {
        self.primitive_types
            .iter()
            .enumerate()
            .map(|(i, t)| (PrimitiveTypeId::from_index(i), t))
    }
    pub fn entity_types(&self) -> impl Iterator<Item = (EntityTypeId, &EntityType)> {
        self.entity_types
            .iter()
            .enumerate()
            .map(|(i, t)| (EntityTypeId::from_index(i), t))
    }
    pub fn complex_types(&self) -> impl Iterator<Item = (ComplexTypeId, &ComplexType)> {
        self.complex_types
            .iter()
            .enumerate()
            .map(|(i, t)| (ComplexTypeId::from_index(i), t))
    }
    pub fn enum_types(&self) -> impl Iterator<Item = (EnumTypeId, &EnumType)> {
        self.enum_types
            .iter()
            .enumerate()
            .map(|(i, t)| (EnumTypeId::from_index(i), t))
    }
    pub fn type_definitions(&self) -> impl Iterator<Item = (TypeDefId, &TypeDef)> {
        self.type_definitions
            .iter()
            .enumerate()
            .map(|(i, t)| (TypeDefId::from_index(i), t))
    }
    pub fn entity_containers(
        &self,
    ) -> impl Iterator<Item = (EntityContainerId, &EntityContainer)> {
        self.entity_containers
            .iter()
            .enumerate()
            .map(|(i, t)| (EntityContainerId::from_index(i), t))
    }
    pub fn entity_sets(&self) -> impl Iterator<Item = (EntitySetId, &EntitySet)> {
        self.entity_sets
            .iter()
            .enumerate()
            .map(|(i, t)| (EntitySetId::from_index(i), t))
    }
    pub fn singletons(&self) -> impl Iterator<Item = (SingletonId, &Singleton)> {
        self.singletons
            .iter()
            .enumerate()
            .map(|(i, t)| (SingletonId::from_index(i), t))
    }
    pub fn actions(&self) -> impl Iterator<Item = (ActionId, &Action)> {
        self.actions
            .iter()
            .enumerate()
            .map(|(i, t)| (ActionId::from_index(i), t))
    }
    pub fn functions(&self) -> impl Iterator<Item = (FunctionId, &Function)> {
        self.functions
            .iter()
            .enumerate()
            .map(|(i, t)| (FunctionId::from_index(i), t))
    }
    pub fn terms(&self) -> impl Iterator<Item = (TermId, &Term)> {
        self.terms
            .iter()
            .enumerate()
            .map(|(i, t)| (TermId::from_index(i), t))
    }
    pub fn action_imports(&self) -> impl Iterator<Item = (ActionImportId, &ActionImport)> {
        self.action_imports
            .iter()
            .enumerate()
            .map(|(i, t)| (ActionImportId::from_index(i), t))
    }
    pub fn function_imports(
        &self,
    ) -> impl Iterator<Item = (FunctionImportId, &FunctionImport)> {
        self.function_imports
            .iter()
            .enumerate()
            .map(|(i, t)| (FunctionImportId::from_index(i), t))
    }

    // ----- Internal arena-push helpers (used by `builtins` and, later, the
    //       resolver). pub(crate) so they don't leak. -----------------------

    pub(crate) fn push_schema_info(&mut self, info: SchemaInfo) {
        if let Some(alias) = info.alias.clone() {
            self.aliases.insert(alias, info.namespace.clone());
        }
        self.schemas.push(info);
    }

    pub(crate) fn push_primitive(&mut self, p: PrimitiveType) -> PrimitiveTypeId {
        let qname = p.qualified_name.clone();
        self.primitive_types.push(p);
        let id = PrimitiveTypeId::from_index(self.primitive_types.len() - 1);
        self.type_by_qname.insert(qname, NamedTypeId::Primitive(id));
        id
    }
}

impl Default for EdmModel {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Diagnostics
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionError {
    /// The raw textual reference that failed to resolve.
    pub at: String,
    pub kind: ResolutionErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionErrorKind {
    UnknownType(QualifiedName),
    UnknownTerm(QualifiedName),
    UnknownContainer(QualifiedName),
    UnknownContainerChild {
        container: QualifiedName,
        child: String,
    },
    /// More than one overload matched the given signature.
    OverloadAmbiguous {
        name: QualifiedName,
    },
    /// No overload matched the given signature.
    OverloadNotFound {
        name: QualifiedName,
        signature: String,
    },
    /// A path segment couldn't be followed (e.g. accessing a property that
    /// doesn't exist on the resolved parent type).
    BrokenSegment {
        parent: NamedElementRef,
        segment: String,
    },
    /// `TargetPath::parse` failed on the input.
    InvalidPath(ParsePathError),
    /// Two schemas (or a schema and the builtin Edm) declared the same
    /// qualified name.
    DuplicateName(QualifiedName),
    /// Two schemas declared the same alias.
    DuplicateAlias(String),
    /// A CSDL `Type="..."` attribute was syntactically invalid.
    InvalidTypeReference(String),
    /// The base of a path included an overload signature but the resolver
    /// could not pick a single overload (or actions/functions aren't
    /// supported in this round).
    OverloadResolutionUnsupported,
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "could not resolve {:?}: {:?}", self.at, self.kind)
    }
}

impl std::error::Error for ResolutionError {}

impl EdmModel {
    // pub fn entity_types_iter(&self, schema: &SchemaInfo ) -> impl IntoIter<'_,&EntityType> {
    //     &self.entity_types.into_iter().filter(et => e)
    // }
}
