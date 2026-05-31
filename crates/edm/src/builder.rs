//! Build an [`EdmModel`] from a [`CsdlReader`] token stream.
//!
//! The builder is a small stack machine: every CSDL element start pushes a
//! [`Frame`] holding the partial in-flight struct; every end pops the frame
//! and integrates the now-complete struct into its parent (or into the model
//! itself for `Schema`). `AnnotationExpression` tokens flow into the topmost
//! `Frame::Annotation`.
//!
//! Unknown elements are tracked as [`Frame::Unknown`] so the depth stays
//! consistent without erroring — that keeps the builder forward-compatible
//! with CSDL constructs we haven't modeled yet (Action, Function, Term) as
//! well as wrapper elements (`edmx:Edmx`, `edmx:DataServices`, `Reference`).

use std::borrow::Cow;
use std::io::BufRead;

use crate::Result;
use crate::error::Error;
use crate::expr::{Annotation, CsdlAnnotationExpression};
use crate::syntactic::*;
use crate::reader::{CsdlReader, CsdlToken, Location};

/// CSDL element meta-table: the structural constraints the builder enforces.
mod meta;


pub fn build_model<R: BufRead>(reader: &mut CsdlReader<R>) -> Result<EdmModel> {
    let mut b = Builder::default();
    b.run(reader)?;
    Ok(b.model)
}

#[derive(Default)]
struct Builder {
    model: EdmModel,
    stack: Vec<Frame>,
}

enum Local {
    Start {
        name: String,
        attrs: Vec<(String, String)>,
    },
    End {
        name: String,
    },
    AnnotationExpression(CsdlAnnotationExpression),
}

enum Frame {
    Schema(Schema),
    EntityType(EntityType),
    ComplexType(ComplexType),
    EnumType(EnumType),
    EnumMember(EnumMember),
    TypeDefinition(TypeDefinition),
    Property(Property),
    NavigationProperty(NavigationProperty),
    ReferentialConstraint(ReferentialConstraint),
    Key(Key),
    PropertyRef(PropertyRef),
    EntityContainer(EntityContainer),
    EntitySet(EntitySet),
    Singleton(Singleton),
    NavigationPropertyBinding(NavigationPropertyBinding),
    Annotation {
        annotation: Annotation,
        expressions_seen: u32,
    },
    /// Element we don't model. Tracks depth so End handling stays correct.
    Unknown { name: String },
}

impl Frame {
    fn label(&self) -> &str {
        match self {
            Self::Schema(_) => "Schema",
            Self::EntityType(_) => "EntityType",
            Self::ComplexType(_) => "ComplexType",
            Self::EnumType(_) => "EnumType",
            Self::EnumMember(_) => "Member",
            Self::TypeDefinition(_) => "TypeDefinition",
            Self::Property(_) => "Property",
            Self::NavigationProperty(_) => "NavigationProperty",
            Self::ReferentialConstraint(_) => "ReferentialConstraint",
            Self::Key(_) => "Key",
            Self::PropertyRef(_) => "PropertyRef",
            Self::EntityContainer(_) => "EntityContainer",
            Self::EntitySet(_) => "EntitySet",
            Self::Singleton(_) => "Singleton",
            Self::NavigationPropertyBinding(_) => "NavigationPropertyBinding",
            Self::Annotation { .. } => "Annotation",
            Self::Unknown { name, .. } => name.as_str(),
        }
    }
}

impl Builder {
    fn run<R: BufRead>(&mut self, reader: &mut CsdlReader<R>) -> Result<()> {
        loop {
            // Detach the token from `reader`'s lifetime so we can read its
            // location afterward without the borrow checker tripping over the
            // (already consumed) Cow borrow.
            let local = match reader.next_token()? {
                None => return Ok(()),
                Some(CsdlToken::StartCsdlElement { name, attributes }) => Local::Start {
                    name: name.into_owned(),
                    attrs: attributes
                        .into_iter()
                        .map(|(k, v)| (k.into_owned(), v.into_owned()))
                        .collect(),
                },
                Some(CsdlToken::EndCsdlElement { name }) => Local::End {
                    name: name.into_owned(),
                },
                Some(CsdlToken::AnnotationExpression(e)) => Local::AnnotationExpression(e),
            };
            let loc = reader.current_location();
            match local {
                Local::Start { name, attrs } => {
                    // Re-wrap attrs as Cow<str> so handle_start can stay generic.
                    let attrs: Vec<(Cow<'_, str>, Cow<'_, str>)> = attrs
                        .iter()
                        .map(|(k, v)| (Cow::Borrowed(k.as_str()), Cow::Borrowed(v.as_str())))
                        .collect();
                    self.handle_start(&name, &attrs, loc)?;
                }
                Local::End { name } => self.handle_end(&name, loc)?,
                Local::AnnotationExpression(e) => self.handle_annotation_expr(e),
            }
        }
    }

    fn handle_start(
        &mut self,
        name: &str,
        attrs: &[(Cow<'_, str>, Cow<'_, str>)],
        loc: Location,
    ) -> Result<()> {
        // Single validation prelude using the meta-table — no per-arm rule
        // duplication. Unknown elements have no rule and fall through.
        self.validate_parent(name, loc)?;
        match name {
            "Schema" => {
                self.stack.push(Frame::Schema(Schema {
                    namespace: attr_owned(attrs, "Namespace").unwrap_or_default(),
                    alias: attr_owned(attrs, "Alias"),
                    entity_types: Vec::new(),
                    complex_types: Vec::new(),
                    enum_types: Vec::new(),
                    type_definitions: Vec::new(),
                    entity_containers: Vec::new(),
                    annotations: Vec::new(),
                }));
            }
            "EntityType" => {
                self.stack.push(Frame::EntityType(EntityType {
                    name: attr_owned(attrs, "Name").unwrap_or_default(),
                    base_type: attr_owned(attrs, "BaseType"),
                    abstract_: attr_bool(attrs, "Abstract", false),
                    open_type: attr_bool(attrs, "OpenType", false),
                    has_stream: attr_bool(attrs, "HasStream", false),
                    key: None,
                    properties: Vec::new(),
                    navigation_properties: Vec::new(),
                    annotations: Vec::new(),
                }));
            }
            "ComplexType" => {
                self.stack.push(Frame::ComplexType(ComplexType {
                    name: attr_owned(attrs, "Name").unwrap_or_default(),
                    base_type: attr_owned(attrs, "BaseType"),
                    abstract_: attr_bool(attrs, "Abstract", false),
                    open_type: attr_bool(attrs, "OpenType", false),
                    properties: Vec::new(),
                    navigation_properties: Vec::new(),
                    annotations: Vec::new(),
                }));
            }
            "Key" => {
                self.stack.push(Frame::Key(Key {
                    property_refs: Vec::new(),
                }));
            }
            "PropertyRef" => {
                self.stack.push(Frame::PropertyRef(PropertyRef {
                    name: attr_owned(attrs, "Name").unwrap_or_default(),
                    alias: attr_owned(attrs, "Alias"),
                }));
            }
            "Property" => {
                self.stack.push(Frame::Property(Property {
                    name: attr_owned(attrs, "Name").unwrap_or_default(),
                    type_: attr_owned(attrs, "Type").unwrap_or_default(),
                    nullable: attr_bool(attrs, "Nullable", true),
                    facets: parse_facets(attrs),
                    annotations: Vec::new(),
                }));
            }
            "NavigationProperty" => {
                self.stack.push(Frame::NavigationProperty(NavigationProperty {
                    name: attr_owned(attrs, "Name").unwrap_or_default(),
                    type_: attr_owned(attrs, "Type").unwrap_or_default(),
                    nullable: attr_bool(attrs, "Nullable", true),
                    partner: attr_owned(attrs, "Partner"),
                    contains_target: attr_bool(attrs, "ContainsTarget", false),
                    referential_constraints: Vec::new(),
                    on_delete: None,
                    annotations: Vec::new(),
                }));
            }
            "ReferentialConstraint" => {
                self.stack.push(Frame::ReferentialConstraint(ReferentialConstraint {
                    property: attr_owned(attrs, "Property").unwrap_or_default(),
                    referenced_property: attr_owned(attrs, "ReferencedProperty")
                        .unwrap_or_default(),
                    annotations: Vec::new(),
                }));
            }
            "OnDelete" => {
                let raw = attr(attrs, "Action").unwrap_or("None");
                let action = match raw {
                    "Cascade" => OnDeleteAction::Cascade,
                    "None" => OnDeleteAction::None,
                    "SetNull" => OnDeleteAction::SetNull,
                    "SetDefault" => OnDeleteAction::SetDefault,
                    other => return Err(err_at(loc, format!("unknown OnDelete Action='{other}'"))),
                };
                if let Some(Frame::NavigationProperty(np)) = self.stack.last_mut() {
                    np.on_delete = Some(action);
                }
                let _ = loc;
                self.stack.push(Frame::Unknown {
                    name: "OnDelete".to_string(),
                });
            }
            "EnumType" => {
                self.stack.push(Frame::EnumType(EnumType {
                    name: attr_owned(attrs, "Name").unwrap_or_default(),
                    underlying_type: attr_owned(attrs, "UnderlyingType"),
                    is_flags: attr_bool(attrs, "IsFlags", false),
                    members: Vec::new(),
                    annotations: Vec::new(),
                }));
            }
            "Member" => {
                self.stack.push(Frame::EnumMember(EnumMember {
                    name: attr_owned(attrs, "Name").unwrap_or_default(),
                    value: attr(attrs, "Value").and_then(|s| s.parse().ok()),
                    annotations: Vec::new(),
                }));
            }
            "TypeDefinition" => {
                self.stack.push(Frame::TypeDefinition(TypeDefinition {
                    name: attr_owned(attrs, "Name").unwrap_or_default(),
                    underlying_type: attr_owned(attrs, "UnderlyingType").unwrap_or_default(),
                    facets: parse_facets(attrs),
                    annotations: Vec::new(),
                }));
            }
            "EntityContainer" => {
                self.stack.push(Frame::EntityContainer(EntityContainer {
                    name: attr_owned(attrs, "Name").unwrap_or_default(),
                    extends: attr_owned(attrs, "Extends"),
                    entity_sets: Vec::new(),
                    singletons: Vec::new(),
                    annotations: Vec::new(),
                }));
            }
            "EntitySet" => {
                self.stack.push(Frame::EntitySet(EntitySet {
                    name: attr_owned(attrs, "Name").unwrap_or_default(),
                    entity_type: attr_owned(attrs, "EntityType").unwrap_or_default(),
                    include_in_service_document: attr_bool(
                        attrs,
                        "IncludeInServiceDocument",
                        true,
                    ),
                    navigation_property_bindings: Vec::new(),
                    annotations: Vec::new(),
                }));
            }
            "Singleton" => {
                self.stack.push(Frame::Singleton(Singleton {
                    name: attr_owned(attrs, "Name").unwrap_or_default(),
                    type_: attr_owned(attrs, "Type").unwrap_or_default(),
                    navigation_property_bindings: Vec::new(),
                    annotations: Vec::new(),
                }));
            }
            "NavigationPropertyBinding" => {
                self.stack.push(Frame::NavigationPropertyBinding(NavigationPropertyBinding {
                    path: attr_owned(attrs, "Path").unwrap_or_default(),
                    target: attr_owned(attrs, "Target").unwrap_or_default(),
                }));
            }
            "Annotation" => {
                self.stack.push(Frame::Annotation {
                    annotation: Annotation {
                        term: attr_owned(attrs, "Term").unwrap_or_default(),
                        qualifier: attr_owned(attrs, "Qualifier"),
                        expression: None,
                    },
                    expressions_seen: 0,
                });
            }
            _ => {
                let _ = loc;
                self.stack.push(Frame::Unknown {
                    name: name.to_string(),
                });
            }
        }
        Ok(())
    }

    fn handle_end(&mut self, name: &str, loc: Location) -> Result<()> {
        let frame = self.stack.pop().ok_or_else(|| {
            err_at(loc, format!("unexpected </{name}> with empty element stack"))
        })?;
        if frame.label() != name {
            return Err(err_at(
                loc,
                format!("</{name}> closes <{}>", frame.label()),
            ));
        }

        match frame {
            Frame::Schema(s) => self.model.schemas.push(s),
            Frame::EntityType(et) => {
                if let Some(Frame::Schema(s)) = self.stack.last_mut() {
                    s.entity_types.push(et);
                }
            }
            Frame::ComplexType(ct) => {
                if let Some(Frame::Schema(s)) = self.stack.last_mut() {
                    s.complex_types.push(ct);
                }
            }
            Frame::EnumType(et) => {
                if let Some(Frame::Schema(s)) = self.stack.last_mut() {
                    s.enum_types.push(et);
                }
            }
            Frame::EnumMember(em) => {
                if let Some(Frame::EnumType(et)) = self.stack.last_mut() {
                    et.members.push(em);
                }
            }
            Frame::TypeDefinition(td) => {
                if let Some(Frame::Schema(s)) = self.stack.last_mut() {
                    s.type_definitions.push(td);
                }
            }
            Frame::EntityContainer(ec) => {
                if let Some(Frame::Schema(s)) = self.stack.last_mut() {
                    s.entity_containers.push(ec);
                }
            }
            Frame::EntitySet(es) => {
                if let Some(Frame::EntityContainer(ec)) = self.stack.last_mut() {
                    ec.entity_sets.push(es);
                }
            }
            Frame::Singleton(s) => {
                if let Some(Frame::EntityContainer(ec)) = self.stack.last_mut() {
                    ec.singletons.push(s);
                }
            }
            Frame::NavigationPropertyBinding(npb) => match self.stack.last_mut() {
                Some(Frame::EntitySet(es)) => es.navigation_property_bindings.push(npb),
                Some(Frame::Singleton(s)) => s.navigation_property_bindings.push(npb),
                _ => {}
            },
            Frame::Property(p) => match self.stack.last_mut() {
                Some(Frame::EntityType(et)) => et.properties.push(p),
                Some(Frame::ComplexType(ct)) => ct.properties.push(p),
                _ => {}
            },
            Frame::NavigationProperty(np) => match self.stack.last_mut() {
                Some(Frame::EntityType(et)) => et.navigation_properties.push(np),
                Some(Frame::ComplexType(ct)) => ct.navigation_properties.push(np),
                _ => {}
            },
            Frame::Key(k) => {
                if let Some(Frame::EntityType(et)) = self.stack.last_mut() {
                    et.key = Some(k);
                }
            }
            Frame::PropertyRef(pr) => {
                if let Some(Frame::Key(k)) = self.stack.last_mut() {
                    k.property_refs.push(pr);
                }
            }
            Frame::ReferentialConstraint(rc) => {
                if let Some(Frame::NavigationProperty(np)) = self.stack.last_mut() {
                    np.referential_constraints.push(rc);
                }
            }
            Frame::Annotation { annotation, .. } => {
                self.attach_annotation(annotation);
            }
            Frame::Unknown { .. } => {}
        }
        Ok(())
    }

    fn handle_annotation_expr(&mut self, expr: CsdlAnnotationExpression) {
        if let Some(Frame::Annotation {
            annotation,
            expressions_seen,
        }) = self.stack.last_mut()
        {
            *expressions_seen += 1;
            if *expressions_seen == 1 {
                annotation.expression = Some(expr);
            } else {
                eprintln!(
                    "warning: annotation '{}' has more than one expression; keeping the first, ignoring {expr:?}",
                    annotation.term
                );
            }
        }
    }

    fn attach_annotation(&mut self, ann: Annotation) {
        match self.stack.last_mut() {
            Some(Frame::Schema(s)) => s.annotations.push(ann),
            Some(Frame::EntityType(et)) => et.annotations.push(ann),
            Some(Frame::ComplexType(ct)) => ct.annotations.push(ann),
            Some(Frame::EnumType(et)) => et.annotations.push(ann),
            Some(Frame::EnumMember(em)) => em.annotations.push(ann),
            Some(Frame::TypeDefinition(td)) => td.annotations.push(ann),
            Some(Frame::Property(p)) => p.annotations.push(ann),
            Some(Frame::NavigationProperty(np)) => np.annotations.push(ann),
            Some(Frame::ReferentialConstraint(rc)) => rc.annotations.push(ann),
            Some(Frame::EntityContainer(ec)) => ec.annotations.push(ann),
            Some(Frame::EntitySet(es)) => es.annotations.push(ann),
            Some(Frame::Singleton(s)) => s.annotations.push(ann),
            _ => {}
        }
    }

    /// Validate the current top-of-stack against the meta-table's rule for
    /// `name`. Elements with no rule (unknown / forward-compat wrappers like
    /// `edmx:Edmx`) pass through.
    fn validate_parent(&self, name: &str, loc: Location) -> Result<()> {
        let Some(rule) = meta::rule(name) else {
            return Ok(());
        };
        match rule.parents {
            meta::ParentSpec::Any => Ok(()),
            meta::ParentSpec::OneOf(allowed) => {
                let parent = self.stack.last().map(|f| f.label());
                if parent.map_or(false, |p| allowed.contains(&p)) {
                    Ok(())
                } else {
                    Err(err_at(
                        loc,
                        format!(
                            "<{name}> not allowed inside <{}>",
                            parent.unwrap_or("(root)")
                        ),
                    ))
                }
            }
        }
    }
}

fn err_at(loc: Location, msg: String) -> Error {
    Error::Csdl(format!(
        "at line {}, column {}: {msg}",
        loc.line, loc.column
    ))
}

fn attr<'a>(attrs: &'a [(Cow<'_, str>, Cow<'_, str>)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k.as_ref() == key)
        .map(|(_, v)| v.as_ref())
}

fn attr_owned(attrs: &[(Cow<'_, str>, Cow<'_, str>)], key: &str) -> Option<String> {
    attr(attrs, key).map(|s| s.to_string())
}

fn attr_bool(attrs: &[(Cow<'_, str>, Cow<'_, str>)], key: &str, default: bool) -> bool {
    attr(attrs, key)
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

fn parse_facets(attrs: &[(Cow<'_, str>, Cow<'_, str>)]) -> Facets {
    Facets {
        max_length: attr(attrs, "MaxLength").map(parse_max_length),
        precision: attr(attrs, "Precision").and_then(|s| s.parse().ok()),
        scale: attr(attrs, "Scale").map(parse_scale),
        srid: attr(attrs, "SRID").map(parse_srid),
        unicode: attr(attrs, "Unicode").map(|v| v.eq_ignore_ascii_case("true")),
        default_value: attr_owned(attrs, "DefaultValue"),
    }
}

fn parse_max_length(s: &str) -> MaxLength {
    if s.eq_ignore_ascii_case("max") {
        MaxLength::Max
    } else {
        MaxLength::Fixed(s.parse().unwrap_or(0))
    }
}

fn parse_scale(s: &str) -> Scale {
    if s.eq_ignore_ascii_case("variable") {
        Scale::Variable
    } else if s.eq_ignore_ascii_case("floating") {
        Scale::Floating
    } else {
        Scale::Fixed(s.parse().unwrap_or(0))
    }
}

fn parse_srid(s: &str) -> Srid {
    if s.eq_ignore_ascii_case("variable") {
        Srid::Variable
    } else {
        Srid::Value(s.parse().unwrap_or(0))
    }
}
