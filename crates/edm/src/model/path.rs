//! [`TargetPath`] AST + parser — the "small language" used for external
//! annotation targets, dynamic-expression paths, and similar string-encoded
//! references throughout CSDL.
//!
//! Grammar (informal):
//! ```text
//! TargetPath  := NamedRef ( '/' Segment )* ( '@' QName ( '#' Qualifier )? )?
//! NamedRef    := QName ( '(' SigArg ( ',' SigArg )* ')' )?
//! SigArg      := QName
//!              | 'Collection' '(' SigArg ')'
//! QName       := Ident ( '.' Ident )+
//! Segment     := Ident
//! Qualifier   := Ident
//! Ident       := [A-Za-z_] [A-Za-z0-9_]*
//! ```
//!
//! Examples:
//!   `Sales.Customer`
//!   `Sales.Customer/Name`
//!   `Sales.Customer@Core.Description`
//!   `Sales.Customer/Name@Core.Description#Tablet`
//!   `Sales.UpdateOrder(Sales.Order, Edm.Int32)`
//!   `Sales.MyFunc(Collection(Sales.Customer))`

use std::fmt;

use super::names::QualifiedName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPath {
    /// The leading namespaced reference (always present).
    pub base: NamedRef,
    /// Slash-separated path segments following the base, in source order.
    /// e.g. for `Sales.Customer/Address/City` this is `["Address", "City"]`.
    pub segments: Vec<String>,
    /// Optional `@Term[#Qualifier]` suffix selecting an annotation usage.
    pub annotation: Option<AnnotationSuffix>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedRef {
    pub qname: QualifiedName,
    /// `Some(args)` for an action/function overload selector like
    /// `Sales.MyFunc(Sales.Customer, Edm.Int32)`. `None` otherwise.
    pub overload: Option<Vec<SignatureArg>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureArg {
    Named(QualifiedName),
    Collection(Box<SignatureArg>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationSuffix {
    pub term: QualifiedName,
    pub qualifier: Option<String>,
}

// ============================================================================
// Parser
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsePathError {
    Empty,
    UnexpectedChar { at: usize, ch: char },
    UnexpectedEnd,
    ExpectedIdent { at: usize },
    UnclosedParen { at: usize },
    TrailingInput { at: usize },
    /// Identifier didn't have the required structure for its position
    /// (e.g. a bare `Foo` where a qualified `Namespace.Name` was expected).
    NotQualified { at: usize },
}

impl fmt::Display for ParsePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty target path"),
            Self::UnexpectedChar { at, ch } => {
                write!(f, "unexpected character {ch:?} at byte {at}")
            }
            Self::UnexpectedEnd => f.write_str("unexpected end of input"),
            Self::ExpectedIdent { at } => write!(f, "expected identifier at byte {at}"),
            Self::UnclosedParen { at } => write!(f, "unclosed '(' opened at byte {at}"),
            Self::TrailingInput { at } => write!(f, "trailing input at byte {at}"),
            Self::NotQualified { at } => {
                write!(f, "expected a qualified name (Namespace.Name) at byte {at}")
            }
        }
    }
}

impl std::error::Error for ParsePathError {}

impl TargetPath {
    pub fn parse(input: &str) -> Result<Self, ParsePathError> {
        let mut p = Parser {
            input: input.as_bytes(),
            pos: 0,
        };
        if p.input.is_empty() {
            return Err(ParsePathError::Empty);
        }
        let base = p.parse_named_ref()?;
        let mut segments = Vec::new();
        while p.peek() == Some(b'/') {
            p.bump();
            segments.push(p.parse_ident()?.to_string());
        }
        let annotation = if p.peek() == Some(b'@') {
            p.bump();
            let term = p.parse_qualified_name()?;
            let qualifier = if p.peek() == Some(b'#') {
                p.bump();
                Some(p.parse_ident()?.to_string())
            } else {
                None
            };
            Some(AnnotationSuffix { term, qualifier })
        } else {
            None
        };
        if p.pos < p.input.len() {
            return Err(ParsePathError::TrailingInput { at: p.pos });
        }
        Ok(TargetPath {
            base,
            segments,
            annotation,
        })
    }
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn parse_ident(&mut self) -> Result<&'a str, ParsePathError> {
        let start = self.pos;
        match self.peek() {
            Some(b) if is_ident_start(b) => self.bump(),
            _ => return Err(ParsePathError::ExpectedIdent { at: start }),
        }
        while let Some(b) = self.peek() {
            if is_ident_cont(b) {
                self.bump();
            } else {
                break;
            }
        }
        Ok(std::str::from_utf8(&self.input[start..self.pos]).expect("ASCII ident"))
    }

    /// Parses a dotted name like `Sales.Customer` or `Org.OData.Core.V1.Term`.
    /// Requires at least one dot (i.e. namespace.name).
    fn parse_qualified_name(&mut self) -> Result<QualifiedName, ParsePathError> {
        let start = self.pos;
        let first = self.parse_ident()?.to_string();
        let mut parts = vec![first];
        while self.peek() == Some(b'.') {
            self.bump();
            parts.push(self.parse_ident()?.to_string());
        }
        if parts.len() < 2 {
            return Err(ParsePathError::NotQualified { at: start });
        }
        let name = parts.pop().unwrap();
        let namespace = parts.join(".");
        Ok(QualifiedName { namespace, name })
    }

    fn parse_named_ref(&mut self) -> Result<NamedRef, ParsePathError> {
        let qname = self.parse_qualified_name()?;
        let overload = if self.peek() == Some(b'(') {
            let open_at = self.pos;
            self.bump();
            let mut args = Vec::new();
            if self.peek() != Some(b')') {
                loop {
                    args.push(self.parse_signature_arg()?);
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => {
                            self.bump();
                            self.skip_ws();
                        }
                        Some(b')') => break,
                        Some(c) => {
                            return Err(ParsePathError::UnexpectedChar {
                                at: self.pos,
                                ch: c as char,
                            });
                        }
                        None => return Err(ParsePathError::UnclosedParen { at: open_at }),
                    }
                }
            }
            if self.peek() != Some(b')') {
                return Err(ParsePathError::UnclosedParen { at: open_at });
            }
            self.bump();
            Some(args)
        } else {
            None
        };
        Ok(NamedRef { qname, overload })
    }

    fn parse_signature_arg(&mut self) -> Result<SignatureArg, ParsePathError> {
        self.skip_ws();
        // Check for "Collection(" — Collection is a reserved keyword in
        // CSDL type syntax, not a normal qualified name. We detect it by
        // peeking the literal identifier and the following '('.
        if self.starts_with(b"Collection(") {
            self.pos += b"Collection".len();
            let open_at = self.pos;
            self.bump();
            let inner = self.parse_signature_arg()?;
            self.skip_ws();
            if self.peek() != Some(b')') {
                return Err(ParsePathError::UnclosedParen { at: open_at });
            }
            self.bump();
            Ok(SignatureArg::Collection(Box::new(inner)))
        } else {
            self.parse_qualified_name().map(SignatureArg::Named)
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.bump();
        }
    }

    fn starts_with(&self, needle: &[u8]) -> bool {
        self.input[self.pos..].starts_with(needle)
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_cont(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qn(ns: &str, n: &str) -> QualifiedName {
        QualifiedName::new(ns, n)
    }

    #[test]
    fn just_a_qualified_name() {
        let p = TargetPath::parse("Sales.Customer").unwrap();
        assert_eq!(p.base.qname, qn("Sales", "Customer"));
        assert!(p.base.overload.is_none());
        assert!(p.segments.is_empty());
        assert!(p.annotation.is_none());
    }

    #[test]
    fn multipart_namespace() {
        let p = TargetPath::parse("Org.OData.Core.V1.Description").unwrap();
        assert_eq!(p.base.qname, qn("Org.OData.Core.V1", "Description"));
    }

    #[test]
    fn path_segments() {
        let p = TargetPath::parse("Sales.Customer/Address/City").unwrap();
        assert_eq!(p.base.qname, qn("Sales", "Customer"));
        assert_eq!(p.segments, vec!["Address", "City"]);
    }

    #[test]
    fn annotation_suffix() {
        let p = TargetPath::parse("Sales.Customer@Core.Description").unwrap();
        let a = p.annotation.unwrap();
        assert_eq!(a.term, qn("Core", "Description"));
        assert!(a.qualifier.is_none());
    }

    #[test]
    fn annotation_with_qualifier() {
        let p = TargetPath::parse("Sales.Customer/Name@UI.Importance#Tablet").unwrap();
        assert_eq!(p.segments, vec!["Name"]);
        let a = p.annotation.unwrap();
        assert_eq!(a.term, qn("UI", "Importance"));
        assert_eq!(a.qualifier.as_deref(), Some("Tablet"));
    }

    #[test]
    fn overload_single_arg() {
        let p = TargetPath::parse("Sales.MyFunc(Sales.Customer)").unwrap();
        assert_eq!(p.base.qname, qn("Sales", "MyFunc"));
        let args = p.base.overload.unwrap();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0], SignatureArg::Named(qn("Sales", "Customer")));
    }

    #[test]
    fn overload_multiple_args() {
        let p = TargetPath::parse("Sales.UpdateOrder(Sales.Order, Edm.Int32)").unwrap();
        let args = p.base.overload.unwrap();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], SignatureArg::Named(qn("Sales", "Order")));
        assert_eq!(args[1], SignatureArg::Named(qn("Edm", "Int32")));
    }

    #[test]
    fn overload_collection_arg() {
        let p = TargetPath::parse("Sales.MyFunc(Collection(Sales.Customer))").unwrap();
        let args = p.base.overload.unwrap();
        assert_eq!(
            args[0],
            SignatureArg::Collection(Box::new(SignatureArg::Named(qn("Sales", "Customer"))))
        );
    }

    #[test]
    fn overload_no_args() {
        let p = TargetPath::parse("Sales.MyFunc()").unwrap();
        let args = p.base.overload.unwrap();
        assert!(args.is_empty());
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(TargetPath::parse(""), Err(ParsePathError::Empty));
    }

    #[test]
    fn rejects_unqualified_base() {
        assert!(matches!(
            TargetPath::parse("Customer"),
            Err(ParsePathError::NotQualified { .. })
        ));
    }
}
