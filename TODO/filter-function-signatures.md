# Filter Function Signature Validation

## Goal

Validate OData filter function calls against known signatures so the parser can reject unsupported functions and wrong argument arity early.

## Scope

- Add function metadata for supported built-in functions (name, min/max arity, rough argument categories).
- Validate function call names and argument counts during parsing.
- Return targeted parse errors for unknown functions and invalid arity.
- Add tests for valid calls, unknown function names, and argument count mismatches.

## Notes

Start with arity validation first; strict type-category validation can be added incrementally after core signatures are established.
