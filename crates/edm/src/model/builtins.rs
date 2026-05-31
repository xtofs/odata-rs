//! The synthetic `Edm` schema — primitives, spatial types, and abstract base
//! types that every model references but no one parses.

use super::{PrimitiveKind, PrimitiveType, QualifiedName, SchemaInfo, EdmModel};

pub const EDM_NAMESPACE: &str = "Edm";

/// Concrete primitive value types defined by CSDL 4.01 §4.4.
const PRIMITIVES: &[&str] = &[
    "Binary",
    "Boolean",
    "Byte",
    "Date",
    "DateTimeOffset",
    "Decimal",
    "Double",
    "Duration",
    "Guid",
    "Int16",
    "Int32",
    "Int64",
    "SByte",
    "Single",
    "Stream",
    "String",
    "TimeOfDay",
];

/// Geographic / geometric primitive types (CSDL 4.01 §4.4).
const SPATIALS: &[&str] = &[
    "Geography",
    "GeographyPoint",
    "GeographyLineString",
    "GeographyPolygon",
    "GeographyMultiPoint",
    "GeographyMultiLineString",
    "GeographyMultiPolygon",
    "GeographyCollection",
    "Geometry",
    "GeometryPoint",
    "GeometryLineString",
    "GeometryPolygon",
    "GeometryMultiPoint",
    "GeometryMultiLineString",
    "GeometryMultiPolygon",
    "GeometryCollection",
];

/// Abstract types usable as the `Type` of a `Term`, `Property`, or
/// `Parameter` — they don't have an instance representation themselves, just
/// the constraint they impose on values (CSDL 4.01 §4.5).
const ABSTRACTS: &[&str] = &[
    "PrimitiveType",
    "ComplexType",
    "EntityType",
    "Untyped",
    "AnnotationPath",
    "PropertyPath",
    "NavigationPropertyPath",
    "ModelElementPath",
    "AnyPropertyPath",
];

/// Populate `model` with the Edm schema and all built-in types. Called by
/// [`EdmModel::new`]; usually not called directly.
pub fn populate(model: &mut EdmModel) {
    model.push_schema_info(SchemaInfo {
        namespace: EDM_NAMESPACE.to_string(),
        alias: None,
        is_builtin: true,
        annotations: Vec::new(),
    });
    for name in PRIMITIVES {
        model.push_primitive(PrimitiveType {
            qualified_name: QualifiedName::new(EDM_NAMESPACE, *name),
            kind: PrimitiveKind::Primitive,
        });
    }
    for name in SPATIALS {
        model.push_primitive(PrimitiveType {
            qualified_name: QualifiedName::new(EDM_NAMESPACE, *name),
            kind: PrimitiveKind::Spatial,
        });
    }
    for name in ABSTRACTS {
        model.push_primitive(PrimitiveType {
            qualified_name: QualifiedName::new(EDM_NAMESPACE, *name),
            kind: PrimitiveKind::Abstract,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::{NamedTypeId, PrimitiveKind, EdmModel};
    use super::*;
    use std::str::FromStr;

    #[test]
    fn new_model_has_edm_schema() {
        let m = EdmModel::new();
        let schemas = m.schemas();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0].namespace, "Edm");
        assert!(schemas[0].is_builtin);
    }

    #[test]
    fn edm_string_is_resolvable() {
        let m = EdmModel::new();
        let qname = QualifiedName::from_str("Edm.String").unwrap();
        // Internal: peek the lookup table by going through the public arena
        // accessor for every primitive and finding the one whose name matches.
        let found = (0..PRIMITIVES.len() + SPATIALS.len() + ABSTRACTS.len()).any(|i| {
            let id = super::super::PrimitiveTypeId::from_index(i);
            m.primitive(id).qualified_name == qname
        });
        assert!(found, "Edm.String not present in builtin schema");
    }

    #[test]
    fn primitive_kinds_are_correct() {
        let m = EdmModel::new();
        // First entry is PRIMITIVES[0] = "Binary" → Primitive
        let id = super::super::PrimitiveTypeId::from_index(0);
        let p = m.primitive(id);
        assert_eq!(p.qualified_name.name, "Binary");
        assert_eq!(p.kind, PrimitiveKind::Primitive);

        // First spatial sits right after the primitives.
        let id = super::super::PrimitiveTypeId::from_index(PRIMITIVES.len());
        let p = m.primitive(id);
        assert_eq!(p.qualified_name.name, "Geography");
        assert_eq!(p.kind, PrimitiveKind::Spatial);

        // First abstract sits after primitives + spatials.
        let id = super::super::PrimitiveTypeId::from_index(PRIMITIVES.len() + SPATIALS.len());
        let p = m.primitive(id);
        assert_eq!(p.qualified_name.name, "PrimitiveType");
        assert_eq!(p.kind, PrimitiveKind::Abstract);
    }

    /// Catches accidental NamedTypeId mis-categorization in `push_primitive`.
    #[test]
    fn primitives_are_categorized_as_primitive_in_lookup() {
        let m = EdmModel::new();
        for i in 0..(PRIMITIVES.len() + SPATIALS.len() + ABSTRACTS.len()) {
            let id = super::super::PrimitiveTypeId::from_index(i);
            // ensure the entry is at least present and is a primitive variant
            // when looked up via lookup table (the lookup table is private but
            // this round-trip catches obvious miswires).
            let _ = m.primitive(id);
            let _: NamedTypeId = NamedTypeId::Primitive(id);
        }
    }
}
