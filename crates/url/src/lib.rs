use std::{collections::BTreeMap, fmt::Debug};

use url::Url;

/// System query options extracted from an OData request URL.
///
/// This is the subset of [`ODataQuery`] that a request handler actually needs:
/// it omits the resource path, full URL, and path markers (which are already
/// implied by the route a handler is bound to).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryOptions {
    pub select: Option<SelectClause>,
    pub filter: Option<FilterClause>,
    pub expand: Option<ExpandClause>,
    pub top: Option<u64>,
    pub skip: Option<u64>,
    pub orderby: Option<OrderByClause>,
    pub count: Option<bool>,
    pub custom: BTreeMap<String, Vec<String>>,
}

impl QueryOptions {
    /// Parse the `?...` part of an OData URL (without the leading `?`).
    ///
    /// An empty string yields `QueryOptions::default()`.
    pub fn parse(query: &str) -> Result<Self, ParseError> {
        let pairs = url::form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()));
        parse_query_options(pairs)
    }
}

impl From<ODataQuery> for QueryOptions {
    fn from(q: ODataQuery) -> Self {
        Self {
            select: q.select,
            filter: q.filter,
            expand: q.expand,
            top: q.top,
            skip: q.skip,
            orderby: q.orderby,
            count: q.inlinecount,
            custom: q.custom,
        }
    }
}

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
    pub expression: FilterExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterExpression {
    pub kind: FilterExpressionKind,
    /// Byte span in the decoded `$filter` expression string.
    pub span: FilterSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterExpressionKind {
    Literal(FilterLiteral),
    Member(FilterMemberPath),
    FunctionCall(FilterFunctionCall),
    Unary {
        operator: FilterUnaryOperator,
        operand: Box<FilterExpression>,
    },
    Binary {
        left: Box<FilterExpression>,
        operator: FilterBinaryOperator,
        right: Box<FilterExpression>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FilterSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterLiteral {
    Null,
    Boolean(bool),
    Number(String),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterMemberPath {
    pub segments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterFunctionCall {
    pub name: String,
    pub arguments: Vec<FilterExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterUnaryOperator {
    Not,
    Negate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterBinaryOperator {
    Or,
    And,
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
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
    #[error("invalid filter expression at position {position}: {message}")]
    InvalidFilterExpression {
        value: String,
        position: usize,
        message: String,
    },
}

impl ODataQuery {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let url = Url::parse(input).map_err(|error| ParseError::InvalidUrl(error.to_string()))?;
        Self::from_url(url)
    }

    pub fn from_url(url: Url) -> Result<Self, ParseError> {
        let (resource_path, each, count, r#ref, value) = parse_resource_path(&url);

        let pairs = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()));
        let options = parse_query_options(pairs)?;

        Ok(Self {
            each,
            custom: options.custom,
            count,
            expand: options.expand,
            filter: options.filter,
            fragment: url.fragment().map(ToOwned::to_owned),
            inlinecount: options.count,
            orderby: options.orderby,
            resource_path,
            r#ref,
            select: options.select,
            skip: options.skip,
            top: options.top,
            value,
            url,
        })
    }
}

fn parse_query_options<I>(pairs: I) -> Result<QueryOptions, ParseError>
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut out = QueryOptions::default();

    for (name, option_value) in pairs {
        let option_name = normalize_query_option_name(&name);

        match option_name {
            "select" => {
                assign_once(&mut out.select, "select", SelectClause::parse(&option_value)?)?;
            }
            "filter" => {
                assign_once(&mut out.filter, "filter", FilterClause::parse(&option_value)?)?;
            }
            "expand" => {
                assign_once(&mut out.expand, "expand", ExpandClause::parse(&option_value)?)?;
            }
            "top" => {
                assign_once(
                    &mut out.top,
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
                    &mut out.skip,
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
                    &mut out.orderby,
                    "orderby",
                    OrderByClause {
                        expression: option_value,
                    },
                )?;
            }
            "count" => {
                assign_once(
                    &mut out.count,
                    "inlinecount",
                    parse_boolean(&option_value).ok_or_else(|| ParseError::InvalidBoolean {
                        option: "inlinecount".to_string(),
                        value: option_value.clone(),
                    })?,
                )?;
            }
            _ => {
                out.custom.entry(name).or_default().push(option_value);
            }
        }
    }

    Ok(out)
}

impl SelectClause {
    fn parse(value: &str) -> Result<Self, ParseError> {
        Ok(Self {
            items: split_list(value),
        })
    }
}

impl FilterClause {
    fn parse(value: &str) -> Result<Self, ParseError> {
        let mut parser = FilterParser::new(value);
        let expression = parser.parse_expression()?;
        parser.consume_whitespace();

        if !parser.is_eof() {
            return Err(ParseError::InvalidFilterExpression {
                value: value.to_string(),
                position: parser.position(),
                message: "unexpected trailing input".to_string(),
            });
        }

        Ok(Self { expression })
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

struct FilterParser<'a> {
    input: &'a str,
    index: usize,
}

impl<'a> FilterParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, index: 0 }
    }

    fn position(&self) -> usize {
        self.index
    }

    fn is_eof(&self) -> bool {
        self.index >= self.input.len()
    }

    fn current(&self) -> &'a str {
        &self.input[self.index..]
    }

    fn consume_whitespace(&mut self) {
        while let Some(ch) = self.current().chars().next() {
            if ch.is_whitespace() {
                self.index += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn parse_expression(&mut self) -> Result<FilterExpression, ParseError> {
        self.parse_or_expression()
    }

    fn parse_or_expression(&mut self) -> Result<FilterExpression, ParseError> {
        let mut left = self.parse_and_expression()?;

        loop {
            self.consume_whitespace();
            if !self.consume_keyword("or") {
                break;
            }

            let right = self.parse_and_expression()?;
            let span = FilterSpan {
                start: left.span.start,
                end: right.span.end,
            };
            left = FilterExpression {
                kind: FilterExpressionKind::Binary {
                    left: Box::new(left),
                    operator: FilterBinaryOperator::Or,
                    right: Box::new(right),
                },
                span,
            };
        }

        Ok(left)
    }

    fn parse_and_expression(&mut self) -> Result<FilterExpression, ParseError> {
        let mut left = self.parse_comparison_expression()?;

        loop {
            self.consume_whitespace();
            if !self.consume_keyword("and") {
                break;
            }

            let right = self.parse_comparison_expression()?;
            let span = FilterSpan {
                start: left.span.start,
                end: right.span.end,
            };
            left = FilterExpression {
                kind: FilterExpressionKind::Binary {
                    left: Box::new(left),
                    operator: FilterBinaryOperator::And,
                    right: Box::new(right),
                },
                span,
            };
        }

        Ok(left)
    }

    fn parse_comparison_expression(&mut self) -> Result<FilterExpression, ParseError> {
        let mut left = self.parse_additive_expression()?;

        loop {
            self.consume_whitespace();
            let operator = if self.consume_keyword("eq") {
                Some(FilterBinaryOperator::Equal)
            } else if self.consume_keyword("ne") {
                Some(FilterBinaryOperator::NotEqual)
            } else if self.consume_keyword("gt") {
                Some(FilterBinaryOperator::GreaterThan)
            } else if self.consume_keyword("ge") {
                Some(FilterBinaryOperator::GreaterThanOrEqual)
            } else if self.consume_keyword("lt") {
                Some(FilterBinaryOperator::LessThan)
            } else if self.consume_keyword("le") {
                Some(FilterBinaryOperator::LessThanOrEqual)
            } else {
                None
            };

            let Some(operator) = operator else {
                break;
            };

            let right = self.parse_additive_expression()?;
            let span = FilterSpan {
                start: left.span.start,
                end: right.span.end,
            };
            left = FilterExpression {
                kind: FilterExpressionKind::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                },
                span,
            };
        }

        Ok(left)
    }

    fn parse_additive_expression(&mut self) -> Result<FilterExpression, ParseError> {
        let mut left = self.parse_multiplicative_expression()?;

        loop {
            self.consume_whitespace();
            let operator = if self.consume_keyword("add") {
                Some(FilterBinaryOperator::Add)
            } else if self.consume_keyword("sub") {
                Some(FilterBinaryOperator::Subtract)
            } else {
                None
            };

            let Some(operator) = operator else {
                break;
            };

            let right = self.parse_multiplicative_expression()?;
            let span = FilterSpan {
                start: left.span.start,
                end: right.span.end,
            };
            left = FilterExpression {
                kind: FilterExpressionKind::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                },
                span,
            };
        }

        Ok(left)
    }

    fn parse_multiplicative_expression(&mut self) -> Result<FilterExpression, ParseError> {
        let mut left = self.parse_unary_expression()?;

        loop {
            self.consume_whitespace();
            let operator = if self.consume_keyword("mul") {
                Some(FilterBinaryOperator::Multiply)
            } else if self.consume_keyword("div") {
                Some(FilterBinaryOperator::Divide)
            } else if self.consume_keyword("mod") {
                Some(FilterBinaryOperator::Modulo)
            } else {
                None
            };

            let Some(operator) = operator else {
                break;
            };

            let right = self.parse_unary_expression()?;
            let span = FilterSpan {
                start: left.span.start,
                end: right.span.end,
            };
            left = FilterExpression {
                kind: FilterExpressionKind::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                },
                span,
            };
        }

        Ok(left)
    }

    fn parse_unary_expression(&mut self) -> Result<FilterExpression, ParseError> {
        self.consume_whitespace();
        let start = self.position();

        if self.consume_keyword("not") {
            let operand = self.parse_unary_expression()?;
            return Ok(FilterExpression {
                span: FilterSpan {
                    start,
                    end: operand.span.end,
                },
                kind: FilterExpressionKind::Unary {
                    operator: FilterUnaryOperator::Not,
                    operand: Box::new(operand),
                },
            });
        }

        if self.consume_char('-') {
            let operand = self.parse_unary_expression()?;
            return Ok(FilterExpression {
                span: FilterSpan {
                    start,
                    end: operand.span.end,
                },
                kind: FilterExpressionKind::Unary {
                    operator: FilterUnaryOperator::Negate,
                    operand: Box::new(operand),
                },
            });
        }

        self.parse_primary_expression()
    }

    fn parse_primary_expression(&mut self) -> Result<FilterExpression, ParseError> {
        self.consume_whitespace();
        let start = self.position();

        if self.consume_char('(') {
            let mut expression = self.parse_expression()?;
            self.consume_whitespace();
            if !self.consume_char(')') {
                return self.error("expected ')' to close grouped expression");
            }
            expression.span = FilterSpan {
                start,
                end: self.position(),
            };
            return Ok(expression);
        }

        if let Some(text) = self.consume_string_literal() {
            return Ok(FilterExpression {
                kind: FilterExpressionKind::Literal(FilterLiteral::String(text)),
                span: FilterSpan {
                    start,
                    end: self.position(),
                },
            });
        }

        if let Some(number) = self.consume_number_literal() {
            return Ok(FilterExpression {
                kind: FilterExpressionKind::Literal(FilterLiteral::Number(number)),
                span: FilterSpan {
                    start,
                    end: self.position(),
                },
            });
        }

        if let Some(identifier) = self.consume_identifier() {
            if identifier.eq_ignore_ascii_case("true") {
                return Ok(FilterExpression {
                    kind: FilterExpressionKind::Literal(FilterLiteral::Boolean(true)),
                    span: FilterSpan {
                        start,
                        end: self.position(),
                    },
                });
            }

            if identifier.eq_ignore_ascii_case("false") {
                return Ok(FilterExpression {
                    kind: FilterExpressionKind::Literal(FilterLiteral::Boolean(false)),
                    span: FilterSpan {
                        start,
                        end: self.position(),
                    },
                });
            }

            if identifier.eq_ignore_ascii_case("null") {
                return Ok(FilterExpression {
                    kind: FilterExpressionKind::Literal(FilterLiteral::Null),
                    span: FilterSpan {
                        start,
                        end: self.position(),
                    },
                });
            }

            self.consume_whitespace();
            if self.consume_char('(') {
                let mut arguments = Vec::new();

                self.consume_whitespace();
                if !self.consume_char(')') {
                    loop {
                        let argument = self.parse_expression()?;
                        arguments.push(argument);

                        self.consume_whitespace();
                        if self.consume_char(')') {
                            break;
                        }

                        if !self.consume_char(',') {
                            return self.error("expected ',' or ')' in function call");
                        }
                    }
                }

                return Ok(FilterExpression {
                    kind: FilterExpressionKind::FunctionCall(FilterFunctionCall {
                        name: identifier,
                        arguments,
                    }),
                    span: FilterSpan {
                        start,
                        end: self.position(),
                    },
                });
            }

            return Ok(FilterExpression {
                kind: FilterExpressionKind::Member(FilterMemberPath {
                    segments: identifier
                        .split('/')
                        .filter(|segment| !segment.is_empty())
                        .map(ToOwned::to_owned)
                        .collect(),
                }),
                span: FilterSpan {
                    start,
                    end: self.position(),
                },
            });
        }

        self.error("expected filter expression")
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        let mut candidate = self.current().chars();

        for expected in keyword.chars() {
            let Some(actual) = candidate.next() else {
                return false;
            };

            if !actual.eq_ignore_ascii_case(&expected) {
                return false;
            }
        }

        if let Some(next) = candidate.next() {
            if is_identifier_char(next) {
                return false;
            }
        }

        self.index += keyword.len();
        true
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.current().starts_with(expected) {
            self.index += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn consume_identifier(&mut self) -> Option<String> {
        let mut characters = self.current().char_indices();
        let (_, first) = characters.next()?;
        if !is_identifier_start(first) {
            return None;
        }

        let mut end = first.len_utf8();
        for (offset, ch) in characters {
            if is_identifier_char(ch) {
                end = offset + ch.len_utf8();
            } else {
                break;
            }
        }

        let identifier = &self.current()[..end];
        self.index += end;
        Some(identifier.to_string())
    }

    fn consume_string_literal(&mut self) -> Option<String> {
        if !self.current().starts_with('\'') {
            return None;
        }

        self.index += 1;
        let mut value = String::new();

        while let Some(ch) = self.current().chars().next() {
            self.index += ch.len_utf8();

            if ch == '\'' {
                if self.current().starts_with('\'') {
                    self.index += 1;
                    value.push('\'');
                    continue;
                }

                return Some(value);
            }

            value.push(ch);
        }

        None
    }

    fn consume_number_literal(&mut self) -> Option<String> {
        let mut seen_digit = false;
        let mut seen_dot = false;
        let mut end = 0usize;

        for (offset, ch) in self.current().char_indices() {
            if ch.is_ascii_digit() {
                seen_digit = true;
                end = offset + ch.len_utf8();
                continue;
            }

            if ch == '.' && !seen_dot {
                seen_dot = true;
                end = offset + ch.len_utf8();
                continue;
            }

            break;
        }

        if !seen_digit {
            return None;
        }

        let value = self.current()[..end].to_string();
        self.index += end;
        Some(value)
    }

    fn error<T>(&self, message: &str) -> Result<T, ParseError> {
        Err(ParseError::InvalidFilterExpression {
            value: self.input.to_string(),
            position: self.index,
            message: message.to_string(),
        })
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '$'
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '/' | '.')
}

#[cfg(test)]
mod tests;

mod display;
