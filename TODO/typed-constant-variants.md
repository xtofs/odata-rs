# Typed variants for CSDL constant expressions

`CsdlExpression` currently stores several constants as `String` because std has no lossless native type for them:

- `Date(String)` — ISO 8601 date
- `DateTimeOffset(String)`
- `Decimal(String)` — arbitrary precision
- `Duration(String)` — ISO 8601 duration
- `Guid(String)`
- `TimeOfDay(String)`

Adopting typed representations would require external crates:

- `chrono` or `time` for Date / DateTimeOffset / TimeOfDay / Duration
- `rust_decimal` for Decimal
- `uuid` for Guid

**Decision so far**: keep them as `String` for now. The reader should not pull in these dependencies before the parser is functional end-to-end. Revisit once `EdmModel` construction is in place and we have a clearer picture of downstream consumers' needs.

**When to revisit**: when a consuming crate (query engine, serializer) needs to *compute* on these values rather than just round-trip them, the string form will become painful and we should introduce typed variants then.
