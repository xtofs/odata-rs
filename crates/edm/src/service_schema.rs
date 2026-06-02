#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schema {
    pub namespace: String,
    entity_types: Vec<EntityType>,
    entity_sets: Vec<EntitySet>,
}

impl Schema {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self { namespace: namespace.into(), entity_types: Vec::new(), entity_sets: Vec::new() }
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntityType {
    pub name: String,
    pub navigation_properties: Vec<NavigationProperty>,
}

impl EntityType {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), navigation_properties: Vec::new() }
    }

    pub fn with_nav_prop(mut self, nav: NavigationProperty) -> Self {
        self.navigation_properties.push(nav);
        self
    }

    pub fn contained_nav_props(&self) -> impl Iterator<Item = &NavigationProperty> {
        self.navigation_properties.iter().filter(|n| n.contains_target)
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
        Self { name: name.into(), target_type: target_type.into(), contains_target: false }
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
        Self { name: name.into(), entity_type_name: entity_type_name.into() }
    }
}