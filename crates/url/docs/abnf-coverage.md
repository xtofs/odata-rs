# ABNF Coverage

| ABNF rule or concern                                         | Status                | Rust location                                                    |
| ------------------------------------------------------------ | --------------------- | ---------------------------------------------------------------- |
| `query`                                                      | handled by dependency | `url` crate boundary                                             |
| `fragment`                                                   | handled by dependency | `url` crate boundary                                             |
| `pchar` / `pct-encoded` / `unreserved`                       | handled by dependency | `url` crate boundary                                             |
| path segmentation                                            | handled by dependency | `url` crate boundary                                             |
| percent-encoding normalization                               | handled by dependency | `url` crate boundary                                             |
| `resourcePath`                                               | partially implemented | `crates/url/src/lib.rs::ResourcePath` and `ODataQuery::from_url` |
| `each` / `count` / `ref` / `value` path markers              | implemented           | `crates/url/src/lib.rs::ODataQuery` and `parse_resource_path`    |
| `$select` / `select`                                         | partially implemented | `crates/url/src/lib.rs::SelectClause::parse`                     |
| `$filter` / `filter`                                         | partially implemented | `crates/url/src/lib.rs::FilterClause`                            |
| `$expand` / `expand`                                         | partially implemented | `crates/url/src/lib.rs::ExpandClause::parse`                     |
| `$top` / `top`                                               | implemented           | `crates/url/src/lib.rs::ODataQuery::from_url`                    |
| `$skip` / `skip`                                             | implemented           | `crates/url/src/lib.rs::ODataQuery::from_url`                    |
| `$orderby` / `orderby`                                       | partially implemented | `crates/url/src/lib.rs::OrderByClause`                           |
| `$count` query option (`inlinecount`)                        | implemented           | `crates/url/src/lib.rs::ODataQuery::from_url`                    |
| `customQueryOption`                                          | implemented           | `crates/url/src/lib.rs::ODataQuery::from_url`                    |
| key predicates, navigation, bound operations, function calls | deferred              | not yet projected into `ODataQuery`                              |
| lambda operators (`any`, `all`)                              | deferred              | not implemented                                                  |
| `$search`                                                    | deferred              | not implemented                                                  |
| `$compute`                                                   | deferred              | not implemented                                                  |

Notes:

- `handled by dependency` means the generic URL parser owns that boundary.
- `partially implemented` means the value is present in `ODataQuery`, but its internal OData grammar is still coarse.
- `deferred` means the rule is not represented in `ODataQuery` yet and does not have a dedicated parser path.
- The parser rejects duplicate well-known options and invalid numeric or boolean values with `ParseError`.
