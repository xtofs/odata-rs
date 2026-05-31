use odata_url::ODataQuery;

#[test]
fn parses_well_known_query_options_as_fields() {
    let query = ODataQuery::parse(
        "https://example.test/Customers/$count?$expand=Orders,Trips&$select=Id,Name&$count=false",
    )
    .expect("query should parse");

    assert_eq!(query.resource_path.segments, vec!["Customers"]);
    assert!(query.count);
    assert_eq!(
        query.expand.expect("expand should be present").items,
        vec!["Orders", "Trips"]
    );
    assert_eq!(
        query.select.expect("select should be present").items,
        vec!["Id", "Name"]
    );
    assert_eq!(query.inlinecount, Some(false));
}
