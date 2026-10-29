# Schema migrations

Forward-only `.surql` files, named `NNNN_snake_case_name.surql`, embedded in the binary by
`src/migrations.rs` (persistence.md § Migrations). Never edit a file once it is merged: startup
refuses a database whose applied migration checksum differs. Add a new file instead, list it in
`MIGRATIONS`, and update `tests/fixtures/schema.snapshot` with
`UPDATE_SCHEMA_SNAPSHOT=1 cargo test -p health-surrealdb --test migrations`.

CI also runs `surreal validate` and `surql-fmt --check` over this folder.
