# Semantic Validation Checklist

This checklist is now split into:

- rules already implemented in `src/validator.rs`
- remaining semantic backlog

## Implemented now

- [x] No duplicate key properties.
- [x] Key properties must exist on the declaring entity.
- [x] Key properties cannot be complex-valued.
- [x] No duplicate schema child names.
- [x] No duplicate entity child names (property/navigation combined).
- [x] No duplicate complex child names (property/navigation combined).
- [x] No duplicate enum member names.
- [x] No duplicate entity container child names (entity set/singleton).

## Remaining backlog

### A. Entity type semantics

- [ ] Non-abstract entity types define a key.
- [ ] Key properties are non-nullable.
- [ ] Key identity is consistent across inheritance.

### B. Entity type inheritance semantics

- [ ] Derived types do not declare a key.
- [ ] Derived types do not add key properties.
- [ ] Derived types do not override inherited properties.
- [ ] Derived types do not override inherited navigation properties.
- [ ] Derived types do not change type/nullability of inherited properties.
- [ ] Inheritance hierarchy contains no cycles.
- [ ] Base types are entity types.

### C. Structural property semantics

- [ ] No conflicts with inherited property/navigation names.
- [ ] Default values are type-compatible.
- [ ] Facets (`Precision`, `Scale`, `MaxLength`) are type-valid.

### D. Complex type semantics

- [ ] Complex type inheritance contains no cycles.
- [ ] Derived complex types do not redefine inherited properties.

### E. Navigation property semantics

- [ ] Multiplicity is semantically valid for target entity.
- [ ] If a Partner navigation property exists it points back correctly.
- [ ] Containment navigation properties are used in valid container contexts.

### F. Referential constraint semantics

- [ ] Principal key properties are non-nullable.
- [ ] Dependent properties exist and match principal key types.
- [ ] Constraint cardinalities match.
- [ ] Referential constraints do not form cycles.

### G. Entity container semantics

- [ ] NavigationPropertyBinding.Path resolves from the enclosing entity set/singleton declared entity type.
- [ ] NavigationPropertyBinding.Path segments are restricted to: type casts, complex properties, and containment navigation properties, followed by a final non-containment navigation property.
- [ ] NavigationPropertyBinding.Path MUST NOT contain non-containment navigation properties before the final segment.
- [ ] NavigationPropertyBinding.Path final segment MUST identify a navigation property (or a 4.01 terminal type-cast variant, when supported).
- [ ] NavigationPropertyBinding.Path type-cast segments are valid and in-scope, and cast position constraints are enforced.
- [ ] NavigationPropertyBinding.Path complex-property traversal is valid for the current structured type at each segment.
- [ ] NavigationPropertyBinding.Path containment traversal is valid, including collection-valued containment semantics (binding applies to all items).
- [ ] NavigationPropertyBinding.Path recursive sub-paths are accepted and interpreted recursively (positive-cycle semantics).
- [ ] No duplicate NavigationPropertyBinding.Path values within the same binding source, with "most specific path wins" behavior for type-cast-specialized paths.
- [ ] NavigationPropertyBinding.Target simple identifier resolves to an entity set/singleton in the same entity container.
- [ ] NavigationPropertyBinding.Target target path resolves to an in-scope entity set, singleton, or direct/indirect containment navigation property of a singleton.
- [ ] NavigationPropertyBinding.Target target path traversal before the final containment segment is restricted to single-valued complex properties and single-valued containment navigation properties.
- [ ] NavigationPropertyBinding.Target target path MUST NOT contain non-containment navigation properties before the final segment.
- [ ] NavigationPropertyBinding.Target terminal segment kind is validated against allowed target kinds (entity set, singleton, containment nav property).

### H. Function semantics

- [ ] Overloads differ by parameter signature.
- [ ] Bound functions declare a valid binding parameter.
- [ ] Parameter names are unique.

### I. Action semantics

- [ ] Overloads differ by parameter signature.
- [ ] Bound actions declare a valid binding parameter.
- [ ] Parameter names are unique.

================================================
