//! Qualified names — the simplest piece of the CSDL reference language.

use std::fmt;
use std::str::FromStr;

/// `Namespace.Name`. The namespace may contain dots (`Org.OData.Core.V1`);
/// the last dot separates namespace from local name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedName {
    pub namespace: String,
    pub name: String,
}

impl QualifiedName {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }
}

impl fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.namespace, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseQualifiedNameError;

impl fmt::Display for ParseQualifiedNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("qualified name must contain a '.' separating namespace from local name")
    }
}

impl std::error::Error for ParseQualifiedNameError {}

impl FromStr for QualifiedName {
    type Err = ParseQualifiedNameError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.rsplit_once('.') {
            Some((ns, n)) if !ns.is_empty() && !n.is_empty() => Ok(QualifiedName {
                namespace: ns.to_string(),
                name: n.to_string(),
            }),
            _ => Err(ParseQualifiedNameError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple() {
        assert_eq!(
            "Sales.Customer".parse::<QualifiedName>().unwrap(),
            QualifiedName::new("Sales", "Customer")
        );
    }

    #[test]
    fn parses_multipart_namespace() {
        assert_eq!(
            "Org.OData.Core.V1.Description"
                .parse::<QualifiedName>()
                .unwrap(),
            QualifiedName::new("Org.OData.Core.V1", "Description")
        );
    }

    #[test]
    fn rejects_unqualified() {
        assert!("Customer".parse::<QualifiedName>().is_err());
    }

    #[test]
    fn display_round_trips() {
        let q = QualifiedName::new("Sales", "Customer");
        assert_eq!(format!("{q}"), "Sales.Customer");
        assert_eq!(q, format!("{q}").parse().unwrap());
    }
}
