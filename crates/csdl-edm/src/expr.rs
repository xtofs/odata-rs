#[derive(Debug, Clone, PartialEq)]
pub enum CsdlAnnotationExpression {
    // Constant expressions
    Binary(Vec<u8>),
    Bool(bool),
    Date(String),
    DateTimeOffset(String),
    Decimal(String),
    Duration(String),
    EnumMember(String),
    Float(f64),
    Guid(String),
    Int(i64),
    String(String),
    TimeOfDay(String),
    Null,

    // Path expressions
    Path(String),
    PropertyPath(String),
    NavigationPropertyPath(String),
    AnnotationPath(String),

    // Dynamic expressions
    Not(Box<CsdlAnnotationExpression>),
    BinaryExpression {
        op: BinaryOperator,
        lhs: Box<CsdlAnnotationExpression>,
        rhs: Box<CsdlAnnotationExpression>,
    },

    If {
        test: Box<CsdlAnnotationExpression>,
        then_: Box<CsdlAnnotationExpression>,
        else_: Option<Box<CsdlAnnotationExpression>>,
    },
    Apply {
        function: String,
        args: Vec<CsdlAnnotationExpression>,
    },
    Cast {
        type_: String,
        expr: Box<CsdlAnnotationExpression>,
    },
    IsOf {
        type_: String,
        expr: Box<CsdlAnnotationExpression>,
    },

    Record {
        type_: Option<String>,
        properties: Vec<PropertyValue>,
        /// Record-level own- and sibling-target annotations (CSDL 14.5.14).
        /// `<Annotation>` children of `<Record>` in XML; `@Term` keys on the
        /// Record object in JSON. Sibling-target keys (`Foo@Term`) attach to
        /// the named `PropertyValue` via `PropertyValue::annotations`, not
        /// here.
        annotations: Vec<Annotation>,
    },
    Collection(Vec<CsdlAnnotationExpression>),

    LabeledElement {
        name: String,
        expr: Box<CsdlAnnotationExpression>,
    },
    LabeledElementReference(String),

    UrlRef(Box<CsdlAnnotationExpression>),
}

/// Operator for a binary [`CsdlAnnotationExpression::Binary`] expression.
/// Mirrors the CSDL 4.01 binary dynamic-expression element names (14.5.1, 14.5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    And,
    Or,
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

/// A single PropertyValue inside a Record (CSDL 14.5.14). Semantically carries
/// one annotation expression. The reader's recursive parser takes the first
/// expression it sees (inline attribute form takes precedence over nested
/// element form); any extras are dropped. `None` means no expression was
/// present at all.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyValue {
    pub property: String,
    pub value: Option<CsdlAnnotationExpression>,
    pub annotations: Vec<Annotation>,
}

// `Annotation` lives in `model.rs`. `PropertyValue` carries them too, so we
// import the model definition here. The two modules are mutually-referential
// (model uses CsdlAnnotationExpression from here, we use Annotation from
// model) — Rust handles that fine.
use crate::csdl::Annotation;
