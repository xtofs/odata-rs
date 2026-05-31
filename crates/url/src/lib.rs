use std::collections::BTreeMap;

use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Parsed OData request data produced from a URL.
///
/// This type is pure data: it captures the resource path, well-known OData
/// query options, custom query options, and URL metadata without attaching any
/// execution behavior.
pub struct ODataQuery {
    /// The parsed input URL.
    pub url: Url,
    /// The OData resource path segments, excluding path markers.
    pub resource_path: ResourcePath,
    /// Path marker present as a flag when the URL contains `/$each`.
    pub each: bool,
    /// Path marker present as a flag when the URL contains `/$count`.
    pub count: bool,
    /// Path marker present as a flag when the URL contains `/$ref`.
    pub r#ref: bool,
    /// Path marker present as a flag when the URL contains `/$value`.
    pub value: bool,
    /// Parsed `$select` query option.
    pub select: Option<SelectClause>,
    /// Parsed `$filter` query option body.
    pub filter: Option<FilterClause>,
    /// Parsed `$expand` query option.
    pub expand: Option<ExpandClause>,
    /// Parsed `$top` query option.
    pub top: Option<u64>,
    /// Parsed `$skip` query option.
    pub skip: Option<u64>,
    /// Parsed `$orderby` query option body.
    pub orderby: Option<OrderByClause>,
    /// Parsed inline count query option. `None` means absent.
    pub inlinecount: Option<bool>,
    /// Custom query options not recognized as OData system options.
    pub custom: BTreeMap<String, Vec<String>>,
    /// URL fragment, if present.
    pub fragment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePath {
    pub segments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectClause {
    pub items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterClause {
    pub expression: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandClause {
    pub items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderByClause {
    pub expression: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("duplicate OData query option: {0}")]
    DuplicateQueryOption(String),
    #[error("invalid integer for {option}: {value}")]
    InvalidInteger { option: String, value: String },
    #[error("invalid boolean for {option}: {value}")]
    InvalidBoolean { option: String, value: String },
}

impl ODataQuery {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let url = Url::parse(input).map_err(|error| ParseError::InvalidUrl(error.to_string()))?;
        Self::from_url(url)
    }

    pub fn from_url(url: Url) -> Result<Self, ParseError> {
        let (resource_path, each, count, r#ref, value) = parse_resource_path(&url);

        let mut select: Option<SelectClause> = None;
        let mut filter: Option<FilterClause> = None;
        let mut expand: Option<ExpandClause> = None;
        let mut top: Option<u64> = None;
        let mut skip: Option<u64> = None;
        let mut orderby: Option<OrderByClause> = None;
        let mut inlinecount: Option<bool> = None;
        let mut custom: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for (name, value) in url.query_pairs() {
            let option_name = normalize_query_option_name(&name);
            let option_value = value.into_owned();

            match option_name {
                "select" => {
                    assign_once(&mut select, "select", SelectClause::parse(&option_value)?)?;
                }
                "filter" => {
                    assign_once(
                        &mut filter,
                        "filter",
                        FilterClause {
                            expression: option_value,
                        },
                    )?;
                }
                "expand" => {
                    assign_once(&mut expand, "expand", ExpandClause::parse(&option_value)?)?;
                }
                "top" => {
                    assign_once(
                        &mut top,
                        "top",
                        option_value
                            .parse::<u64>()
                            .map_err(|_| ParseError::InvalidInteger {
                                option: "top".to_string(),
                                value: option_value.clone(),
                            })?,
                    )?;
                }
                "skip" => {
                    assign_once(
                        &mut skip,
                        "skip",
                        option_value
                            .parse::<u64>()
                            .map_err(|_| ParseError::InvalidInteger {
                                option: "skip".to_string(),
                                value: option_value.clone(),
                            })?,
                    )?;
                }
                "orderby" => {
                    assign_once(
                        &mut orderby,
                        "orderby",
                        OrderByClause {
                            expression: option_value,
                        },
                    )?;
                }
                "count" => {
                    assign_once(
                        &mut inlinecount,
                        "inlinecount",
                        parse_boolean(&option_value).ok_or_else(|| ParseError::InvalidBoolean {
                            option: "inlinecount".to_string(),
                            value: option_value.clone(),
                        })?,
                    )?;
                }
                _ => {
                    custom
                        .entry(name.into_owned())
                        .or_default()
                        .push(option_value);
                }
            }
        }

        Ok(Self {
            each,
            custom,
            count,
            expand,
            filter,
            fragment: url.fragment().map(ToOwned::to_owned),
            inlinecount,
            orderby,
            resource_path,
            r#ref,
            select,
            skip,
            top,
            value,
            url,
        })
    }
}

impl SelectClause {
    fn parse(value: &str) -> Result<Self, ParseError> {
        Ok(Self {
            items: split_list(value),
        })
    }
}

impl ExpandClause {
    fn parse(value: &str) -> Result<Self, ParseError> {
        Ok(Self {
            items: split_list(value),
        })
    }
}

fn assign_once<T>(slot: &mut Option<T>, option: &str, value: T) -> Result<(), ParseError> {
    if slot.is_some() {
        return Err(ParseError::DuplicateQueryOption(option.to_string()));
    }

    *slot = Some(value);
    Ok(())
}

fn normalize_query_option_name(name: &str) -> &str {
    name.strip_prefix('$').unwrap_or(name)
}

fn parse_resource_path(url: &Url) -> (ResourcePath, bool, bool, bool, bool) {
    let mut resource_path = ResourcePath {
        segments: Vec::new(),
    };
    let mut each = false;
    let mut count = false;
    let mut r#ref = false;
    let mut value = false;

    for segment in url.path_segments().into_iter().flatten() {
        match segment {
            "$each" => each = true,
            "$count" => count = true,
            "$ref" => r#ref = true,
            "$value" => value = true,
            _ => resource_path.segments.push(segment.to_owned()),
        }
    }

    (resource_path, each, count, r#ref, value)
}

fn parse_boolean(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ExpandClause, FilterClause, ODataQuery, OrderByClause, ParseError, ResourcePath,
        SelectClause,
    };

    #[test]
    fn parses_typed_query_options() {
        let query = ODataQuery::parse(
            "https://example.test/odata/People(1)/Orders?$select=Id,Name&$filter=Rating%20gt%205&$expand=Items,Customer&$top=10&$skip=3&$orderby=Name%20desc&$count=true&x-custom=abc#anchor",
        )
        .expect("query should parse");

        assert_eq!(
            query.resource_path,
            ResourcePath {
                segments: vec![
                    "odata".to_string(),
                    "People(1)".to_string(),
                    "Orders".to_string()
                ],
            }
        );
        assert!(!query.each);
        assert!(!query.count);
        assert!(!query.r#ref);
        assert!(!query.value);
        assert_eq!(
            query.select,
            Some(SelectClause {
                items: vec!["Id".to_string(), "Name".to_string()],
            })
        );
        assert_eq!(
            query.filter,
            Some(FilterClause {
                expression: "Rating gt 5".to_string(),
            })
        );
        assert_eq!(
            query.expand,
            Some(ExpandClause {
                items: vec!["Items".to_string(), "Customer".to_string()],
            })
        );
        assert_eq!(query.top, Some(10));
        assert_eq!(query.skip, Some(3));
        assert_eq!(
            query.orderby,
            Some(OrderByClause {
                expression: "Name desc".to_string(),
            })
        );
        assert_eq!(query.inlinecount, Some(true));
        assert_eq!(
            query.custom.get("x-custom").cloned(),
            Some(vec!["abc".to_string()])
        );
        assert_eq!(query.fragment.as_deref(), Some("anchor"));
    }

    #[test]
    fn parses_path_markers_as_flags() {
        let query = ODataQuery::parse("https://example.test/Customers/$count/$ref/$value/$each")
            .expect("query should parse");

        assert_eq!(query.resource_path.segments, vec!["Customers"]);
        assert!(query.count);
        assert!(query.r#ref);
        assert!(query.value);
        assert!(query.each);
    }

    #[test]
    fn rejects_invalid_urls() {
        let error = ODataQuery::parse("not a url").expect_err("invalid url should fail");

        assert!(matches!(error, ParseError::InvalidUrl(_)));
    }

    #[test]
    fn rejects_invalid_boolean_and_duplicates() {
        let boolean_error = ODataQuery::parse("https://example.test/Customers?$count=maybe")
            .expect_err("invalid boolean should fail");
        assert!(matches!(boolean_error, ParseError::InvalidBoolean { .. }));

        let duplicate_error = ODataQuery::parse("https://example.test/Customers?$top=1&$top=2")
            .expect_err("duplicate option should fail");
        assert!(matches!(duplicate_error, ParseError::DuplicateQueryOption(name) if name == "top"));
    }
}
