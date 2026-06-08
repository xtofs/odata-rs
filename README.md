## odata-rs

odata-rs is an early-stage Rust workspace for building OData services with a clear crate split: `csdl-edm` for CSDL parsing / EDM modeling / resolution / validation, `odata-rs-url` for typed OData URL parsing, and `odata-rs-service` for service-side execution contracts, with the root `odata-rs` crate acting as a feature-gated facade over those modules while the architecture is refined.

```sh
cargo test --workspace
```

```sh
cargo run -p odata-rs-url --example parse_urls
```

```sh
cargo run -p csdl-edm --example parse_resolve_validate
```

```sh
cargo run --example rooms --features sqlx-sqlite
```

run the rooms end-to-end HTTP scenario with Hurl (starts/stops service automatically)

```powershell
pwsh ./scripts/test-rooms-hurl.ps1
```

```bash
bash ./scripts/test-rooms-hurl.sh
```

show HTTP trace messages

```sh
RUST_LOG=tower_http=trace cargo run --example rooms
```

show HTTP and SQl trace messages

```sh
RUST_LOG=tower_http=trace,sqlx=trace cargo run --example rooms
```
