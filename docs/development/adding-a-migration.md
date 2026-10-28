# Adding a migration

> Status: planned layout. No code exists yet; the migrations module of `health-surrealdb` and its directory are created by the scaffold.

## When to use

Any change to the SurrealDB schema (tables, fields, indexes, guard records), or a rewrite of stored resources.

## Steps

1. **Schema change:** write a new, versioned `.surql` file. Migrations are **forward-only**: never edit or delete one that has been applied; startup fails if an applied migration's checksum changed.
2. Follow the table shapes in [persistence.md](../architecture/persistence.md): `SCHEMAFULL` head tables, `<type>_history` keyed `[resource_uuid, version_seq]`, array-keyed `idx_*` tables, `record<…>` references (never `set<record>`, SurrealDB #7621).
3. **Data change** (rewriting resources): write it in Rust, as a data migration, not in SurrealQL.
4. **Reindexing is never a migration.** A new or changed SearchParameter starts the resumable background reindex job instead.
5. Validate with `surreal validate` and format with `surql-fmt` (CI runs `surql-fmt --check`).
6. Update the schema snapshot that CI compares with `INFO FOR DB` after migrating a fresh database.
7. Extend the export → import round-trip fixture so it covers the new schema. A release is blocked if any migration lacks this test.
8. Remember where it runs: at startup on every node before `/health/ready`, inside the edge and desktop processes (with extended service-manager timeouts), after a mandatory pre-update backup.

## Expected files

- `crates/infrastructure/health-surrealdb/<migrations dir>/<version>_<name>.surql` (directory and naming to be fixed by the scaffold; files are embedded in the binary)
- Rust data migrations in the same module
- The schema snapshot file used by `integration-tests.yml`
- Round-trip fixture data

## Contracts

- `_migration` table: applied versions with checksums.
- The table → Owner mapping (each new table belongs to exactly one Owner's repository, or to `health-surrealdb`'s shared infrastructure tables).

## Tests

- **Repository** suite against the pinned SurrealDB, embedded and server.
- `integration-tests.yml`: migrations on a fresh database, then `INFO FOR DB` compared with the snapshot; export → import round trip.
- **Recovery** suite (export → import, restore drills).
- `documentation.yml` runs `surreal validate` and `surql-fmt --check` on `*.surql`.

## Governing decisions

[ADR 0007](../adr/0007-embedded-single-process-edge-with-cold-spare.md), [ADR 0017](../adr/0017-head-history-and-range-keyed-index-tables.md), [persistence.md § Migrations](../architecture/persistence.md#migrations), [facility-edge.md](../architecture/facility-edge.md), [backup-and-recovery.md](../architecture/backup-and-recovery.md), [deployment.md](../architecture/deployment.md).
