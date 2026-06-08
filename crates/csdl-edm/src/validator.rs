//! Semantic validation that is intentionally separate from reference resolution.

use std::collections::HashSet;

use crate::edm::{
    Action, ComplexType, DocumentModel, EntityContainer, EntityContainerElement, EntityType,
    EnumType, Function, Model, ResolvedType, SchemaElement, Term,
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
struct NavigationPartnerConsistencyRule;
struct ReferentialConstraintConsistencyRule;
struct TermBaseTermCycleRule;
struct BoundOperationBindingParameterRule;
struct OperationEntitySetPathRule;

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
                    for binding in &set.navigation_property_bindings {
                        let target = first_path_segment(&binding.target);
                        if !known_targets.contains(target) {
                            ctx.push(ValidationError::UnknownContainerTarget {
                                container: container.name.clone(),
                                source_kind: "EntitySet.NavigationPropertyBinding",
                                source: set.name.clone(),
                                target: binding.target.clone(),
                            });
                        }
                    }
                }
                EntityContainerElement::Singleton(singleton) => {
                    for binding in &singleton.navigation_property_bindings {
                        let target = first_path_segment(&binding.target);
                        if !known_targets.contains(target) {
                            ctx.push(ValidationError::UnknownContainerTarget {
                                container: container.name.clone(),
                                source_kind: "Singleton.NavigationPropertyBinding",
                                source: singleton.name.clone(),
                                target: binding.target.clone(),
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

fn first_path_segment(path: &str) -> &str {
    path.split('/').next().unwrap_or(path)
}
