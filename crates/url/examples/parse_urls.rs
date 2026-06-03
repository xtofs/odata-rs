use odata_url::ODataQuery;

fn main() {
    let examples = [
        "https://example.test/odata/People(1)/Orders?$select=Id,Name&$filter=Rating%20gt%205&$expand=Items,Customer&$top=10&$skip=3&$orderby=Name%20desc&$count=true",
        "https://example.test/Customers/$count/$ref/$value/$each",
        "https://example.test/Customers?$filter=contains(tolower(Name),'acme')%20or%20Orders/anyCount%20gt%200",
        "https://example.test/Customers?$filter=(Rating%20gt%205)%20and%20Name%20eq%20'Bob'&x-tenant=west",
    ];

    for (index, input) in examples.iter().enumerate() {
        println!("Example #{}", index + 1);
        println!("  URL: {}", input);

        match ODataQuery::parse(input) {
            Ok(query) => {
                println!("  Path segments: {:?}", query.resource_path.segments);
                println!(
                    "  Path markers: each={} count={} ref={} value={}",
                    query.each, query.count, query.r#ref, query.value
                );
                println!("  Select: {:?}", query.select);
                println!("  Filter: {:?}", query.filter);
                if let Some(f) = query.filter {
                    println!("  Filter: {}", f);
                }
                println!("  Expand: {:?}", query.expand);
                println!("  Top/Skip: {:?}/{:?}", query.page.top, query.page.skip);
                println!("  OrderBy: {:?}", query.orderby);
                println!("  Inline count: {:?}", query.inlinecount);
                println!("  Custom: {:?}", query.custom);
            }
            Err(error) => {
                println!("  Parse error: {}", error);
            }
        }

        println!();
    }
}
