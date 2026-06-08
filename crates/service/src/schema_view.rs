//! Internal projection of `csdl_edm::edm::Model` to exactly what the router
//! needs.
//!
//! This module is **crate-private on purpose**. It is the router's working
//! set — not a public re-projection of the EDM model. Anything that wants
//! richer schema information should reach for `csdl_edm::edm::Model` directly.
//!
//! What's kept here:
//!
//! - The set of entity sets, keyed by name, each with the short name of its
//!   target entity type. That's the URL prefix `/EntitySet[/{id}]`.
//! - The set of entity types referenced by those entity sets, each with the
//!   *contained* navigation properties on it. Contained navs are what
//!   produce `/EntitySet/{id}/NavProp[/{nav_id}]` routes.
//!
//! What's intentionally **not** kept: properties, facets, base types, key
//! lists, non-contained navs, complex types, enums, type definitions, terms,
//! functions, actions, annotations. All of that is already in `edm::Model`;
//! the router doesn't use it.
//!
//! Construction goes through [`Schema::from_model`]. Because the semantic
//! graph is already resolved (Arcs for direct references, short names),
//! there is **no string splitting** and no risk of dangling references.

use csdl_edm::edm::{EntityContainerElement, Model};

/// Router-facing view of a single schema. Crate-private.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Schema {
    pub namespace: String,
    entity_types: Vec<EntityType>,
    entity_sets: Vec<EntitySet>,
}

impl Schema {
    /// Build the router view from a resolved EDM model. Walks the entity
    /// container's `EntitySet` elements, follows the `Arc<EntityType>` target
    /// of each, and keeps only contained navigation properties.
    pub(crate) fn from_model(model: &Model) -> Self {
        let mut schema = Schema {
            namespace: model.namespace.clone(),
            entity_types: Vec::new(),
            entity_sets: Vec::new(),
        };

        let Some(container) = &model.entity_container else {
            return schema;
        };

        // Collect (entity_set_name, target_entity_type) pairs, then derive the
        // entity-type view from the set of distinct targets.
        let mut seen_type_names: Vec<String> = Vec::new();
        for element in &container.elements {
            let EntityContainerElement::EntitySet(es) = element.as_ref() else {
                continue;
            };

            schema.entity_sets.push(EntitySet {
                name: es.name.clone(),
                entity_type_name: es.target.name.clone(),
            });

            if seen_type_names.contains(&es.target.name) {
                continue;
            }
            seen_type_names.push(es.target.name.clone());

            let mut contained = Vec::new();
            for nav in es.target.navigation_properties() {
                if nav.contains_target != Some(true) {
                    continue;
                }
                let target_name = nav
                    .target
                    .upgrade()
                    .map(|et| et.name.clone())
                    .unwrap_or_default();
                contained.push(NavigationProperty {
                    name: nav.name.clone(),
                    target_type: target_name,
                });
            }

            schema.entity_types.push(EntityType {
                name: es.target.name.clone(),
                contained_navigation_properties: contained,
            });
        }

        schema
    }

    pub(crate) fn entity_set(&self, name: &str) -> Option<&EntitySet> {
        self.entity_sets.iter().find(|es| es.name == name)
    }

    pub(crate) fn entity_sets(&self) -> impl Iterator<Item = &EntitySet> {
        self.entity_sets.iter()
    }

    pub(crate) fn entity_type(&self, name: &str) -> Option<&EntityType> {
        self.entity_types.iter().find(|et| et.name == name)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EntityType {
    pub name: String,
    /// Only contained nav props — the rest are not routable.
    pub contained_navigation_properties: Vec<NavigationProperty>,
}

impl EntityType {
    pub(crate) fn contained_nav_props(&self) -> impl Iterator<Item = &NavigationProperty> {
        self.contained_navigation_properties.iter()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NavigationProperty {
    pub name: String,
    /// Short name of the target entity type (resolved via the Arc graph;
    /// no string splitting).
    pub target_type: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EntitySet {
    pub name: String,
    pub entity_type_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use csdl_edm::parser::from_xml_reader;
    use csdl_edm::resolver::Resolver;

    fn rooms_model() -> std::sync::Arc<Model> {
        const CSDL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
            <edmx:Edmx xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx" Version="4.0">
                <edmx:DataServices>
                    <Schema Namespace="Bm" xmlns="http://docs.oasis-open.org/odata/ns/edm">
                        <EntityType Name="Printer">
                            <Key><PropertyRef Name="Id" /></Key>
                            <Property Name="Id" Type="Edm.String" Nullable="false" />
                        </EntityType>
                        <EntityType Name="Room">
                            <Key><PropertyRef Name="Id" /></Key>
                            <Property Name="Id" Type="Edm.String" Nullable="false" />
                            <NavigationProperty Name="Printers" Type="Collection(Bm.Printer)" ContainsTarget="true" />
                        </EntityType>
                        <EntityContainer Name="Container">
                            <EntitySet Name="Rooms" EntityType="Bm.Room" />
                        </EntityContainer>
                    </Schema>
                </edmx:DataServices>
            </edmx:Edmx>"#;
        let document = from_xml_reader(CSDL.as_bytes()).expect("parse");
        let edmx = document.edmx.expect("edmx");
        let document_model = Resolver::resolve_document(edmx).expect("resolve");
        document_model.schemas.first().cloned().expect("schema")
    }

    #[test]
    fn projects_entity_sets_and_contained_nav_props() {
        let model = rooms_model();
        let schema = Schema::from_model(&model);

        let rooms = schema
            .entity_set("Rooms")
            .expect("Rooms entity set should exist");
        assert_eq!(rooms.entity_type_name, "Room");

        let room = schema
            .entity_type("Room")
            .expect("Room entity type should exist");
        let contained: Vec<&str> = room
            .contained_nav_props()
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(contained, vec!["Printers"]);
        // Target is short-named because it came from the Arc graph.
        let printers_nav = room.contained_nav_props().next().unwrap();
        assert_eq!(printers_nav.target_type, "Printer");
    }
}
