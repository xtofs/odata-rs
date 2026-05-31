# Filter Lambda Support (`any` / `all`)

## Goal

Implement lambda operator parsing for `$filter` according to OData ABNF so nested collection predicates become part of the structured filter AST.

## Scope

- Parse member path lambda forms like `Orders/any(o: o/Amount gt 10)` and `Orders/all(o: o/Amount gt 0)`.
- Add AST nodes for lambda operators and range variables.
- Handle empty-lambda forms like `Orders/any()` where applicable.
- Add parser diagnostics for malformed lambda syntax (missing variable, missing `:`, missing body).
- Add unit tests for nested lambda expressions and precedence interactions.

## Notes

Keep spans attached to lambda nodes and lambda bodies so downstream translation and diagnostics can highlight precise source ranges.
