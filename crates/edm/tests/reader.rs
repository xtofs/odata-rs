//! Data-driven tests for [`odata_edm::reader::CsdlReader`].
//!
//! Each `reader_case!` invocation expands to a `#[test]` so failures are
//! reported per-case in `cargo test` output. Add new edge cases by adding
//! more `reader_case!` calls below — no other plumbing needed.
//!
//! The expected slice is the sequence of formatted tokens produced by
//! [`fmt_token`]. The format intentionally distinguishes:
//!   - `Start(<Element> attr=value, attr=value)` — opening of a CSDL **element**
//!   - `End(<Element>)`                          — closing of a CSDL element
//!   - `Expr(<debug-of-CsdlAnnotationExpression>)` — one annotation expression
//!
//! This makes the element-vs-expression distinction immediately visible in
//! every assertion.

use std::io::BufRead;

use odata_edm::reader::{CsdlReader, CsdlToken};

fn fmt_token(t: CsdlToken<'_>) -> String {
    match t {
        CsdlToken::StartCsdlElement { name, attributes } => {
            let attr_str = attributes
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(",");
            if attr_str.is_empty() {
                format!("Start({name})")
            } else {
                format!("Start({name} {attr_str})")
            }
        }
        CsdlToken::EndCsdlElement { name } => format!("End({name})"),
        CsdlToken::AnnotationExpression(e) => format!("Expr({e:?})"),
    }
}

fn collect<R: BufRead>(reader: &mut CsdlReader<R>) -> Vec<String> {
    let mut out = Vec::new();
    while let Some(t) = reader.next_token().expect("reader error") {
        out.push(fmt_token(t));
    }
    out
}

macro_rules! reader_case {
    ($name:ident, $xml:expr, [$($expected:expr),* $(,)?]) => {
        #[test]
        fn $name() {
            let mut r = CsdlReader::from_reader($xml.as_bytes());
            let got = collect(&mut r);
            let expected: Vec<&str> = vec![$($expected),*];
            assert_eq!(got, expected);
        }
    };
}

// ----- Plain element pass-through ------------------------------------------

reader_case!(
    basic_elements,
    r#"<Schema Namespace="N"><EntityType Name="T"/></Schema>"#,
    [
        "Start(Schema Namespace=N)",
        "Start(EntityType Name=T)",
        "End(EntityType)",
        "End(Schema)",
    ]
);

reader_case!(
    nested_elements_pass_through,
    r#"<Schema Namespace="N"><EntityType Name="T"><Property Name="P" Type="Edm.String"/></EntityType></Schema>"#,
    [
        "Start(Schema Namespace=N)",
        "Start(EntityType Name=T)",
        "Start(Property Name=P,Type=Edm.String)",
        "End(Property)",
        "End(EntityType)",
        "End(Schema)",
    ]
);

// ----- Inline-attribute annotation expression normalization ----------------

reader_case!(
    inline_annotation_string_empty_element,
    r#"<Annotation Term="Core.Description" String="hello"/>"#,
    [
        "Start(Annotation Term=Core.Description)",
        r#"Expr(String("hello"))"#,
        "End(Annotation)",
    ]
);

reader_case!(
    inline_annotation_bool_empty_element,
    r#"<Annotation Term="X" Bool="true"/>"#,
    [
        "Start(Annotation Term=X)",
        "Expr(Bool(true))",
        "End(Annotation)",
    ]
);

reader_case!(
    inline_annotation_int_empty_element,
    r#"<Annotation Term="X" Int="42"/>"#,
    [
        "Start(Annotation Term=X)",
        "Expr(Int(42))",
        "End(Annotation)",
    ]
);

// ----- Nested annotation expression normalization --------------------------

reader_case!(
    nested_annotation_string,
    r#"<Annotation Term="Core.Description"><String>hello</String></Annotation>"#,
    [
        "Start(Annotation Term=Core.Description)",
        r#"Expr(String("hello"))"#,
        "End(Annotation)",
    ]
);

reader_case!(
    inline_and_nested_forms_produce_identical_token_streams_string,
    r#"<Annotation Term="X" String="hi"/>"#,
    [
        "Start(Annotation Term=X)",
        r#"Expr(String("hi"))"#,
        "End(Annotation)",
    ]
);

// Companion to the case above: same expected tokens, different XML source.
// This is the only case that asserts the EQUIVALENCE explicitly, so it lives
// as a normal #[test] rather than a reader_case! invocation.
#[test]
fn inline_and_nested_forms_match_explicit() {
    let inline = r#"<Annotation Term="X" String="hi"/>"#;
    let nested = r#"<Annotation Term="X"><String>hi</String></Annotation>"#;
    let mut a = CsdlReader::from_reader(inline.as_bytes());
    let mut b = CsdlReader::from_reader(nested.as_bytes());
    assert_eq!(collect(&mut a), collect(&mut b));
}

// ----- Dual form: inline attribute + nested element on same Annotation -----
// Per the spec the body of <Annotation> is one expression, but this XML is
// syntactically well-formed. The reader emits BOTH expressions; the semantic
// layer decides validity. See CSDL 14.2.

reader_case!(
    annotation_with_inline_attr_and_nested_body_emits_both,
    r#"<Annotation Term="X" String="a"><Int>7</Int></Annotation>"#,
    [
        "Start(Annotation Term=X)",
        r#"Expr(String("a"))"#,
        "Expr(Int(7))",
        "End(Annotation)",
    ]
);

reader_case!(
    annotation_with_multiple_nested_expressions_emits_all,
    r#"<Annotation Term="X"><String>a</String><Int>7</Int></Annotation>"#,
    [
        "Start(Annotation Term=X)",
        r#"Expr(String("a"))"#,
        "Expr(Int(7))",
        "End(Annotation)",
    ]
);

// ----- Empty annotation (marker; no expression) ---------------------------

reader_case!(
    empty_annotation_self_closing_is_marker_only,
    r#"<Annotation Term="T"/>"#,
    ["Start(Annotation Term=T)", "End(Annotation)"]
);

reader_case!(
    empty_annotation_explicit_close_is_marker_only,
    r#"<Annotation Term="T"></Annotation>"#,
    ["Start(Annotation Term=T)", "End(Annotation)"]
);

// ----- Qualifier attribute is preserved on Start ---------------------------

reader_case!(
    annotation_qualifier_is_preserved,
    r#"<Annotation Term="UI.Importance" Qualifier="Compact" EnumMember="UI.ImportanceType/High"/>"#,
    [
        "Start(Annotation Term=UI.Importance,Qualifier=Compact)",
        r#"Expr(EnumMember("UI.ImportanceType/High"))"#,
        "End(Annotation)",
    ]
);

// ----- Annotation placed on a CSDL element passes the element through ------

reader_case!(
    annotation_inside_entitytype,
    r#"<EntityType Name="T"><Annotation Term="Core.Description" String="desc"/></EntityType>"#,
    [
        "Start(EntityType Name=T)",
        "Start(Annotation Term=Core.Description)",
        r#"Expr(String("desc"))"#,
        "End(Annotation)",
        "End(EntityType)",
    ]
);

// ----- Path-family expressions --------------------------------------------

reader_case!(
    inline_property_path,
    r#"<Annotation Term="X" PropertyPath="Name"/>"#,
    [
        "Start(Annotation Term=X)",
        r#"Expr(PropertyPath("Name"))"#,
        "End(Annotation)",
    ]
);

reader_case!(
    nested_collection_of_property_paths,
    r#"<Annotation Term="UI.SelectionFields"><Collection><PropertyPath>Name</PropertyPath><PropertyPath>Total</PropertyPath></Collection></Annotation>"#,
    [
        "Start(Annotation Term=UI.SelectionFields)",
        r#"Expr(Collection([PropertyPath("Name"), PropertyPath("Total")]))"#,
        "End(Annotation)",
    ]
);

// ----- Record + PropertyValue (inline + nested forms within PropertyValue) -

reader_case!(
    record_with_mixed_propertyvalue_forms,
    r#"<Annotation Term="T"><Record Type="R"><PropertyValue Property="A" String="x"/><PropertyValue Property="B"><Int>42</Int></PropertyValue></Record></Annotation>"#,
    [
        "Start(Annotation Term=T)",
        "Expr(Record { type_: Some(\"R\"), properties: [PropertyValue { property: \"A\", value: Some(String(\"x\")), annotations: [] }, PropertyValue { property: \"B\", value: Some(Int(42)), annotations: [] }] })",
        "End(Annotation)",
    ]
);

// Dual form on a PropertyValue: inline attribute wins, nested expression(s)
// in the body are silently dropped at the reader's recursive-parser layer.
// See TODO/structured-warnings.md for surfacing this as a warning.
reader_case!(
    propertyvalue_with_inline_attr_and_nested_body_keeps_inline,
    r#"<Annotation Term="T"><Record><PropertyValue Property="P" String="a"><Int>7</Int></PropertyValue></Record></Annotation>"#,
    [
        "Start(Annotation Term=T)",
        "Expr(Record { type_: None, properties: [PropertyValue { property: \"P\", value: Some(String(\"a\")), annotations: [] }] })",
        "End(Annotation)",
    ]
);

// ----- If expression -------------------------------------------------------

reader_case!(
    nested_annotation_if_with_else,
    r#"<Annotation Term="T"><If><Path>p</Path><String>yes</String><String>no</String></If></Annotation>"#,
    [
        "Start(Annotation Term=T)",
        "Expr(If { test: Path(\"p\"), then_: String(\"yes\"), else_: Some(String(\"no\")) })",
        "End(Annotation)",
    ]
);

reader_case!(
    nested_annotation_if_without_else,
    r#"<Annotation Term="T"><If><Path>p</Path><String>yes</String></If></Annotation>"#,
    [
        "Start(Annotation Term=T)",
        "Expr(If { test: Path(\"p\"), then_: String(\"yes\"), else_: None })",
        "End(Annotation)",
    ]
);

// ----- Binary operators: all map to one BinaryExpression variant -----------

reader_case!(
    binary_eq,
    r#"<Annotation Term="T"><Eq><Path>a</Path><Int>1</Int></Eq></Annotation>"#,
    [
        "Start(Annotation Term=T)",
        "Expr(BinaryExpression { op: Eq, lhs: Path(\"a\"), rhs: Int(1) })",
        "End(Annotation)",
    ]
);

reader_case!(
    binary_and,
    r#"<Annotation Term="T"><And><Bool>true</Bool><Bool>false</Bool></And></Annotation>"#,
    [
        "Start(Annotation Term=T)",
        "Expr(BinaryExpression { op: And, lhs: Bool(true), rhs: Bool(false) })",
        "End(Annotation)",
    ]
);

reader_case!(
    binary_or_ne_gt_ge_lt_le_nested,
    // Builds: Or(Ne(a,1), And(Gt(b,Ge(c,2)), Lt(d,Le(e,3))))
    r#"<Annotation Term="T"><Or><Ne><Path>a</Path><Int>1</Int></Ne><And><Gt><Path>b</Path><Ge><Path>c</Path><Int>2</Int></Ge></Gt><Lt><Path>d</Path><Le><Path>e</Path><Int>3</Int></Le></Lt></And></Or></Annotation>"#,
    [
        "Start(Annotation Term=T)",
        "Expr(BinaryExpression { op: Or, lhs: BinaryExpression { op: Ne, lhs: Path(\"a\"), rhs: Int(1) }, rhs: BinaryExpression { op: And, lhs: BinaryExpression { op: Gt, lhs: Path(\"b\"), rhs: BinaryExpression { op: Ge, lhs: Path(\"c\"), rhs: Int(2) } }, rhs: BinaryExpression { op: Lt, lhs: Path(\"d\"), rhs: BinaryExpression { op: Le, lhs: Path(\"e\"), rhs: Int(3) } } } })",
        "End(Annotation)",
    ]
);

reader_case!(
    not_wraps_binary_expression,
    r#"<Annotation Term="T"><Not><Eq><Path>a</Path><Int>1</Int></Eq></Not></Annotation>"#,
    [
        "Start(Annotation Term=T)",
        "Expr(Not(BinaryExpression { op: Eq, lhs: Path(\"a\"), rhs: Int(1) }))",
        "End(Annotation)",
    ]
);

// ----- Source location tracking --------------------------------------------

#[test]
fn current_location_reports_line_and_column() {
    // After consuming `<Schema>\n    <EntityType Name="T"/>` the cursor is
    // on line 2 at the byte just after the `>` of the empty element.
    //   line 1: <Schema>             (consumed)
    //   line 2:     <EntityType Name="T"/>   ← cursor lands here, column 27
    let xml = "<Schema>\n    <EntityType Name=\"T\"/>\n</Schema>";
    let mut r = CsdlReader::from_reader(xml.as_bytes());
    let _ = r.next_token().unwrap().unwrap(); // <Schema>
    let _ = r.next_token().unwrap().unwrap(); // <EntityType .../>
    let loc = r.current_location();
    assert_eq!(loc.line, 2, "expected line 2, got {loc:?}");
    assert_eq!(loc.column, 27, "expected column 27, got {loc:?}");
}

#[test]
fn error_message_includes_line_and_column() {
    // <If> requires 2 or 3 children; this gives it only 1 — the error fires
    // on line 2 when the parser tries to validate the child count.
    let xml = "<Annotation Term=\"T\">\n  <If><Bool>true</Bool></If>\n</Annotation>";
    let mut r = CsdlReader::from_reader(xml.as_bytes());
    let err = loop {
        match r.next_token() {
            Ok(Some(_)) => continue,
            Ok(None) => panic!("expected a CSDL error before EOF"),
            Err(e) => break e,
        }
    };
    let msg = err.to_string();
    assert!(msg.contains("line 2"), "missing line in error: {msg}");
    assert!(msg.contains("column"), "missing column in error: {msg}");
    assert!(msg.contains("<If>"), "missing context in error: {msg}");
}
