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

- [ ] Navigation property bindings resolve to valid navigation paths.
- [ ] Navigation property bindings target valid entity sets/singletons.

### H. Function semantics

- [ ] Overloads differ by parameter signature.
- [ ] Bound functions declare a valid binding parameter.
- [ ] Parameter names are unique.

### I. Action semantics

- [ ] Overloads differ by parameter signature.
- [ ] Bound actions declare a valid binding parameter.
- [ ] Parameter names are unique.

================================================
