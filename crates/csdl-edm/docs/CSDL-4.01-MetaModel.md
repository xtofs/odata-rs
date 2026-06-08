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
  - Version: String (value, required)

<a id="element-reference"></a>

### Element: Reference

- Parent: [Edmx](#element-edmx)
- Children:
  - [Include](#element-include) (1..\*)
  - [IncludeAnnotations](#element-includeannotations) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Uri: String (value, required)

<a id="element-include"></a>

### Element: Include

- Parent: [Reference](#element-reference)
- Children: none
- Attributes:
  - Namespace: String (value, required)
  - Alias: String (value, optional)

<a id="element-includeannotations"></a>

### Element: IncludeAnnotations

- Parent: [Reference](#element-reference)
- Children: none
- Attributes:
  - TermNamespace: String (value, required)
  - TargetNamespace: String (value, optional)
  - Qualifier: String (value, optional)

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
  - Namespace: String (value, required)
  - Alias: String (value, optional)

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
  - Name: String (value, required)
  - BaseType: String (reference, optional) [EntityType](#element-entitytype)
  - Abstract: bool (value, optional)
  - OpenType: bool (value, optional)
  - HasStream: bool (value, optional)

<a id="element-complextype"></a>

### Element: ComplexType

- Parent: [Schema](#element-schema)
- Children:
  - [Property](#element-property) (0..\*)
  - [NavigationProperty](#element-navigationproperty) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - BaseType: String (reference, optional) [ComplexType](#element-complextype)
  - Abstract: bool (value, optional)
  - OpenType: bool (value, optional)

<a id="element-enumtype"></a>

### Element: EnumType

- Parent: [Schema](#element-schema)
- Children:
  - [Member](#element-member) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - UnderlyingType: String (reference, optional) Edm.PrimitiveType
  - IsFlags: bool (value, optional)

<a id="element-member"></a>

### Element: Member

- Parent: [EnumType](#element-enumtype)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - Value: Int64 (value, optional)

<a id="element-typedefinition"></a>

### Element: TypeDefinition

- Parent: [Schema](#element-schema)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - UnderlyingType: String (reference, required) Edm.PrimitiveType
  - MaxLength: String|Int (value, optional)
  - Precision: Int (value, optional)
  - Scale: Int|variable (value, optional; token variable is lowercase)
  - SRID: Int|variable (value, optional; token variable is lowercase)
  - Unicode: bool (value, optional)

<a id="element-term"></a>

### Element: Term

- Parent: [Schema](#element-schema)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - Type: String (reference, required) Edm.PrimitiveType | [TypeDefinition](#element-typedefinition) | [EnumType](#element-enumtype) | [ComplexType](#element-complextype) | [EntityType](#element-entitytype)
  - BaseTerm: String (reference, optional) [Term](#element-term)
  - DefaultValue: String (value, optional)
  - AppliesTo: String (value, optional)
  - Nullable: bool (value, optional)
  - MaxLength: String|Int (value, optional)
  - Precision: Int (value, optional)
  - Scale: Int|variable (value, optional; token variable is lowercase)
  - SRID: Int|variable (value, optional; token variable is lowercase)
  - Unicode: bool (value, optional)

<a id="element-property"></a>

### Element: Property

- Parent: [EntityType](#element-entitytype) | [ComplexType](#element-complextype)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - Type: String (reference, required) Edm.PrimitiveType | [TypeDefinition](#element-typedefinition) | [EnumType](#element-enumtype) | [ComplexType](#element-complextype)
  - Nullable: bool (value, optional)
  - MaxLength: String|Int (value, optional)
  - Precision: Int (value, optional)
  - Scale: Int|variable (value, optional; token variable is lowercase)
  - SRID: Int|variable (value, optional; token variable is lowercase)
  - Unicode: bool (value, optional)
  - DefaultValue: String (value, optional)

<a id="element-navigationproperty"></a>

### Element: NavigationProperty

- Parent: [EntityType](#element-entitytype) | [ComplexType](#element-complextype)
- Children:
  - [ReferentialConstraint](#element-referentialconstraint) (0..\*)
  - [OnDelete](#element-ondelete) (0..1)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - Type: String (reference, required) [EntityType](#element-entitytype)
  - Nullable: bool (value, optional)
  - Partner: String (reference, optional) [NavigationProperty](#element-navigationproperty)
  - ContainsTarget: bool (value, optional)

## 4. Operation Elements

<a id="element-action"></a>

### Element: Action

- Parent: [Schema](#element-schema)
- Children:
  - [Parameter](#element-parameter) (0..\*)
  - [ReturnType](#element-returntype) (0..1)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - IsBound: bool (value, optional)
  - EntitySetPath: String (value, optional)

<a id="element-function"></a>

### Element: Function

- Parent: [Schema](#element-schema)
- Children:
  - [Parameter](#element-parameter) (0..\*)
  - [ReturnType](#element-returntype) (1)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - IsBound: bool (value, optional)
  - IsComposable: bool (value, optional)
  - EntitySetPath: String (value, optional)

<a id="element-parameter"></a>

### Element: Parameter

- Parent: [Action](#element-action) | [Function](#element-function)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - Type: String (reference, required) Edm.PrimitiveType | [TypeDefinition](#element-typedefinition) | [EnumType](#element-enumtype) | [ComplexType](#element-complextype) | [EntityType](#element-entitytype)
  - Nullable: bool (value, optional)
  - MaxLength: String|Int (value, optional)
  - Precision: Int (value, optional)
  - Scale: Int|variable (value, optional; token variable is lowercase)
  - SRID: Int|variable (value, optional; token variable is lowercase)
  - Unicode: bool (value, optional)
  - DefaultValue: String (value, optional)

<a id="element-returntype"></a>

### Element: ReturnType

- Parent: [Action](#element-action) | [Function](#element-function)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Type: String (reference, required) Edm.PrimitiveType | [TypeDefinition](#element-typedefinition) | [EnumType](#element-enumtype) | [ComplexType](#element-complextype) | [EntityType](#element-entitytype)
  - Nullable: bool (value, optional)
  - MaxLength: String|Int (value, optional)
  - Precision: Int (value, optional)
  - Scale: Int|variable (value, optional; token variable is lowercase)
  - SRID: Int|variable (value, optional; token variable is lowercase)
  - Unicode: bool (value, optional)

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
  - Name: String (value, required)
  - Extends: String (reference, optional) [EntityContainer](#element-entitycontainer)

<a id="element-entityset"></a>

### Element: EntitySet

- Parent: [EntityContainer](#element-entitycontainer)
- Children:
  - [NavigationPropertyBinding](#element-navigationpropertybinding) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - EntityType: String (reference, required) [EntityType](#element-entitytype)
  - IncludeInServiceDocument: bool (value, optional)

<a id="element-singleton"></a>

### Element: Singleton

- Parent: [EntityContainer](#element-entitycontainer)
- Children:
  - [NavigationPropertyBinding](#element-navigationpropertybinding) (0..\*)
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - Type: String (reference, required) [EntityType](#element-entitytype)
  - IncludeInServiceDocument: bool (value, optional)

<a id="element-actionimport"></a>

### Element: ActionImport

- Parent: [EntityContainer](#element-entitycontainer)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - Action: String (reference, required) [Action](#element-action)
  - EntitySet: String (reference, optional) [EntitySet](#element-entityset)
  - IncludeInServiceDocument: bool (value, optional)

<a id="element-functionimport"></a>

### Element: FunctionImport

- Parent: [EntityContainer](#element-entitycontainer)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Name: String (value, required)
  - Function: String (reference, required) [Function](#element-function)
  - EntitySet: String (reference, optional) [EntitySet](#element-entityset)
  - IncludeInServiceDocument: bool (value, optional)

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
  - Name: String (reference, required) [Property](#element-property)
  - Alias: String (value, optional)

<a id="element-referentialconstraint"></a>

### Element: ReferentialConstraint

- Parent: [NavigationProperty](#element-navigationproperty)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Property: String (reference, required) [Property](#element-property)
  - ReferencedProperty: String (reference, required) [Property](#element-property)

<a id="element-ondelete"></a>

### Element: OnDelete

- Parent: [NavigationProperty](#element-navigationproperty)
- Children:
  - [Annotation](#element-annotation) (0..\*)
- Attributes:
  - Action: enum (value, required) Cascade | None | SetNull | SetDefault

<a id="element-navigationpropertybinding"></a>

### Element: NavigationPropertyBinding

- Parent: [EntitySet](#element-entityset) | [Singleton](#element-singleton)
- Children: none
- Attributes:
  - Path: String (reference, required) [NavigationProperty](#element-navigationproperty)
  - Target: String (reference, required) [EntitySet](#element-entityset) | [Singleton](#element-singleton)

## 7. Annotation Elements

<a id="element-annotations"></a>

### Element: Annotations

- Parent: [Schema](#element-schema)
- Children:
  - [Annotation](#element-annotation) (1..\*)
- Attributes:
  - Target: String (reference, required) model element path
  - Qualifier: String (value, optional)

<a id="element-annotation"></a>

### Element: Annotation

- Parent: any annotatable element and [Annotations](#element-annotations)
- Children:
  - nested [Annotation](#element-annotation) (0..\*)
  - expression payload. Not an Element in the sense used here. Out of scope in this document
- Attributes:
  - Term: String (reference, required) [Term](#element-term)
  - Qualifier: String (value, optional)
  - Path: String (reference, optional) expression path domain

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
