use crate::builder::build_model;
use crate::reader::CsdlReader;
use crate::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schema {
    pub namespace: String,
    entity_types: Vec<EntityType>,
    entity_sets: Vec<EntitySet>,
}

impl Schema {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            entity_types: Vec::new(),
            entity_sets: Vec::new(),
        }
    }

    pub fn add_entity_type(&mut self, entity_type: EntityType) {
        self.entity_types.push(entity_type);
    }

    pub fn add_entity_set(&mut self, entity_set: EntitySet) {
        self.entity_sets.push(entity_set);
    }

    pub fn entity_set(&self, name: &str) -> Option<&EntitySet> {
        self.entity_sets.iter().find(|es| es.name == name)
    }

    pub fn entity_sets(&self) -> impl Iterator<Item = &EntitySet> {
        self.entity_sets.iter()
    }

    pub fn entity_type(&self, name: &str) -> Option<&EntityType> {
        self.entity_types.iter().find(|et| et.name == name)
    }

    /// Parse CSDL XML and project the parts needed by the service layer
    /// (entity sets + contained navigation collections) into this schema type.
    pub fn from_csdl(csdl: &str) -> Result<Self> {
        let mut reader = CsdlReader::from_reader(csdl.as_bytes());
        let parsed = build_model(&mut reader)?;

        let namespace = parsed
            .schemas
            .first()
            .map(|s| s.namespace.clone())
            .unwrap_or_else(|| "Default".to_string());
        let mut schema = Schema::new(namespace);

        for s in parsed.schemas {
            for et in s.entity_types {
                let mut entity_type = EntityType::new(et.name);
                for nav in et
                    .navigation_properties
                    .into_iter()
                    .filter(|n| n.contains_target)
                {
                    entity_type = entity_type.with_nav_prop(
                        NavigationProperty::new(nav.name, short_type_name(&nav.type_)).contained(),
                    );
                }
                schema.add_entity_type(entity_type);
            }

            for container in s.entity_containers {
                for es in container.entity_sets {
                    schema
                        .add_entity_set(EntitySet::new(es.name, short_type_name(&es.entity_type)));
                }
            }
        }

        if schema.entity_sets.is_empty() {
            return Err(Error::Csdl(
                "no EntitySet definitions found in CSDL; cannot build service schema".to_string(),
            ));
        }

        Ok(schema)
    }
}

fn short_type_name(type_ref: &str) -> String {
    let trimmed = type_ref.trim();
    let without_collection = trimmed
        .strip_prefix("Collection(")
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(trimmed);

    without_collection
        .rsplit('.')
        .next()
        .unwrap_or(without_collection)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::Schema;

    const CSDL: &str = r#"
        <Schema Namespace="BuildingManagement" xmlns="http://docs.oasis-open.org/odata/ns/edm">
            <EntityType Name="Printer" />
            <EntityType Name="Room">
                <NavigationProperty Name="Printers" Type="Collection(BuildingManagement.Printer)" ContainsTarget="true" />
                <NavigationProperty Name="Owner" Type="BuildingManagement.User" />
            </EntityType>
            <EntityContainer Name="Container">
                <EntitySet Name="Rooms" EntityType="BuildingManagement.Room" />
            </EntityContainer>
        </Schema>
        "#;

    #[test]
    fn from_csdl_projects_entity_sets_and_contained_nav_props() {
        let schema = Schema::from_csdl(CSDL).expect("csdl should parse");

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
    }

    #[test]
    fn from_csdl_requires_entity_set_definitions() {
        let csdl = r#"
<Schema Namespace="BuildingManagement" xmlns="http://docs.oasis-open.org/odata/ns/edm">
    <EntityType Name="Room" />
</Schema>
"#;

        let err = Schema::from_csdl(csdl).expect_err("missing entity sets should error");
        assert!(err.to_string().contains("no EntitySet definitions"));
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntityType {
    pub name: String,
    pub navigation_properties: Vec<NavigationProperty>,
}

impl EntityType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            navigation_properties: Vec::new(),
        }
    }

    pub fn with_nav_prop(mut self, nav: NavigationProperty) -> Self {
        self.navigation_properties.push(nav);
        self
    }

    pub fn contained_nav_props(&self) -> impl Iterator<Item = &NavigationProperty> {
        self.navigation_properties
            .iter()
            .filter(|n| n.contains_target)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NavigationProperty {
    pub name: String,
    pub target_type: String,
    pub contains_target: bool,
}

impl NavigationProperty {
    pub fn new(name: impl Into<String>, target_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            target_type: target_type.into(),
            contains_target: false,
        }
    }

    pub fn contained(mut self) -> Self {
        self.contains_target = true;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntitySet {
    pub name: String,
    pub entity_type_name: String,
}

impl EntitySet {
    pub fn new(name: impl Into<String>, entity_type_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entity_type_name: entity_type_name.into(),
        }
    }
}
