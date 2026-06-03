## odata-rs

odata-rs is an early-stage Rust workspace for building OData services with a clear crate split: `odata-rs-edm` for EDM/CSDL modeling, `odata-rs-url` for typed OData URL parsing, and `odata-rs-service` for service-side execution contracts, with the root `odata-rs` crate acting as a feature-gated facade over those modules while the architecture is refined.

```sh
cargo test --workspace
```

```sh
cargo run -p odata-rs-url --example parse_urls
```

```sh
cargo run -p odata-rs-edm --example csdl_to_model
```

```sh
cargo run -p odata-rs-service --example rooms --features="sqlx-sqlite"
```

```sh
RUST_LOG=tower_http=trace cargo run --example rooms -p odata-rs-service --features sqlx-sqlite
```
