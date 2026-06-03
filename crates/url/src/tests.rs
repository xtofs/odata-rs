use super::{
    ExpandClause, FilterBinaryOperator, FilterClause, FilterExpression, FilterExpressionKind,
    FilterFunctionCall, FilterLiteral, FilterMemberPath, ODataQuery, OrderByClause, ParseError,
    ResourcePath, SelectClause,
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
            expression: FilterExpression {
                kind: FilterExpressionKind::Binary {
                    left: Box::new(FilterExpression {
                        kind: FilterExpressionKind::Member(FilterMemberPath {
                            segments: vec!["Rating".to_string()],
                        }),
                        span: super::FilterSpan { start: 0, end: 7 },
                    }),
                    operator: FilterBinaryOperator::GreaterThan,
                    right: Box::new(FilterExpression {
                        kind: FilterExpressionKind::Literal(
                            FilterLiteral::Number("5".to_string(),)
                        ),
                        span: super::FilterSpan { start: 10, end: 11 },
                    }),
                },
                span: super::FilterSpan { start: 0, end: 11 },
            },
        })
    );
    assert_eq!(
        query.expand,
        Some(ExpandClause {
            items: vec!["Items".to_string(), "Customer".to_string()],
        })
    );
    assert_eq!(query.page.top, Some(10));
    assert_eq!(query.page.skip, Some(3));
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

#[test]
fn parses_filter_common_expr_with_precedence() {
    let query = ODataQuery::parse(
        "https://example.test/Customers?$filter=Price%20add%202%20mul%203%20gt%2010%20and%20not%20(Discontinued%20eq%20true)",
    )
    .expect("query should parse");

    let filter = query.filter.expect("filter should be present");
    assert!(matches!(
        filter.expression.kind,
        FilterExpressionKind::Binary {
            operator: FilterBinaryOperator::And,
            ..
        }
    ));
    assert_eq!(filter.expression.span.start, 0);
    assert_eq!(
        filter.expression.span.end,
        "Price add 2 mul 3 gt 10 and not (Discontinued eq true)".len()
    );
}

#[test]
fn parses_filter_function_calls_and_paths() {
    let query = ODataQuery::parse(
        "https://example.test/Customers?$filter=contains(tolower(Name),'acme')%20or%20Orders/anyCount%20gt%200",
    )
    .expect("query should parse");

    let filter = query.filter.expect("filter should be present");
    let FilterExpressionKind::Binary {
        left,
        operator,
        right,
    } = filter.expression.kind
    else {
        panic!("expected binary expression");
    };

    assert_eq!(operator, FilterBinaryOperator::Or);
    assert!(matches!(
        left.kind,
        FilterExpressionKind::FunctionCall(FilterFunctionCall { .. })
    ));
    assert!(matches!(
        right.kind,
        FilterExpressionKind::Binary {
            left,
            operator: FilterBinaryOperator::GreaterThan,
            right,
        } if matches!(left.kind, FilterExpressionKind::Member(FilterMemberPath { .. }))
            && matches!(right.kind, FilterExpressionKind::Literal(FilterLiteral::Number(_)))
    ));
}

#[test]
fn tracks_filter_node_spans() {
    let query = ODataQuery::parse(
        "https://example.test/Customers?$filter=(Rating%20gt%205)%20and%20Name%20eq%20'Bob'",
    )
    .expect("query should parse");

    let filter = query.filter.expect("filter should be present");
    assert_eq!(filter.expression.span.start, 0);
    assert_eq!(
        filter.expression.span.end,
        "(Rating gt 5) and Name eq 'Bob'".len()
    );

    let FilterExpressionKind::Binary { left, right, .. } = filter.expression.kind else {
        panic!("expected binary expression");
    };

    assert_eq!(left.span.start, 0);
    assert_eq!(left.span.end, "(Rating gt 5)".len());
    assert_eq!(right.span.start, "(Rating gt 5) and ".len());
}

#[test]
fn rejects_invalid_filter_expressions() {
    let error = ODataQuery::parse("https://example.test/Customers?$filter=Price%20gt")
        .expect_err("incomplete expression should fail");

    assert!(matches!(error, ParseError::InvalidFilterExpression { .. }));
}

#[test]
fn display_keeps_and_or_precedence_with_parentheses() {
    let query = ODataQuery::parse(
        "https://example.test/Customers?$filter=(A%20eq%201%20or%20B%20eq%202)%20and%20C%20eq%203",
    )
    .expect("query should parse");

    let display = query.filter.expect("filter should be present").to_string();

    assert_eq!(display, "(A eq 1 or B eq 2) and C eq 3");
}

#[test]
fn display_formats_unary_not_as_keyword() {
    let query = ODataQuery::parse("https://example.test/Customers?$filter=not%20(A%20eq%201)")
        .expect("query should parse");

    let display = query.filter.expect("filter should be present").to_string();

    assert_eq!(display, "not (A eq 1)");
}

#[test]
fn display_escapes_single_quotes_in_string_literals() {
    let query =
        ODataQuery::parse("https://example.test/Customers?$filter=Name%20eq%20%27O%27%27Brien%27")
            .expect("query should parse");

    let display = query.filter.expect("filter should be present").to_string();

    assert_eq!(display, "Name eq 'O''Brien'");
}
