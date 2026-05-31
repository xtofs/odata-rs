
pub(super) struct ElementRule {
    pub name: &'static str,
    pub parents: ParentSpec,
}

pub(super) enum ParentSpec {
    /// No parent restriction. Used for elements that can appear in many
    /// places (e.g. `Annotation`) or whose parent may legitimately be a
    /// wrapper element the builder doesn't model (e.g. `Schema` under
    /// `edmx:DataServices`).
    Any,
    /// Element must appear inside one of these named parents.
    OneOf(&'static [&'static str]),
}


/// CSDL element meta-table: the structural constraints the builder enforces.
///
/// Single source of truth for "where can each element appear?". Extending
/// this table (required/optional attributes, annotation-acceptance) is
/// tracked in `TODO/element-metadata.md`.
pub(super) const RULES: &[ElementRule] = &[
    ElementRule { name: "Schema",                    parents: ParentSpec::Any },
    ElementRule { name: "EntityType",                parents: ParentSpec::OneOf(&["Schema"]) },
    ElementRule { name: "ComplexType",               parents: ParentSpec::OneOf(&["Schema"]) },
    ElementRule { name: "Key",                       parents: ParentSpec::OneOf(&["EntityType"]) },
    ElementRule { name: "PropertyRef",               parents: ParentSpec::OneOf(&["Key"]) },
    ElementRule { name: "Property",                  parents: ParentSpec::OneOf(&["EntityType", "ComplexType"]) },
    ElementRule { name: "NavigationProperty",        parents: ParentSpec::OneOf(&["EntityType", "ComplexType"]) },
    ElementRule { name: "ReferentialConstraint",     parents: ParentSpec::OneOf(&["NavigationProperty"]) },
    ElementRule { name: "OnDelete",                  parents: ParentSpec::OneOf(&["NavigationProperty"]) },
    ElementRule { name: "EnumType",                  parents: ParentSpec::OneOf(&["Schema"]) },
    ElementRule { name: "Member",                    parents: ParentSpec::OneOf(&["EnumType"]) },
    ElementRule { name: "TypeDefinition",            parents: ParentSpec::OneOf(&["Schema"]) },
    ElementRule { name: "EntityContainer",           parents: ParentSpec::OneOf(&["Schema"]) },
    ElementRule { name: "EntitySet",                 parents: ParentSpec::OneOf(&["EntityContainer"]) },
    ElementRule { name: "Singleton",                 parents: ParentSpec::OneOf(&["EntityContainer"]) },
    ElementRule { name: "NavigationPropertyBinding", parents: ParentSpec::OneOf(&["EntitySet", "Singleton"]) },
    ElementRule { name: "Annotation",                parents: ParentSpec::Any },
];

pub(super) fn rule(name: &str) -> Option<&'static ElementRule> {
    RULES.iter().find(|r| r.name == name)
}
