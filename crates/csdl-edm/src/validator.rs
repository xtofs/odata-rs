//! Semantic validation that is intentionally separate from reference resolution.

use std::collections::HashSet;
use std::sync::Arc;

use crate::edm::{
    Action, BindingPath, ComplexType, DocumentModel, EntityContainer, EntityContainerElement,
    EntityType, EnumType, Function, Model, ResolvedType, SchemaElement, Term,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyPathClassification {
    Unknown,
    Scalar,
    Complex,
    Collection,
}

#[derive(Debug, Clone)]
pub enum ValidationError {
    DuplicateKey {
        entity: String,
        key: String,
    },
    UnknownKeyProperty {
        entity: String,
        key: String,
    },
    NonScalarKeyProperty {
        entity: String,
        key: String,
    },
    DuplicateChildName {
        parent_kind: &'static str,
        parent: String,
        child_kind: &'static str,
        child: String,
    },
    UnknownContainerTarget {
        container: String,
        source_kind: &'static str,
        source: String,
        target: String,
    },
    InvalidNavigationPropertyBinding {
        container: String,
        source_kind: &'static str,
        source: String,
        attribute: &'static str,
        value: String,
        reason: &'static str,
    },
    BoundOperationMissingBindingParameter {
        operation_kind: &'static str,
        operation: String,
    },
    InvalidEntitySetPath {
        operation_kind: &'static str,
        operation: String,
        path: String,
        reason: &'static str,
    },
    UnknownNavigationPartner {
        entity: String,
        navigation: String,
        partner: String,
        target_entity: String,
    },
    UnknownReferentialConstraintProperty {
        entity: String,
        navigation: String,
        property: String,
    },
    UnknownReferentialConstraintReferencedProperty {
        entity: String,
        navigation: String,
        target_entity: String,
        referenced_property: String,
    },
    CyclicTermBaseTerm {
        term: String,
    },
}

pub fn validate_document(document: &DocumentModel) -> Result<(), Vec<ValidationError>> {
    let engine = ValidatorEngine::default();
    let mut errors = Vec::new();

    for schema in &document.schemas {
        errors.extend(engine.validate_model_collect(schema.as_ref()));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn validate_model(model: &Model) -> Result<(), Vec<ValidationError>> {
    let engine = ValidatorEngine::default();
    let errors = engine.validate_model_collect(model);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Default)]
struct ValidationContext {
    errors: Vec<ValidationError>,
}

impl ValidationContext {
    fn push(&mut self, error: ValidationError) {
        self.errors.push(error);
    }
}

trait ValidationRule: Send + Sync {
    fn name(&self) -> &'static str;

    fn visit_model(&self, _model: &Model, _ctx: &mut ValidationContext) {}
    fn visit_entity_type(&self, _entity: &EntityType, _ctx: &mut ValidationContext) {}
    fn visit_complex_type(&self, _complex: &ComplexType, _ctx: &mut ValidationContext) {}
    fn visit_enum_type(&self, _enum_type: &EnumType, _ctx: &mut ValidationContext) {}
    fn visit_function(&self, _function: &Function, _ctx: &mut ValidationContext) {}
    fn visit_action(&self, _action: &Action, _ctx: &mut ValidationContext) {}
    fn visit_term(&self, _term: &Term, _ctx: &mut ValidationContext) {}
    fn visit_entity_container(&self, _container: &EntityContainer, _ctx: &mut ValidationContext) {}
}

struct ValidatorEngine {
    rules: Vec<Box<dyn ValidationRule>>,
}

impl Default for ValidatorEngine {
    fn default() -> Self {
        Self {
            rules: vec![
                Box::new(UniqueSchemaChildNamesRule),
                Box::new(UniqueEntityChildNamesRule),
                Box::new(UniqueComplexChildNamesRule),
                Box::new(UniqueEnumMemberNamesRule),
                Box::new(UniqueEntityContainerChildNamesRule),
                Box::new(NavigationPropertyBindingSemanticsRule),
                Box::new(KnownEntityContainerTargetsRule),
                Box::new(NavigationPartnerConsistencyRule),
                Box::new(ReferentialConstraintConsistencyRule),
                Box::new(TermBaseTermCycleRule),
                Box::new(BoundOperationBindingParameterRule),
                Box::new(OperationEntitySetPathRule),
                Box::new(DuplicateEntityKeyRule),
                Box::new(UnknownEntityKeyPropertyRule),
                Box::new(NonScalarEntityKeyRule),
            ],
        }
    }
}

impl ValidatorEngine {
    fn validate_model_collect(&self, model: &Model) -> Vec<ValidationError> {
        let mut ctx = ValidationContext::default();
        self.walk_model(model, &mut ctx);
        ctx.errors
    }

    fn apply_rules(&self, mut f: impl FnMut(&dyn ValidationRule)) {
        for rule in &self.rules {
            let _ = rule.name();
            f(rule.as_ref());
        }
    }

    fn walk_model(&self, model: &Model, ctx: &mut ValidationContext) {
        self.apply_rules(|rule| rule.visit_model(model, ctx));

        for element in &model.elements {
            match element.as_ref() {
                SchemaElement::EntityType(entity) => {
                    self.apply_rules(|rule| rule.visit_entity_type(entity, ctx));
                }
                SchemaElement::ComplexType(complex) => {
                    self.apply_rules(|rule| rule.visit_complex_type(complex, ctx));
                }
                SchemaElement::EnumType(enum_type) => {
                    self.apply_rules(|rule| rule.visit_enum_type(enum_type, ctx));
                }
                SchemaElement::Function(function) => {
                    self.apply_rules(|rule| rule.visit_function(function, ctx));
                }
                SchemaElement::Action(action) => {
                    self.apply_rules(|rule| rule.visit_action(action, ctx));
                }
                SchemaElement::TypeDefinition(_) => {}
                SchemaElement::Term(term) => {
                    self.apply_rules(|rule| rule.visit_term(term, ctx));
                }
            }
        }

        if let Some(container) = model.entity_container.as_ref() {
            self.apply_rules(|rule| rule.visit_entity_container(container, ctx));
        }
    }
}

fn report_duplicate_names<I>(
    names: I,
    parent_kind: &'static str,
    parent: String,
    child_kind: &'static str,
    ctx: &mut ValidationContext,
) where
    I: IntoIterator<Item = String>,
{
    let mut seen = HashSet::new();
    let mut reported = HashSet::new();

    for name in names {
        if !seen.insert(name.clone()) && reported.insert(name.clone()) {
            ctx.push(ValidationError::DuplicateChildName {
                parent_kind,
                parent: parent.clone(),
                child_kind,
                child: name,
            });
        }
    }
}

fn schema_element_name(element: &SchemaElement) -> &str {
    match element {
        SchemaElement::EntityType(e) => &e.name,
        SchemaElement::ComplexType(c) => &c.name,
        SchemaElement::EnumType(en) => &en.name,
        SchemaElement::TypeDefinition(td) => &td.name,
        SchemaElement::Term(t) => &t.name,
        SchemaElement::Function(f) => &f.name,
        SchemaElement::Action(a) => &a.name,
    }
}

struct UniqueSchemaChildNamesRule;

impl ValidationRule for UniqueSchemaChildNamesRule {
    fn name(&self) -> &'static str {
        "unique_schema_child_names"
    }

    fn visit_model(&self, model: &Model, ctx: &mut ValidationContext) {
        let names = model
            .elements
            .iter()
            .map(|element| schema_element_name(element.as_ref()).to_owned());

        report_duplicate_names(
            names,
            "Schema",
            model.namespace.clone(),
            "SchemaElement",
            ctx,
        );
    }
}

struct UniqueEntityChildNamesRule;

impl ValidationRule for UniqueEntityChildNamesRule {
    fn name(&self) -> &'static str {
        "unique_entity_child_names"
    }

    fn visit_entity_type(&self, entity: &EntityType, ctx: &mut ValidationContext) {
        let property_names = entity.properties().iter().map(|p| p.name.clone());
        let navigation_names = entity
            .navigation_properties()
            .iter()
            .map(|n| n.name.clone());

        report_duplicate_names(
            property_names.chain(navigation_names),
            "EntityType",
            entity.name.clone(),
            "PropertyOrNavigationProperty",
            ctx,
        );
    }
}

struct UniqueComplexChildNamesRule;

impl ValidationRule for UniqueComplexChildNamesRule {
    fn name(&self) -> &'static str {
        "unique_complex_child_names"
    }

    fn visit_complex_type(&self, complex: &ComplexType, ctx: &mut ValidationContext) {
        let property_names = complex.properties().iter().map(|p| p.name.clone());
        let navigation_names = complex
            .navigation_properties()
            .iter()
            .map(|n| n.name.clone());

        report_duplicate_names(
            property_names.chain(navigation_names),
            "ComplexType",
            complex.name.clone(),
            "PropertyOrNavigationProperty",
            ctx,
        );
    }
}

struct UniqueEnumMemberNamesRule;

impl ValidationRule for UniqueEnumMemberNamesRule {
    fn name(&self) -> &'static str {
        "unique_enum_member_names"
    }

    fn visit_enum_type(&self, enum_type: &EnumType, ctx: &mut ValidationContext) {
        let member_names = enum_type.members.iter().map(|m| m.name.clone());
        report_duplicate_names(
            member_names,
            "EnumType",
            enum_type.name.clone(),
            "Member",
            ctx,
        );
    }
}

struct UniqueEntityContainerChildNamesRule;

impl ValidationRule for UniqueEntityContainerChildNamesRule {
    fn name(&self) -> &'static str {
        "unique_entity_container_child_names"
    }

    fn visit_entity_container(&self, container: &EntityContainer, ctx: &mut ValidationContext) {
        let child_names = container.elements.iter().map(|el| match el.as_ref() {
            EntityContainerElement::EntitySet(es) => es.name.clone(),
            EntityContainerElement::Singleton(s) => s.name.clone(),
            EntityContainerElement::FunctionImport(i) => i.name.clone(),
            EntityContainerElement::ActionImport(i) => i.name.clone(),
        });

        report_duplicate_names(
            child_names,
            "EntityContainer",
            container.name.clone(),
            "EntityContainerElement",
            ctx,
        );
    }
}

struct DuplicateEntityKeyRule;

struct KnownEntityContainerTargetsRule;
struct NavigationPropertyBindingSemanticsRule;
struct NavigationPartnerConsistencyRule;
struct ReferentialConstraintConsistencyRule;
struct TermBaseTermCycleRule;
struct BoundOperationBindingParameterRule;
struct OperationEntitySetPathRule;

#[derive(Clone)]
enum StructuredTypeCursor {
    Entity(Arc<EntityType>),
    Complex(Arc<ComplexType>),
}

impl StructuredTypeCursor {
    fn property(&self, name: &str) -> Option<Arc<crate::edm::Property>> {
        match self {
            StructuredTypeCursor::Entity(entity) => entity
                .properties()
                .iter()
                .find(|property| property.name == name)
                .cloned(),
            StructuredTypeCursor::Complex(complex) => complex
                .properties()
                .iter()
                .find(|property| property.name == name)
                .cloned(),
        }
    }

    fn navigation_property(&self, name: &str) -> Option<Arc<crate::edm::NavigationProperty>> {
        match self {
            StructuredTypeCursor::Entity(entity) => entity
                .navigation_properties()
                .iter()
                .find(|navigation| navigation.name == name)
                .cloned(),
            StructuredTypeCursor::Complex(complex) => complex
                .navigation_properties()
                .iter()
                .find(|navigation| navigation.name == name)
                .cloned(),
        }
    }
}

enum BindingTargetStart {
    EntitySet,
    Singleton(Arc<EntityType>),
}

impl ValidationRule for NavigationPropertyBindingSemanticsRule {
    fn name(&self) -> &'static str {
        "navigation_property_binding_semantics"
    }

    fn visit_model(&self, model: &Model, ctx: &mut ValidationContext) {
        let Some(container) = model.entity_container.as_ref() else {
            return;
        };

        for element in &container.elements {
            match element.as_ref() {
                EntityContainerElement::EntitySet(set) => {
                    for binding in &set.navigation_property_bindings {
                        validate_navigation_binding_path(
                            container.name.as_str(),
                            "EntitySet.NavigationPropertyBinding",
                            set.name.as_str(),
                            set.target.clone(),
                            binding.path.as_str(),
                            ctx,
                        );

                        validate_navigation_binding_target(
                            model,
                            container,
                            "EntitySet.NavigationPropertyBinding",
                            set.name.as_str(),
                            binding.target.as_str(),
                            ctx,
                        );
                    }
                }
                EntityContainerElement::Singleton(singleton) => {
                    for binding in &singleton.navigation_property_bindings {
                        validate_navigation_binding_path(
                            container.name.as_str(),
                            "Singleton.NavigationPropertyBinding",
                            singleton.name.as_str(),
                            singleton.target.clone(),
                            binding.path.as_str(),
                            ctx,
                        );

                        validate_navigation_binding_target(
                            model,
                            container,
                            "Singleton.NavigationPropertyBinding",
                            singleton.name.as_str(),
                            binding.target.as_str(),
                            ctx,
                        );
                    }
                }
                EntityContainerElement::FunctionImport(_)
                | EntityContainerElement::ActionImport(_) => {}
            }
        }
    }
}

impl ValidationRule for KnownEntityContainerTargetsRule {
    fn name(&self) -> &'static str {
        "known_entity_container_targets"
    }

    fn visit_entity_container(&self, container: &EntityContainer, ctx: &mut ValidationContext) {
        let known_targets = container
            .elements
            .iter()
            .filter_map(|element| match element.as_ref() {
                EntityContainerElement::EntitySet(set) => Some(set.name.as_str()),
                EntityContainerElement::Singleton(singleton) => Some(singleton.name.as_str()),
                EntityContainerElement::FunctionImport(_)
                | EntityContainerElement::ActionImport(_) => None,
            })
            .collect::<HashSet<_>>();

        for element in &container.elements {
            match element.as_ref() {
                EntityContainerElement::EntitySet(set) => {
                    for binding in set.navigation_property_bindings() {
                        if let Some(target) = unresolved_binding_target(&binding.target) {
                            ctx.push(ValidationError::UnknownContainerTarget {
                                container: container.name.clone(),
                                source_kind: "EntitySet.NavigationPropertyBinding",
                                source: set.name.clone(),
                                target: target.to_owned(),
                            });
                        }
                    }
                }
                EntityContainerElement::Singleton(singleton) => {
                    for binding in singleton.navigation_property_bindings() {
                        if let Some(target) = unresolved_binding_target(&binding.target) {
                            ctx.push(ValidationError::UnknownContainerTarget {
                                container: container.name.clone(),
                                source_kind: "Singleton.NavigationPropertyBinding",
                                source: singleton.name.clone(),
                                target: target.to_owned(),
                            });
                        }
                    }
                }
                EntityContainerElement::FunctionImport(import_) => {
                    if let Some(target) = &import_.entity_set {
                        let target_name = first_path_segment(target);
                        if !known_targets.contains(target_name) {
                            ctx.push(ValidationError::UnknownContainerTarget {
                                container: container.name.clone(),
                                source_kind: "FunctionImport.EntitySet",
                                source: import_.name.clone(),
                                target: target.clone(),
                            });
                        }
                    }
                }
                EntityContainerElement::ActionImport(import_) => {
                    if let Some(target) = &import_.entity_set {
                        let target_name = first_path_segment(target);
                        if !known_targets.contains(target_name) {
                            ctx.push(ValidationError::UnknownContainerTarget {
                                container: container.name.clone(),
                                source_kind: "ActionImport.EntitySet",
                                source: import_.name.clone(),
                                target: target.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
}

impl ValidationRule for NavigationPartnerConsistencyRule {
    fn name(&self) -> &'static str {
        "navigation_partner_consistency"
    }

    fn visit_entity_type(&self, entity: &EntityType, ctx: &mut ValidationContext) {
        for navigation in entity.navigation_properties() {
            let Some(partner) = navigation.partner.as_deref() else {
                continue;
            };

            let Some(target) = navigation.target.upgrade() else {
                continue;
            };

            if !target
                .navigation_properties()
                .iter()
                .any(|target_navigation| target_navigation.name == partner)
            {
                ctx.push(ValidationError::UnknownNavigationPartner {
                    entity: entity.name.clone(),
                    navigation: navigation.name.clone(),
                    partner: partner.to_owned(),
                    target_entity: target.name.clone(),
                });
            }
        }
    }
}

impl ValidationRule for ReferentialConstraintConsistencyRule {
    fn name(&self) -> &'static str {
        "referential_constraint_consistency"
    }

    fn visit_entity_type(&self, entity: &EntityType, ctx: &mut ValidationContext) {
        for navigation in entity.navigation_properties() {
            let Some(target) = navigation.target.upgrade() else {
                continue;
            };

            for constraint in &navigation.referential_constraints {
                if !entity
                    .properties()
                    .iter()
                    .any(|property| property.name == constraint.property)
                {
                    ctx.push(ValidationError::UnknownReferentialConstraintProperty {
                        entity: entity.name.clone(),
                        navigation: navigation.name.clone(),
                        property: constraint.property.clone(),
                    });
                }

                if !target
                    .properties()
                    .iter()
                    .any(|property| property.name == constraint.referenced_property)
                {
                    ctx.push(
                        ValidationError::UnknownReferentialConstraintReferencedProperty {
                            entity: entity.name.clone(),
                            navigation: navigation.name.clone(),
                            target_entity: target.name.clone(),
                            referenced_property: constraint.referenced_property.clone(),
                        },
                    );
                }
            }
        }
    }
}

impl ValidationRule for TermBaseTermCycleRule {
    fn name(&self) -> &'static str {
        "term_base_term_cycle"
    }

    fn visit_term(&self, term: &Term, ctx: &mut ValidationContext) {
        let Some(start_base) = term.base_term.get().and_then(|base| base.as_ref()) else {
            return;
        };

        let mut seen = HashSet::new();
        seen.insert(term.name.clone());

        let mut current = start_base.upgrade();
        while let Some(current_term) = current {
            if !seen.insert(current_term.name.clone()) {
                ctx.push(ValidationError::CyclicTermBaseTerm {
                    term: term.name.clone(),
                });
                return;
            }

            current = current_term
                .base_term
                .get()
                .and_then(|base| base.as_ref())
                .and_then(|base| base.upgrade());
        }
    }
}

impl ValidationRule for BoundOperationBindingParameterRule {
    fn name(&self) -> &'static str {
        "bound_operation_binding_parameter"
    }

    fn visit_function(&self, function: &Function, ctx: &mut ValidationContext) {
        if function.is_bound && function.parameters.is_empty() {
            ctx.push(ValidationError::BoundOperationMissingBindingParameter {
                operation_kind: "Function",
                operation: function.name.clone(),
            });
        }
    }

    fn visit_action(&self, action: &Action, ctx: &mut ValidationContext) {
        if action.is_bound && action.parameters.is_empty() {
            ctx.push(ValidationError::BoundOperationMissingBindingParameter {
                operation_kind: "Action",
                operation: action.name.clone(),
            });
        }
    }
}

impl ValidationRule for OperationEntitySetPathRule {
    fn name(&self) -> &'static str {
        "operation_entity_set_path"
    }

    fn visit_function(&self, function: &Function, ctx: &mut ValidationContext) {
        validate_entity_set_path(
            "Function",
            &function.name,
            function.is_bound,
            &function.parameters,
            function.entity_set_path.as_deref(),
            ctx,
        );
    }

    fn visit_action(&self, action: &Action, ctx: &mut ValidationContext) {
        validate_entity_set_path(
            "Action",
            &action.name,
            action.is_bound,
            &action.parameters,
            action.entity_set_path.as_deref(),
            ctx,
        );
    }
}

impl ValidationRule for DuplicateEntityKeyRule {
    fn name(&self) -> &'static str {
        "duplicate_entity_key"
    }

    fn visit_entity_type(&self, entity: &EntityType, ctx: &mut ValidationContext) {
        let mut seen = HashSet::new();
        for key in &entity.keys {
            if !seen.insert(key) {
                ctx.push(ValidationError::DuplicateKey {
                    entity: entity.name.clone(),
                    key: key.clone(),
                });
            }
        }
    }
}

struct UnknownEntityKeyPropertyRule;

impl ValidationRule for UnknownEntityKeyPropertyRule {
    fn name(&self) -> &'static str {
        "unknown_entity_key_property"
    }

    fn visit_entity_type(&self, entity: &EntityType, ctx: &mut ValidationContext) {
        for key in &entity.keys {
            if classify_key_path(entity, key) == KeyPathClassification::Unknown {
                ctx.push(ValidationError::UnknownKeyProperty {
                    entity: entity.name.clone(),
                    key: key.clone(),
                });
            }
        }
    }
}

struct NonScalarEntityKeyRule;

impl ValidationRule for NonScalarEntityKeyRule {
    fn name(&self) -> &'static str {
        "non_scalar_entity_key"
    }

    fn visit_entity_type(&self, entity: &EntityType, ctx: &mut ValidationContext) {
        for key in &entity.keys {
            let classification = classify_key_path(entity, key);
            if matches!(
                classification,
                KeyPathClassification::Complex | KeyPathClassification::Collection
            ) {
                ctx.push(ValidationError::NonScalarKeyProperty {
                    entity: entity.name.clone(),
                    key: key.clone(),
                });
            }
        }
    }
}

fn classify_key_path(entity: &EntityType, key_path: &str) -> KeyPathClassification {
    let mut segments = key_path.split('/').filter(|segment| !segment.is_empty());

    let Some(first_segment) = segments.next() else {
        return KeyPathClassification::Unknown;
    };

    let Some(first_property) = entity
        .properties()
        .iter()
        .find(|property| property.name == first_segment)
    else {
        return KeyPathClassification::Unknown;
    };

    if first_property.is_collection {
        return KeyPathClassification::Collection;
    }

    let mut current_type = &first_property.ty;

    for segment in segments {
        let ResolvedType::Complex(complex) = current_type else {
            return KeyPathClassification::Unknown;
        };

        let Some(next_property) = complex
            .properties()
            .iter()
            .find(|property| property.name == segment)
        else {
            return KeyPathClassification::Unknown;
        };

        if next_property.is_collection {
            return KeyPathClassification::Collection;
        }

        current_type = &next_property.ty;
    }

    match current_type {
        ResolvedType::Complex(_) => KeyPathClassification::Complex,
        ResolvedType::Primitive(_) | ResolvedType::Enum(_) | ResolvedType::TypeDefinition(_) => {
            KeyPathClassification::Scalar
        }
    }
}

fn validate_entity_set_path(
    operation_kind: &'static str,
    operation_name: &str,
    is_bound: bool,
    parameters: &[crate::edm::OperationParameter],
    entity_set_path: Option<&str>,
    ctx: &mut ValidationContext,
) {
    let Some(path) = entity_set_path else {
        return;
    };

    if !is_bound {
        ctx.push(ValidationError::InvalidEntitySetPath {
            operation_kind,
            operation: operation_name.to_owned(),
            path: path.to_owned(),
            reason: "EntitySetPath requires a bound operation",
        });
        return;
    }

    if parameters.is_empty() {
        ctx.push(ValidationError::InvalidEntitySetPath {
            operation_kind,
            operation: operation_name.to_owned(),
            path: path.to_owned(),
            reason: "EntitySetPath requires a binding parameter",
        });
        return;
    }

    let first_segment = first_path_segment(path);
    let binding_parameter_name = parameters[0].name.as_str();
    if first_segment != binding_parameter_name {
        ctx.push(ValidationError::InvalidEntitySetPath {
            operation_kind,
            operation: operation_name.to_owned(),
            path: path.to_owned(),
            reason: "EntitySetPath must start with the binding parameter name",
        });
    }
}

fn validate_navigation_binding_path(
    container_name: &str,
    source_kind: &'static str,
    source_name: &str,
    source_type: Arc<EntityType>,
    path: &str,
    ctx: &mut ValidationContext,
) {
    let mut segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .peekable();
    if segments.peek().is_none() {
        ctx.push(ValidationError::InvalidNavigationPropertyBinding {
            container: container_name.to_owned(),
            source_kind,
            source: source_name.to_owned(),
            attribute: "Path",
            value: path.to_owned(),
            reason: "Path must not be empty",
        });
        return;
    }

    let mut effective_segments = segments.collect::<Vec<_>>();
    let has_terminal_type_cast = effective_segments
        .last()
        .map(|segment| segment.contains('.'))
        .unwrap_or(false);
    if has_terminal_type_cast {
        if effective_segments.len() == 1 {
            ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                container: container_name.to_owned(),
                source_kind,
                source: source_name.to_owned(),
                attribute: "Path",
                value: path.to_owned(),
                reason: "Terminal type-cast requires a preceding navigation segment",
            });
            return;
        }
        effective_segments.pop();
    }

    let mut current = StructuredTypeCursor::Entity(source_type);
    let mut saw_final_non_containment_navigation = false;

    for (index, segment) in effective_segments.iter().enumerate() {
        let is_last = index + 1 == effective_segments.len();

        if segment.contains('.') {
            // Type-cast segments are allowed, but detailed cast compatibility
            // requires inheritance metadata that is currently not tracked here.
            continue;
        }

        if let Some(navigation) = current.navigation_property(segment) {
            let contains_target = navigation.contains_target.unwrap_or(false);
            if is_last {
                if contains_target {
                    ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                        container: container_name.to_owned(),
                        source_kind,
                        source: source_name.to_owned(),
                        attribute: "Path",
                        value: path.to_owned(),
                        reason: "Final navigation segment in Path must be non-containment",
                    });
                    return;
                }
                saw_final_non_containment_navigation = true;
            } else if !contains_target {
                ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                    container: container_name.to_owned(),
                    source_kind,
                    source: source_name.to_owned(),
                    attribute: "Path",
                    value: path.to_owned(),
                    reason: "Only containment navigation segments are allowed before the final segment",
                });
                return;
            }

            let Some(target_entity) = navigation.target.upgrade() else {
                ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                    container: container_name.to_owned(),
                    source_kind,
                    source: source_name.to_owned(),
                    attribute: "Path",
                    value: path.to_owned(),
                    reason: "Navigation segment target cannot be resolved",
                });
                return;
            };
            current = StructuredTypeCursor::Entity(target_entity);
            continue;
        }

        let Some(property) = current.property(segment) else {
            ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                container: container_name.to_owned(),
                source_kind,
                source: source_name.to_owned(),
                attribute: "Path",
                value: path.to_owned(),
                reason: "Path segment does not resolve to a property or navigation property",
            });
            return;
        };

        let ResolvedType::Complex(complex) = &property.ty else {
            ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                container: container_name.to_owned(),
                source_kind,
                source: source_name.to_owned(),
                attribute: "Path",
                value: path.to_owned(),
                reason: "Non-navigation path segments must resolve to complex properties",
            });
            return;
        };

        current = StructuredTypeCursor::Complex(complex.clone());
    }

    if !saw_final_non_containment_navigation {
        ctx.push(ValidationError::InvalidNavigationPropertyBinding {
            container: container_name.to_owned(),
            source_kind,
            source: source_name.to_owned(),
            attribute: "Path",
            value: path.to_owned(),
            reason: "Path must end in a navigation property segment",
        });
    }
}

fn validate_navigation_binding_target(
    model: &Model,
    container: &EntityContainer,
    source_kind: &'static str,
    source_name: &str,
    target: &str,
    ctx: &mut ValidationContext,
) {
    let mut segments = target
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        ctx.push(ValidationError::InvalidNavigationPropertyBinding {
            container: container.name.clone(),
            source_kind,
            source: source_name.to_owned(),
            attribute: "Target",
            value: target.to_owned(),
            reason: "Target must not be empty",
        });
        return;
    }

    let qualified_container_name = format!("{}.{}", model.namespace, container.name);
    if segments[0] == qualified_container_name {
        if segments.len() == 1 {
            ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                container: container.name.clone(),
                source_kind,
                source: source_name.to_owned(),
                attribute: "Target",
                value: target.to_owned(),
                reason: "Qualified container prefix in Target must be followed by a container child",
            });
            return;
        }
        segments.remove(0);
    }

    let start = resolve_binding_target_start(container, segments[0]);
    let Some(start) = start else {
        return;
    };

    if segments.len() == 1 {
        return;
    }

    let BindingTargetStart::Singleton(singleton_type) = start else {
        ctx.push(ValidationError::InvalidNavigationPropertyBinding {
            container: container.name.clone(),
            source_kind,
            source: source_name.to_owned(),
            attribute: "Target",
            value: target.to_owned(),
            reason: "Target paths with additional segments must start from a singleton",
        });
        return;
    };

    let mut current = StructuredTypeCursor::Entity(singleton_type);
    for (index, segment) in segments.iter().enumerate().skip(1) {
        let is_last = index + 1 == segments.len();

        if let Some(navigation) = current.navigation_property(segment) {
            let contains_target = navigation.contains_target.unwrap_or(false);
            if !contains_target {
                ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                    container: container.name.clone(),
                    source_kind,
                    source: source_name.to_owned(),
                    attribute: "Target",
                    value: target.to_owned(),
                    reason: "Target path navigation segments must be containment navigation properties",
                });
                return;
            }

            if !is_last && navigation.is_collection {
                ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                    container: container.name.clone(),
                    source_kind,
                    source: source_name.to_owned(),
                    attribute: "Target",
                    value: target.to_owned(),
                    reason: "Intermediate Target path segments must be single-valued",
                });
                return;
            }

            let Some(target_entity) = navigation.target.upgrade() else {
                ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                    container: container.name.clone(),
                    source_kind,
                    source: source_name.to_owned(),
                    attribute: "Target",
                    value: target.to_owned(),
                    reason: "Target path navigation segment target cannot be resolved",
                });
                return;
            };
            current = StructuredTypeCursor::Entity(target_entity);
            continue;
        }

        let Some(property) = current.property(segment) else {
            ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                container: container.name.clone(),
                source_kind,
                source: source_name.to_owned(),
                attribute: "Target",
                value: target.to_owned(),
                reason: "Target path segment does not resolve to a complex property or containment navigation property",
            });
            return;
        };

        if property.is_collection {
            ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                container: container.name.clone(),
                source_kind,
                source: source_name.to_owned(),
                attribute: "Target",
                value: target.to_owned(),
                reason: "Target path property segments must be single-valued complex properties",
            });
            return;
        }

        let ResolvedType::Complex(complex) = &property.ty else {
            ctx.push(ValidationError::InvalidNavigationPropertyBinding {
                container: container.name.clone(),
                source_kind,
                source: source_name.to_owned(),
                attribute: "Target",
                value: target.to_owned(),
                reason: "Target path property segments must be complex properties",
            });
            return;
        };

        current = StructuredTypeCursor::Complex(complex.clone());
    }
}

fn resolve_binding_target_start<'a>(
    container: &'a EntityContainer,
    first_segment: &str,
) -> Option<BindingTargetStart> {
    for element in &container.elements {
        match element.as_ref() {
            EntityContainerElement::EntitySet(set) if set.name == first_segment => {
                return Some(BindingTargetStart::EntitySet);
            }
            EntityContainerElement::Singleton(singleton) if singleton.name == first_segment => {
                return Some(BindingTargetStart::Singleton(singleton.target.clone()));
            }
            EntityContainerElement::EntitySet(_)
            | EntityContainerElement::Singleton(_)
            | EntityContainerElement::FunctionImport(_)
            | EntityContainerElement::ActionImport(_) => {}
        }
    }

    None
}

fn first_path_segment(path: &str) -> &str {
    path.split('/').next().unwrap_or(path)
}

/// Returns the authored name of the first unresolved segment in a binding's
/// resolved target path, if any. A resolved target that contains a
/// [`BindingPath::Unresolved`] segment is a target the resolver could not bind
/// to a known entity set / singleton.
fn unresolved_binding_target(target: &[BindingPath]) -> Option<&str> {
    target.iter().find_map(|segment| match segment {
        BindingPath::Unresolved(name) => Some(name.as_str()),
        _ => None,
    })
}
