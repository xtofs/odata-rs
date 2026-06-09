# CSDL 4.01 Meta-Model

This document defines the structural meta-model for CSDL files.

Purpose:

- define which model elements exist
- define legal parent/child relationships
- classify attributes as value attributes vs symbolic reference attributes
- define allowed reference targets for each symbolic reference attribute

Non-goals:

- implementation status tracking
- annotation expression AST and expression serialization rules (separate domain)

References:

- https://docs.oasis-open.org/odata/odata-csdl-xml/v4.01/odata-csdl-xml-v4.01.html
- https://docs.oasis-open.org/odata/odata-csdl-json/v4.01/odata-csdl-json-v4.01.html

## Contents

- [Conventions](#conventions)
- [Element Definitions (4.01)](#element-definitions-401)
  - [Document Level](#1-document-level)
  - [Schema Level](#2-schema-level)
  - [Type Elements](#3-type-elements)
  - [Operation Elements](#4-operation-elements)
  - [Entity Container Elements](#5-entity-container-elements)
  - [Constraint and Binding Elements](#6-constraint-and-binding-elements)
  - [Annotation Elements](#7-annotation-elements)
- [Resolution Rules](#resolution-rules)

## Conventions

Element and attribute identifiers in this document use the CSDL XML names and casing.

For example: EntityType, NavigationProperty, BaseType, EntitySetPath.

Attribute classes:

- value: scalar/structured data with no symbolic linking
- reference: symbolic name/path requiring resolution

Attribute entry format in this document:

- value attribute: Name: required|optional value <type>
- reference attribute: Name: required|optional reference <target>

Path semantics notes:

- Some reference/value attributes carry path syntax rather than a simple name.
- Unless stated otherwise, path-valued attributes are relative to their local model context.

Cardinality notation:

- 1 exactly one
- 0..1 optional single
- 0..\* zero or more
- 1..\* one or more

Reference target kinds:

- Edm.PrimitiveType: Edm primitive domain (for example Edm.String), not a CSDL element node here

## Element Definitions (4.01)

## 1. Document Level

<a id="element-edmx"></a>

### Element: Edmx

- Parent: none (document root)
- Children:
  - [Reference](#element-reference) (0..\*)
  - [DataServices](#element-dataservices) (1)
- Attributes:
  - Version: required value String

<a id="element-reference"></a>

### Element: Reference

- Parent: [Edmx](#element-edmx)
- Children:
  - [Include](#element-include) (1..\*)
  - [IncludeAnnotations](#element-includeannotations) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Uri: required value String

<a id="element-include"></a>

### Element: Include

- Parent: [Reference](#element-reference)
- Children: none
- Attributes:
  - Namespace: required value String
  - Alias: optional value String

<a id="element-includeannotations"></a>

### Element: IncludeAnnotations

- Parent: [Reference](#element-reference)
- Children: none
- Attributes:
  - TermNamespace: required value String
  - TargetNamespace: optional value String
  - Qualifier: optional value String

<a id="element-dataservices"></a>

### Element: DataServices

- Parent: [Edmx](#element-edmx)
- Children:
  - [Schema](#element-schema) (1..\*)
- Attributes: none

## 2. Schema Level

<a id="element-schema"></a>

### Element: Schema

- Parent: [DataServices](#element-dataservices)
- Children:
  - [EntityType](#element-entitytype) (0..\*)
  - [ComplexType](#element-complextype) (0..\*)
  - [EnumType](#element-enumtype) (0..\*)
  - [TypeDefinition](#element-typedefinition) (0..\*)
  - [Action](#element-action) (0..\*)
  - [Function](#element-function) (0..\*)
  - [Term](#element-term) (0..\*)
  - [EntityContainer](#element-entitycontainer) (0..1)
  - [Annotations](#element-annotations) (0..\*)
- Attributes:
  - Namespace: required value String
  - Alias: optional value String

## 3. Type Elements

<a id="element-entitytype"></a>

### Element: EntityType

- Parent: [Schema](#element-schema)
- Children:
  - [Key](#element-key) (0..1)
  - [Property](#element-property) (0..\*)
  - [NavigationProperty](#element-navigationproperty) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - BaseType: optional reference [EntityType](#element-entitytype)
  - Abstract: optional value bool
  - OpenType: optional value bool
  - HasStream: optional value bool

<a id="element-complextype"></a>

### Element: ComplexType

- Parent: [Schema](#element-schema)
- Children:
  - [Property](#element-property) (0..\*)
  - [NavigationProperty](#element-navigationproperty) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - BaseType: optional reference [ComplexType](#element-complextype)
  - Abstract: optional value bool
  - OpenType: optional value bool

<a id="element-enumtype"></a>

### Element: EnumType

- Parent: [Schema](#element-schema)
- Children:
  - [Member](#element-member) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - UnderlyingType: optional reference Edm.PrimitiveType
  - IsFlags: optional value bool

<a id="element-member"></a>

### Element: Member

- Parent: [EnumType](#element-enumtype)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - Value: optional value Int64

<a id="element-typedefinition"></a>

### Element: TypeDefinition

- Parent: [Schema](#element-schema)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - UnderlyingType: required reference Edm.PrimitiveType
  - MaxLength: optional value Int|`max`
  - Precision: optional value Int
  - Scale: optional value Int|`variable`
  - SRID: optional value Int|String
  - Unicode: optional value bool

<a id="element-term"></a>

### Element: Term

- Parent: [Schema](#element-schema)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - Type: required reference Edm.PrimitiveType | [TypeDefinition](#element-typedefinition) | [EnumType](#element-enumtype) | [ComplexType](#element-complextype) | [EntityType](#element-entitytype)
  - BaseTerm: optional reference [Term](#element-term)
  - DefaultValue: optional value String
  - AppliesTo: optional value String
  - Nullable: optional value bool
  - MaxLength: optional value Int|`max`
  - Precision: optional value Int
  - Scale: optional value Int|`variable`
  - SRID: optional value Int|String
  - Unicode: optional value bool

<a id="element-property"></a>

### Element: Property

- Parent: [EntityType](#element-entitytype) | [ComplexType](#element-complextype)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - Type: required reference Edm.PrimitiveType | [TypeDefinition](#element-typedefinition) | [EnumType](#element-enumtype) | [ComplexType](#element-complextype)
  - Nullable: optional value bool
  - MaxLength: optional value Int|`max`
  - Precision: optional value Int
  - Scale: optional value Int|`variable`
  - SRID: optional value Int|String
  - Unicode: optional value bool
  - DefaultValue: optional value String

<a id="element-navigationproperty"></a>

### Element: NavigationProperty

- Parent: [EntityType](#element-entitytype) | [ComplexType](#element-complextype)
- Children:
  - [ReferentialConstraint](#element-referentialconstraint) (0..\*)
  - [OnDelete](#element-ondelete) (0..1)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - Type: required reference [EntityType](#element-entitytype)
  - Nullable: optional value bool
  - Partner: optional reference [NavigationProperty](#element-navigationproperty)
  - ContainsTarget: optional value bool

## 4. Operation Elements

<a id="element-action"></a>

### Element: Action

- Parent: [Schema](#element-schema)
- Children:
  - [Parameter](#element-parameter) (0..\*)
  - [ReturnType](#element-returntype) (0..1)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - IsBound: optional value bool
  - EntitySetPath: optional value String (path expression, relative to the binding parameter)

<a id="element-function"></a>

### Element: Function

- Parent: [Schema](#element-schema)
- Children:
  - [Parameter](#element-parameter) (0..\*)
  - [ReturnType](#element-returntype) (1)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - IsBound: optional value bool
  - IsComposable: optional value bool
  - EntitySetPath: optional value String (path expression, relative to the binding parameter)

<a id="element-parameter"></a>

### Element: Parameter

- Parent: [Action](#element-action) | [Function](#element-function)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - Type: required reference Edm.PrimitiveType | [TypeDefinition](#element-typedefinition) | [EnumType](#element-enumtype) | [ComplexType](#element-complextype) | [EntityType](#element-entitytype)
  - Nullable: optional value bool
  - MaxLength: optional value Int|`max`
  - Precision: optional value Int
  - Scale: optional value Int|`variable`
  - SRID: optional value Int|String
  - Unicode: optional value bool
  - DefaultValue: optional value String

<a id="element-returntype"></a>

### Element: ReturnType

- Parent: [Action](#element-action) | [Function](#element-function)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Type: required reference Edm.PrimitiveType | [TypeDefinition](#element-typedefinition) | [EnumType](#element-enumtype) | [ComplexType](#element-complextype) | [EntityType](#element-entitytype)
  - Nullable: optional value bool
  - MaxLength: optional value Int|`max`
  - Precision: optional value Int
  - Scale: optional value Int|`variable`
  - SRID: optional value Int|String
  - Unicode: optional value bool

## 5. Entity Container Elements

<a id="element-entitycontainer"></a>

### Element: EntityContainer

- Parent: [Schema](#element-schema)
- Children:
  - [EntitySet](#element-entityset) (0..\*)
  - [Singleton](#element-singleton) (0..\*)
  - [ActionImport](#element-actionimport) (0..\*)
  - [FunctionImport](#element-functionimport) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - Extends: optional reference [EntityContainer](#element-entitycontainer)

<a id="element-entityset"></a>

### Element: EntitySet

- Parent: [EntityContainer](#element-entitycontainer)
- Children:
  - [NavigationPropertyBinding](#element-navigationpropertybinding) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - EntityType: required reference [EntityType](#element-entitytype)
  - IncludeInServiceDocument: optional value bool

<a id="element-singleton"></a>

### Element: Singleton

- Parent: [EntityContainer](#element-entitycontainer)
- Children:
  - [NavigationPropertyBinding](#element-navigationpropertybinding) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - Type: required reference [EntityType](#element-entitytype)
  - IncludeInServiceDocument: optional value bool

<a id="element-actionimport"></a>

### Element: ActionImport

- Parent: [EntityContainer](#element-entitycontainer)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - Action: required reference [Action](#element-action)
  - EntitySet: optional reference [EntitySet](#element-entityset)
  - IncludeInServiceDocument: optional value bool

<a id="element-functionimport"></a>

### Element: FunctionImport

- Parent: [EntityContainer](#element-entitycontainer)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: required value String
  - Function: required reference [Function](#element-function)
  - EntitySet: optional reference [EntitySet](#element-entityset)
  - IncludeInServiceDocument: optional value bool

## 6. Constraint and Binding Elements

<a id="element-key"></a>

### Element: Key

- Parent: [EntityType](#element-entitytype)
- Children:
  - [PropertyRef](#element-propertyref) (1..\*)
- Attributes: none

<a id="element-propertyref"></a>

### Element: PropertyRef

- Parent: [Key](#element-key)
- Children: none
- Attributes:
  - Name: required reference [Property](#element-property)
  - Alias: optional value String

<a id="element-referentialconstraint"></a>

### Element: ReferentialConstraint

- Parent: [NavigationProperty](#element-navigationproperty)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Property: required reference [Property](#element-property)
  - ReferencedProperty: required reference [Property](#element-property)

<a id="element-ondelete"></a>

### Element: OnDelete

- Parent: [NavigationProperty](#element-navigationproperty)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Action: required value enum Cascade | None | SetNull | SetDefault

<a id="element-navigationpropertybinding"></a>

### Element: NavigationPropertyBinding

- Parent: [EntitySet](#element-entityset) | [Singleton](#element-singleton)
- Children: none
- Attributes:
  - Path: required reference navigation path [NavigationProperty](#element-navigationproperty) (relative to the binding source)
  - Target: required reference target path [EntitySet](#element-entityset) | [Singleton](#element-singleton) (relative to the containing entity container)

## 7. Annotation Elements

<a id="element-annotations"></a>

### Element: Annotations

- Parent: [Schema](#element-schema)
- Children:
  - [Annotation](#element-annotation) (1..\*)
- Attributes:
  - Target: required reference model element path (model-level path)
  - Qualifier: optional value String

<a id="element-annotation"></a>

### Element: Annotation

- Parent: any annotatable element and [Annotations](#element-annotations)
- Children:
  - nested [Annotation](#element-annotation) (0..\*)
  - expression payload. Not an Element in the sense used here. Out of scope in this document
- Attributes:
  - Term: required reference [Term](#element-term)
  - Qualifier: optional value String
  - Path: optional reference expression path domain (context-relative)

## Resolution Rules

These rules describe symbolic-reference intent in the meta-model, not current
implementation completeness.

- Name forms may appear as namespace-qualified, alias-qualified, or local
  names.
- Type references are resolved within schema namespace plus document reference
  alias mappings.
- Collection wrappers are part of type syntax; readers may normalize
  collection-ness into explicit fields in the in-memory model.
- Sibling-target annotation forms are legal and represented as annotation
  metadata, independent of expression payload semantics.

## Attribute Type Notes

This section records value-shape detail for commonly overloaded attributes.

- OnDelete.Action:
  enum literal (case-sensitive): Cascade | None | SetNull | SetDefault.
- MaxLength:
  discriminated union: positive integer length OR token `max` (lowercase).
- Scale:
  discriminated union: non-negative integer scale OR token `variable` (lowercase).
- AppliesTo:
  list of target-kind identifiers (not a free-form string in the logical model);
  parsers may carry the wire representation as strings, but model-level code
  should treat these as a closed target-kind set.
