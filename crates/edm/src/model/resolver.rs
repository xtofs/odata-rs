//! The resolver — turns a [`syntactic::EdmModel`] into a semantic
//! [`EdmModel`] by registering schemas, assigning IDs, and replacing every
//! `String` reference with a typed handle.
//!
//! Structure: three passes.
//! 1. Register schemas + aliases.
//! 2. Discover named items and assign IDs (so phase-3 references can resolve
//!    forward as well as backward).
//! 3. Consume the syntactic model and build resolved structs.
//!
//! Errors accumulate; on failure the resolver returns *every* collected
//! diagnostic instead of stopping at the first one.

use std::collections::HashMap;
use std::num::NonZeroU32;

use super::path::{NamedRef, TargetPath};
use super::{
    Action, ActionImport, CallableId, ComplexType, ComplexTypeId, EdmModel, EntityContainer,
    EntityContainerId, EntitySet, EntitySetId, EntityType, EntityTypeId, EnumMember, EnumType,
    EnumTypeId, Function, FunctionImport, Key, NamedElementRef, NamedTypeId, NavigationProperty,
    NavigationPropertyBinding, PrimitiveTypeId, Property, PropertyRef, QualifiedName,
    ReferentialConstraint, ResolutionError, ResolutionErrorKind, SchemaInfo, Singleton,
    SingletonId, StructuralTypeId, Term, TypeDef, TypeDefId, TypeRef,
};
use crate::syntactic;

// ============================================================================
// Public entry points
// ============================================================================

pub(super) fn run(parsed: syntactic::EdmModel) -> Result<EdmModel, Vec<ResolutionError>> {
    let mut model = EdmModel::new();
    let mut errors: Vec<ResolutionError> = Vec::new();

    // ----- Phase 1: register schemas + aliases ------------------------------
    for schema in &parsed.schemas {
        model.schemas.push(SchemaInfo {
            namespace: schema.namespace.clone(),
            alias: schema.alias.clone(),
            is_builtin: false,
            annotations: Vec::new(), // populated in phase 3
        });
        if let Some(alias) = &schema.alias {
            if model.aliases.contains_key(alias) {
                errors.push(ResolutionError {
                    at: alias.clone(),
                    kind: ResolutionErrorKind::DuplicateAlias(alias.clone()),
                });
            } else {
                model
                    .aliases
                    .insert(alias.clone(), schema.namespace.clone());
            }
        }
    }

    // ----- Phase 2: discover named items + assign IDs ----------------------
    let mut disc = Discovery::default();

    let mut next_et: u32 = 1;
    for schema in &parsed.schemas {
        for et in &schema.entity_types {
            let qname = QualifiedName::new(&schema.namespace, &et.name);
            if check_unique_type(&model, &qname, &mut errors) {
                let id = EntityTypeId(NonZeroU32::new(next_et).unwrap());
                next_et += 1;
                disc.entity_types.insert(qname.clone(), id);
                model.type_by_qname.insert(qname, NamedTypeId::Entity(id));
            }
        }
    }

    let mut next_ct: u32 = 1;
    for schema in &parsed.schemas {
        for ct in &schema.complex_types {
            let qname = QualifiedName::new(&schema.namespace, &ct.name);
            if check_unique_type(&model, &qname, &mut errors) {
                let id = ComplexTypeId(NonZeroU32::new(next_ct).unwrap());
                next_ct += 1;
                disc.complex_types.insert(qname.clone(), id);
                model.type_by_qname.insert(qname, NamedTypeId::Complex(id));
            }
        }
    }

    let mut next_en: u32 = 1;
    for schema in &parsed.schemas {
        for en in &schema.enum_types {
            let qname = QualifiedName::new(&schema.namespace, &en.name);
            if check_unique_type(&model, &qname, &mut errors) {
                let id = EnumTypeId(NonZeroU32::new(next_en).unwrap());
                next_en += 1;
                disc.enum_types.insert(qname.clone(), id);
                model.type_by_qname.insert(qname, NamedTypeId::Enum(id));
            }
        }
    }

    let mut next_td: u32 = 1;
    for schema in &parsed.schemas {
        for td in &schema.type_definitions {
            let qname = QualifiedName::new(&schema.namespace, &td.name);
            if check_unique_type(&model, &qname, &mut errors) {
                let id = TypeDefId(NonZeroU32::new(next_td).unwrap());
                next_td += 1;
                disc.type_defs.insert(qname.clone(), id);
                model.type_by_qname.insert(qname, NamedTypeId::TypeDef(id));
            }
        }
    }

    let mut next_c: u32 = 1;
    for schema in &parsed.schemas {
        for ec in &schema.entity_containers {
            let qname = QualifiedName::new(&schema.namespace, &ec.name);
            if model.containers_by_qname.contains_key(&qname) {
                errors.push(ResolutionError {
                    at: qname.to_string(),
                    kind: ResolutionErrorKind::DuplicateName(qname.clone()),
                });
                continue;
            }
            let cid = EntityContainerId(NonZeroU32::new(next_c).unwrap());
            next_c += 1;
            disc.containers.insert(qname.clone(), cid);
            model.containers_by_qname.insert(qname.clone(), cid);

            // Container children get IDs too — keyed by (container qname, child name).
            let mut next_es: u32 = model.entity_sets.len() as u32 + 1;
            for es in &ec.entity_sets {
                let id = EntitySetId(NonZeroU32::new(next_es).unwrap());
                next_es += 1;
                disc.entity_sets.insert((qname.clone(), es.name.clone()), id);
                // reserve the slot — we push placeholder Below, then overwrite in phase 3.
                model.entity_sets.push(placeholder_entity_set(cid));
            }
            let mut next_st: u32 = model.singletons.len() as u32 + 1;
            for s in &ec.singletons {
                let id = SingletonId(NonZeroU32::new(next_st).unwrap());
                next_st += 1;
                disc.singletons.insert((qname.clone(), s.name.clone()), id);
                model.singletons.push(placeholder_singleton(cid));
            }
        }
    }

    // ----- Phase 3: consume the parsed model, build & push resolved structs.
    //
    // We pull per-kind vectors out across all schemas so the push order
    // matches phase-2's ID assignment.
    let mut all_entity_types: Vec<(String, syntactic::EntityType)> = Vec::new();
    let mut all_complex_types: Vec<(String, syntactic::ComplexType)> = Vec::new();
    let mut all_enum_types: Vec<(String, syntactic::EnumType)> = Vec::new();
    let mut all_type_defs: Vec<(String, syntactic::TypeDefinition)> = Vec::new();
    let mut all_containers: Vec<(String, syntactic::EntityContainer)> = Vec::new();
    let mut schema_annotations: Vec<(usize, Vec<crate::expr::Annotation>)> = Vec::new();

    for (schema_index, schema) in parsed.schemas.into_iter().enumerate() {
        let syntactic::Schema {
            namespace,
            alias: _,
            entity_types,
            complex_types,
            enum_types,
            type_definitions,
            entity_containers,
            annotations,
        } = schema;
        schema_annotations.push((schema_index, annotations));
        for et in entity_types {
            all_entity_types.push((namespace.clone(), et));
        }
        for ct in complex_types {
            all_complex_types.push((namespace.clone(), ct));
        }
        for en in enum_types {
            all_enum_types.push((namespace.clone(), en));
        }
        for td in type_definitions {
            all_type_defs.push((namespace.clone(), td));
        }
        for ec in entity_containers {
            all_containers.push((namespace.clone(), ec));
        }
    }

    // Edm.Untyped fallback for broken type references.
    let untyped: NamedTypeId = model
        .lookup_type(&QualifiedName::new("Edm", "Untyped"))
        .expect("Edm.Untyped present in builtin schema");

    // Build entity types
    for (namespace, et) in all_entity_types {
        // The qname this slot corresponds to per phase-2 ID assignment:
        let qname = QualifiedName::new(&namespace, &et.name);
        // If discovery dropped this due to duplicate, skip — we did NOT
        // allocate an ID for it.
        if !disc.entity_types.contains_key(&qname) {
            continue;
        }
        let resolved = build_entity_type(et, qname, &disc, &model, untyped, &mut errors);
        model.entity_types.push(resolved);
    }

    for (namespace, ct) in all_complex_types {
        let qname = QualifiedName::new(&namespace, &ct.name);
        if !disc.complex_types.contains_key(&qname) {
            continue;
        }
        let resolved = build_complex_type(ct, qname, &disc, &model, untyped, &mut errors);
        model.complex_types.push(resolved);
    }

    for (namespace, en) in all_enum_types {
        let qname = QualifiedName::new(&namespace, &en.name);
        if !disc.enum_types.contains_key(&qname) {
            continue;
        }
        let resolved = build_enum_type(en, qname, &model, &mut errors);
        model.enum_types.push(resolved);
    }

    for (namespace, td) in all_type_defs {
        let qname = QualifiedName::new(&namespace, &td.name);
        if !disc.type_defs.contains_key(&qname) {
            continue;
        }
        let resolved = build_type_def(td, qname, &model, untyped, &mut errors);
        model.type_definitions.push(resolved);
    }

    for (namespace, ec) in all_containers {
        let qname = QualifiedName::new(&namespace, &ec.name);
        let cid = match disc.containers.get(&qname) {
            Some(c) => *c,
            None => continue,
        };
        build_entity_container(ec, qname, cid, &disc, &mut model, untyped, &mut errors);
    }

    // Attach schema-level annotations now that the SchemaInfo slots exist.
    for (idx, anns) in schema_annotations {
        // The +1 offset accounts for the builtin Edm schema at index 0.
        let target = idx + 1;
        if let Some(info) = model.schemas.get_mut(target) {
            info.annotations = anns;
        }
    }

    if errors.is_empty() {
        Ok(model)
    } else {
        Err(errors)
    }
}

pub(super) fn resolve_path(
    model: &EdmModel,
    path: &TargetPath,
) -> Result<NamedElementRef, ResolutionError> {
    let mut current = resolve_base(model, &path.base)?;
    for segment in &path.segments {
        current = walk_segment(model, &current, segment)
            .ok_or_else(|| ResolutionError {
                at: format_path(path),
                kind: ResolutionErrorKind::BrokenSegment {
                    parent: current.clone(),
                    segment: segment.clone(),
                },
            })?;
    }
    if let Some(suffix) = &path.annotation {
        let canonical = canonicalize_qname(&suffix.term, &model.aliases);
        let term_id = model
            .terms_by_qname
            .get(&canonical)
            .copied()
            .ok_or_else(|| ResolutionError {
                at: suffix.term.to_string(),
                kind: ResolutionErrorKind::UnknownTerm(canonical),
            })?;
        return Ok(NamedElementRef::AnnotationUsage {
            target: Box::new(current),
            term: term_id,
            qualifier: suffix.qualifier.clone(),
        });
    }
    Ok(current)
}

// ============================================================================
// Discovery — temporary ID maps populated in phase 2 and consumed in phase 3.
// ============================================================================

#[derive(Default)]
struct Discovery {
    entity_types: HashMap<QualifiedName, EntityTypeId>,
    complex_types: HashMap<QualifiedName, ComplexTypeId>,
    enum_types: HashMap<QualifiedName, EnumTypeId>,
    type_defs: HashMap<QualifiedName, TypeDefId>,
    containers: HashMap<QualifiedName, EntityContainerId>,
    entity_sets: HashMap<(QualifiedName, String), EntitySetId>,
    singletons: HashMap<(QualifiedName, String), SingletonId>,
}

// ============================================================================
// Phase 2 helpers
// ============================================================================

fn check_unique_type(
    model: &EdmModel,
    qname: &QualifiedName,
    errors: &mut Vec<ResolutionError>,
) -> bool {
    if model.type_by_qname.contains_key(qname) {
        errors.push(ResolutionError {
            at: qname.to_string(),
            kind: ResolutionErrorKind::DuplicateName(qname.clone()),
        });
        false
    } else {
        true
    }
}

fn placeholder_entity_set(container: EntityContainerId) -> EntitySet {
    // Pushed during phase 2 so IDs match; fields are overwritten in phase 3.
    EntitySet {
        container,
        name: String::new(),
        entity_type: dummy_entity_type_id(),
        include_in_service_document: true,
        navigation_property_bindings: Vec::new(),
        annotations: Vec::new(),
    }
}

fn placeholder_singleton(container: EntityContainerId) -> Singleton {
    Singleton {
        container,
        name: String::new(),
        type_: TypeRef::Named(dummy_primitive_id()),
        navigation_property_bindings: Vec::new(),
        annotations: Vec::new(),
    }
}

fn dummy_entity_type_id() -> EntityTypeId {
    // Will be overwritten before the model is returned. If it ever leaks
    // through, indexing the arena with it panics — surfacing the bug loudly.
    EntityTypeId(NonZeroU32::new(u32::MAX).unwrap())
}

fn dummy_primitive_id() -> NamedTypeId {
    NamedTypeId::Primitive(PrimitiveTypeId(NonZeroU32::new(1).unwrap()))
}

// ============================================================================
// Phase 3 builders
// ============================================================================

fn build_entity_type(
    et: syntactic::EntityType,
    qualified_name: QualifiedName,
    disc: &Discovery,
    model: &EdmModel,
    untyped: NamedTypeId,
    errors: &mut Vec<ResolutionError>,
) -> EntityType {
    let base_type = et.base_type.and_then(|raw| {
        resolve_named_qualified(&raw, &model.aliases)
            .and_then(|q| disc.entity_types.get(&q).copied())
            .or_else(|| {
                errors.push(ResolutionError {
                    at: format!("EntityType {qualified_name}: BaseType {raw}"),
                    kind: ResolutionErrorKind::UnknownType(
                        raw.parse().unwrap_or_else(|_| QualifiedName::new("", &raw)),
                    ),
                });
                None
            })
    });

    let key = et.key.map(|k| Key {
        property_refs: k
            .property_refs
            .into_iter()
            .map(|pr| PropertyRef {
                name: pr.name,
                alias: pr.alias,
            })
            .collect(),
    });

    let context = format!("EntityType {qualified_name}");
    let properties = et
        .properties
        .into_iter()
        .map(|p| build_property(p, &context, model, untyped, errors))
        .collect();
    let navigation_properties = et
        .navigation_properties
        .into_iter()
        .map(|np| build_navigation_property(np, &context, model, untyped, errors))
        .collect();

    EntityType {
        qualified_name,
        base_type,
        abstract_: et.abstract_,
        open_type: et.open_type,
        has_stream: et.has_stream,
        key,
        properties,
        navigation_properties,
        annotations: et.annotations,
    }
}

fn build_complex_type(
    ct: syntactic::ComplexType,
    qualified_name: QualifiedName,
    disc: &Discovery,
    model: &EdmModel,
    untyped: NamedTypeId,
    errors: &mut Vec<ResolutionError>,
) -> ComplexType {
    let base_type = ct.base_type.and_then(|raw| {
        resolve_named_qualified(&raw, &model.aliases)
            .and_then(|q| disc.complex_types.get(&q).copied())
            .or_else(|| {
                errors.push(ResolutionError {
                    at: format!("ComplexType {qualified_name}: BaseType {raw}"),
                    kind: ResolutionErrorKind::UnknownType(
                        raw.parse().unwrap_or_else(|_| QualifiedName::new("", &raw)),
                    ),
                });
                None
            })
    });

    let context = format!("ComplexType {qualified_name}");
    let properties = ct
        .properties
        .into_iter()
        .map(|p| build_property(p, &context, model, untyped, errors))
        .collect();
    let navigation_properties = ct
        .navigation_properties
        .into_iter()
        .map(|np| build_navigation_property(np, &context, model, untyped, errors))
        .collect();

    ComplexType {
        qualified_name,
        base_type,
        abstract_: ct.abstract_,
        open_type: ct.open_type,
        properties,
        navigation_properties,
        annotations: ct.annotations,
    }
}

fn build_enum_type(
    en: syntactic::EnumType,
    qualified_name: QualifiedName,
    model: &EdmModel,
    errors: &mut Vec<ResolutionError>,
) -> EnumType {
    let underlying_type = en.underlying_type.and_then(|raw| {
        let parsed: Result<QualifiedName, _> = raw.parse();
        match parsed {
            Ok(q) => {
                let canonical = canonicalize_qname(&q, &model.aliases);
                match model.type_by_qname.get(&canonical) {
                    Some(NamedTypeId::Primitive(p)) => Some(*p),
                    Some(_) | None => {
                        errors.push(ResolutionError {
                            at: format!("EnumType {qualified_name}: UnderlyingType {raw}"),
                            kind: ResolutionErrorKind::UnknownType(canonical),
                        });
                        None
                    }
                }
            }
            Err(_) => {
                errors.push(ResolutionError {
                    at: format!("EnumType {qualified_name}: UnderlyingType"),
                    kind: ResolutionErrorKind::InvalidTypeReference(raw),
                });
                None
            }
        }
    });

    let members = en
        .members
        .into_iter()
        .map(|m| EnumMember {
            name: m.name,
            value: m.value,
            annotations: m.annotations,
        })
        .collect();

    EnumType {
        qualified_name,
        underlying_type,
        is_flags: en.is_flags,
        members,
        annotations: en.annotations,
    }
}

fn build_type_def(
    td: syntactic::TypeDefinition,
    qualified_name: QualifiedName,
    model: &EdmModel,
    untyped: NamedTypeId,
    errors: &mut Vec<ResolutionError>,
) -> TypeDef {
    let context = format!("TypeDefinition {qualified_name}");
    let underlying_type = match resolve_type_ref(
        &td.underlying_type,
        &model.type_by_qname,
        &model.aliases,
        untyped,
        &context,
        errors,
    ) {
        TypeRef::Named(id) => id,
        TypeRef::Collection(_) => {
            // TypeDefinitions can't wrap a collection per spec; record and
            // fall back to Untyped.
            errors.push(ResolutionError {
                at: format!("{context}: Collection() not valid as UnderlyingType"),
                kind: ResolutionErrorKind::InvalidTypeReference(td.underlying_type),
            });
            untyped
        }
    };

    TypeDef {
        qualified_name,
        underlying_type,
        facets: td.facets,
        annotations: td.annotations,
    }
}

fn build_property(
    p: syntactic::Property,
    parent_context: &str,
    model: &EdmModel,
    untyped: NamedTypeId,
    errors: &mut Vec<ResolutionError>,
) -> Property {
    let context = format!("{parent_context}.Property {}", p.name);
    let type_ = resolve_type_ref(
        &p.type_,
        &model.type_by_qname,
        &model.aliases,
        untyped,
        &context,
        errors,
    );
    Property {
        name: p.name,
        type_,
        nullable: p.nullable,
        facets: p.facets,
        annotations: p.annotations,
    }
}

fn build_navigation_property(
    np: syntactic::NavigationProperty,
    parent_context: &str,
    model: &EdmModel,
    untyped: NamedTypeId,
    errors: &mut Vec<ResolutionError>,
) -> NavigationProperty {
    let context = format!("{parent_context}.NavigationProperty {}", np.name);
    let type_ = resolve_type_ref(
        &np.type_,
        &model.type_by_qname,
        &model.aliases,
        untyped,
        &context,
        errors,
    );
    let referential_constraints = np
        .referential_constraints
        .into_iter()
        .map(|rc| ReferentialConstraint {
            property: rc.property,
            referenced_property: rc.referenced_property,
            annotations: rc.annotations,
        })
        .collect();
    NavigationProperty {
        name: np.name,
        type_,
        nullable: np.nullable,
        partner: np.partner,
        contains_target: np.contains_target,
        referential_constraints,
        on_delete: np.on_delete,
        annotations: np.annotations,
    }
}

fn build_entity_container(
    ec: syntactic::EntityContainer,
    qualified_name: QualifiedName,
    cid: EntityContainerId,
    disc: &Discovery,
    model: &mut EdmModel,
    untyped: NamedTypeId,
    errors: &mut Vec<ResolutionError>,
) {
    let extends = ec.extends.and_then(|raw| {
        let parsed: Result<QualifiedName, _> = raw.parse();
        match parsed {
            Ok(q) => {
                let canonical = canonicalize_qname(&q, &model.aliases);
                match model.containers_by_qname.get(&canonical) {
                    Some(id) => Some(*id),
                    None => {
                        errors.push(ResolutionError {
                            at: format!("EntityContainer {qualified_name}: Extends {raw}"),
                            kind: ResolutionErrorKind::UnknownContainer(canonical),
                        });
                        None
                    }
                }
            }
            Err(_) => {
                errors.push(ResolutionError {
                    at: format!("EntityContainer {qualified_name}: Extends"),
                    kind: ResolutionErrorKind::InvalidTypeReference(raw),
                });
                None
            }
        }
    });

    let mut entity_set_ids: Vec<EntitySetId> = Vec::new();
    let mut singleton_ids: Vec<SingletonId> = Vec::new();

    let context_container = format!("EntityContainer {qualified_name}");

    // Build EntitySets. IDs were pre-assigned in phase 2; we now overwrite
    // the placeholder slots in the arena at those IDs.
    for es in ec.entity_sets {
        let id = match disc.entity_sets.get(&(qualified_name.clone(), es.name.clone())) {
            Some(id) => *id,
            None => continue,
        };
        let context = format!("{context_container}.EntitySet {}", es.name);
        let entity_type = match resolve_type_ref(
            &es.entity_type,
            &model.type_by_qname,
            &model.aliases,
            untyped,
            &context,
            errors,
        ) {
            TypeRef::Named(NamedTypeId::Entity(eid)) => eid,
            _ => {
                errors.push(ResolutionError {
                    at: format!("{context}: EntityType {} must be an EntityType", es.entity_type),
                    kind: ResolutionErrorKind::InvalidTypeReference(es.entity_type.clone()),
                });
                dummy_entity_type_id()
            }
        };
        // NavigationPropertyBindings depend on this container's children
        // being discoverable — they are, because we put them all into
        // `disc.entity_sets` and `disc.singletons` in phase 2.
        let bindings = es
            .navigation_property_bindings
            .into_iter()
            .filter_map(|b| {
                build_navigation_property_binding(
                    b,
                    &qualified_name,
                    disc,
                    &context,
                    errors,
                )
            })
            .collect();

        let slot = &mut model.entity_sets[id.index()];
        *slot = EntitySet {
            container: cid,
            name: es.name,
            entity_type,
            include_in_service_document: es.include_in_service_document,
            navigation_property_bindings: bindings,
            annotations: es.annotations,
        };
        entity_set_ids.push(id);
    }

    for s in ec.singletons {
        let id = match disc.singletons.get(&(qualified_name.clone(), s.name.clone())) {
            Some(id) => *id,
            None => continue,
        };
        let context = format!("{context_container}.Singleton {}", s.name);
        let type_ = resolve_type_ref(
            &s.type_,
            &model.type_by_qname,
            &model.aliases,
            untyped,
            &context,
            errors,
        );
        let bindings = s
            .navigation_property_bindings
            .into_iter()
            .filter_map(|b| {
                build_navigation_property_binding(
                    b,
                    &qualified_name,
                    disc,
                    &context,
                    errors,
                )
            })
            .collect();

        let slot = &mut model.singletons[id.index()];
        *slot = Singleton {
            container: cid,
            name: s.name,
            type_,
            navigation_property_bindings: bindings,
            annotations: s.annotations,
        };
        singleton_ids.push(id);
    }

    let container_struct = EntityContainer {
        qualified_name,
        extends,
        entity_sets: entity_set_ids,
        singletons: singleton_ids,
        action_imports: Vec::new(),    // Action/Function not in syntactic yet
        function_imports: Vec::new(),
        annotations: ec.annotations,
    };
    model.entity_containers.push(container_struct);
}

fn build_navigation_property_binding(
    b: syntactic::NavigationPropertyBinding,
    current_container: &QualifiedName,
    disc: &Discovery,
    parent_context: &str,
    errors: &mut Vec<ResolutionError>,
) -> Option<NavigationPropertyBinding> {
    // Target syntax (simplified handling):
    //   - "Name"                       → child of current container
    //   - "Namespace.Container/Name"   → child of named container
    let target_str = &b.target;
    let target_ref = if let Some((container_part, child_part)) = target_str.split_once('/') {
        // "X/Y" form
        let qname: Result<QualifiedName, _> = container_part.parse();
        let qname = match qname {
            Ok(q) => q,
            Err(_) => {
                errors.push(ResolutionError {
                    at: format!("{parent_context}: NavigationPropertyBinding Target {target_str}"),
                    kind: ResolutionErrorKind::InvalidTypeReference(target_str.clone()),
                });
                return None;
            }
        };
        match disc.entity_sets.get(&(qname.clone(), child_part.to_string())) {
            Some(id) => NamedElementRef::EntitySet(*id),
            None => match disc.singletons.get(&(qname.clone(), child_part.to_string())) {
                Some(id) => NamedElementRef::Singleton(*id),
                None => {
                    errors.push(ResolutionError {
                        at: format!("{parent_context}: NavigationPropertyBinding Target {target_str}"),
                        kind: ResolutionErrorKind::UnknownContainerChild {
                            container: qname,
                            child: child_part.to_string(),
                        },
                    });
                    return None;
                }
            },
        }
    } else {
        // Bare name — child of current container.
        match disc.entity_sets.get(&(current_container.clone(), target_str.clone())) {
            Some(id) => NamedElementRef::EntitySet(*id),
            None => match disc.singletons.get(&(current_container.clone(), target_str.clone())) {
                Some(id) => NamedElementRef::Singleton(*id),
                None => {
                    errors.push(ResolutionError {
                        at: format!(
                            "{parent_context}: NavigationPropertyBinding Target {target_str}"
                        ),
                        kind: ResolutionErrorKind::UnknownContainerChild {
                            container: current_container.clone(),
                            child: target_str.clone(),
                        },
                    });
                    return None;
                }
            },
        }
    };

    Some(NavigationPropertyBinding {
        path: b.path,
        target: target_ref,
    })
}

// ============================================================================
// Type-reference parsing + resolution
// ============================================================================

enum ParsedTypeRef {
    Named(QualifiedName),
    Collection(Box<ParsedTypeRef>),
}

fn parse_type_ref(s: &str) -> Option<ParsedTypeRef> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("Collection(") {
        let inner = rest.strip_suffix(')')?;
        return parse_type_ref(inner).map(|p| ParsedTypeRef::Collection(Box::new(p)));
    }
    s.parse::<QualifiedName>().ok().map(ParsedTypeRef::Named)
}

fn resolve_type_ref(
    raw: &str,
    type_lookup: &HashMap<QualifiedName, NamedTypeId>,
    aliases: &HashMap<String, String>,
    untyped: NamedTypeId,
    context: &str,
    errors: &mut Vec<ResolutionError>,
) -> TypeRef {
    let parsed = match parse_type_ref(raw) {
        Some(p) => p,
        None => {
            errors.push(ResolutionError {
                at: format!("{context}: {raw}"),
                kind: ResolutionErrorKind::InvalidTypeReference(raw.to_string()),
            });
            return TypeRef::Named(untyped);
        }
    };
    resolve_parsed_type_ref(&parsed, type_lookup, aliases, untyped, context, errors)
}

fn resolve_parsed_type_ref(
    p: &ParsedTypeRef,
    type_lookup: &HashMap<QualifiedName, NamedTypeId>,
    aliases: &HashMap<String, String>,
    untyped: NamedTypeId,
    context: &str,
    errors: &mut Vec<ResolutionError>,
) -> TypeRef {
    match p {
        ParsedTypeRef::Collection(inner) => TypeRef::Collection(Box::new(resolve_parsed_type_ref(
            inner, type_lookup, aliases, untyped, context, errors,
        ))),
        ParsedTypeRef::Named(qname) => {
            let canonical = canonicalize_qname(qname, aliases);
            match type_lookup.get(&canonical) {
                Some(id) => TypeRef::Named(*id),
                None => {
                    errors.push(ResolutionError {
                        at: format!("{context}: {qname}"),
                        kind: ResolutionErrorKind::UnknownType(canonical),
                    });
                    TypeRef::Named(untyped)
                }
            }
        }
    }
}

fn resolve_named_qualified(
    raw: &str,
    aliases: &HashMap<String, String>,
) -> Option<QualifiedName> {
    let q: Result<QualifiedName, _> = raw.parse();
    q.ok().map(|q| canonicalize_qname(&q, aliases))
}

fn canonicalize_qname(q: &QualifiedName, aliases: &HashMap<String, String>) -> QualifiedName {
    match aliases.get(&q.namespace) {
        Some(ns) => QualifiedName {
            namespace: ns.clone(),
            name: q.name.clone(),
        },
        None => q.clone(),
    }
}

// ============================================================================
// resolve_path support
// ============================================================================

fn resolve_base(model: &EdmModel, base: &NamedRef) -> Result<NamedElementRef, ResolutionError> {
    if base.overload.is_some() {
        return Err(ResolutionError {
            at: format!("{}", base.qname),
            kind: ResolutionErrorKind::OverloadResolutionUnsupported,
        });
    }
    let canonical = canonicalize_qname(&base.qname, &model.aliases);
    if let Some(named_type) = model.type_by_qname.get(&canonical).copied() {
        return Ok(named_type_to_ref(named_type));
    }
    if let Some(id) = model.containers_by_qname.get(&canonical).copied() {
        return Ok(NamedElementRef::EntityContainer(id));
    }
    if let Some(id) = model.terms_by_qname.get(&canonical).copied() {
        return Ok(NamedElementRef::Term(id));
    }
    Err(ResolutionError {
        at: base.qname.to_string(),
        kind: ResolutionErrorKind::UnknownType(canonical),
    })
}

fn named_type_to_ref(nt: NamedTypeId) -> NamedElementRef {
    match nt {
        NamedTypeId::Primitive(id) => NamedElementRef::Primitive(id),
        NamedTypeId::Entity(id) => NamedElementRef::EntityType(id),
        NamedTypeId::Complex(id) => NamedElementRef::ComplexType(id),
        NamedTypeId::Enum(id) => NamedElementRef::EnumType(id),
        NamedTypeId::TypeDef(id) => NamedElementRef::TypeDef(id),
    }
}

fn walk_segment(model: &EdmModel, current: &NamedElementRef, segment: &str) -> Option<NamedElementRef> {
    match current {
        NamedElementRef::EntityType(et_id) => {
            let et = model.entity_type(*et_id);
            for (i, p) in et.properties.iter().enumerate() {
                if p.name == segment {
                    return Some(NamedElementRef::Property {
                        owner: StructuralTypeId::Entity(*et_id),
                        index: i as u32,
                    });
                }
            }
            for (i, np) in et.navigation_properties.iter().enumerate() {
                if np.name == segment {
                    return Some(NamedElementRef::NavigationProperty {
                        owner: StructuralTypeId::Entity(*et_id),
                        index: i as u32,
                    });
                }
            }
            None
        }
        NamedElementRef::ComplexType(ct_id) => {
            let ct = model.complex_type(*ct_id);
            for (i, p) in ct.properties.iter().enumerate() {
                if p.name == segment {
                    return Some(NamedElementRef::Property {
                        owner: StructuralTypeId::Complex(*ct_id),
                        index: i as u32,
                    });
                }
            }
            for (i, np) in ct.navigation_properties.iter().enumerate() {
                if np.name == segment {
                    return Some(NamedElementRef::NavigationProperty {
                        owner: StructuralTypeId::Complex(*ct_id),
                        index: i as u32,
                    });
                }
            }
            None
        }
        NamedElementRef::EnumType(et_id) => {
            let et = model.enum_type(*et_id);
            for (i, m) in et.members.iter().enumerate() {
                if m.name == segment {
                    return Some(NamedElementRef::EnumMember {
                        owner: *et_id,
                        index: i as u32,
                    });
                }
            }
            None
        }
        NamedElementRef::EntityContainer(c_id) => {
            let c = model.entity_container(*c_id);
            for &es_id in &c.entity_sets {
                if model.entity_set(es_id).name == segment {
                    return Some(NamedElementRef::EntitySet(es_id));
                }
            }
            for &s_id in &c.singletons {
                if model.singleton(s_id).name == segment {
                    return Some(NamedElementRef::Singleton(s_id));
                }
            }
            None
        }
        _ => None,
    }
}

fn format_path(path: &TargetPath) -> String {
    let mut s = path.base.qname.to_string();
    for seg in &path.segments {
        s.push('/');
        s.push_str(seg);
    }
    if let Some(a) = &path.annotation {
        s.push('@');
        s.push_str(&a.term.to_string());
        if let Some(q) = &a.qualifier {
            s.push('#');
            s.push_str(q);
        }
    }
    s
}

// ============================================================================
// Suppress unused-item warnings for items present only for completeness
// ============================================================================

#[allow(dead_code)]
fn _unused_types_to_keep_imports_alive() {
    let _: Option<Action> = None;
    let _: Option<ActionImport> = None;
    let _: Option<Function> = None;
    let _: Option<FunctionImport> = None;
    let _: Option<CallableId> = None;
    let _: Option<Term> = None;
}
