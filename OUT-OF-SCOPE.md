# Out of Scope (v1) and Known Limitations

This file tracks features and constructs deliberately deferred from v1, and known limitations of the v1 implementation. Move items out of this list as they ship.

## Known limitations in v1

### MariaDB raw-SQL fallback for unrepresented constructs

`sqlparser-rs` v0.59 lacks AST nodes for several MariaDB-specific constructs. v1 captures these as a raw-SQL fallback variant: parsing preserves the original text so that round-tripping (parse → emit) is lossless **for the same dialect**, but cross-dialect translation cannot rewrite them and will emit a warning + the original SQL.

Affected constructs:
- `WITH SYSTEM VERSIONING` table option.
- `PERIOD FOR SYSTEM_TIME (start_col, end_col)` column-level / table-level construct.
- `FOR SYSTEM_TIME AS OF | BETWEEN | FROM ... TO | ALL` query-level temporal predicates.
- `CREATE PACKAGE` / `CREATE PACKAGE BODY` (Oracle-compat mode).
- MariaDB vector types (`VECTOR(N)`) and vector functions (`VEC_DISTANCE_*`, `VEC_FROMTEXT`, etc.).
- Application-time period definitions (`PERIOD FOR <name> (start, end)` non-system-time).

The intent is to upstream typed AST support for these to `apache/datafusion-sqlparser-rs` and remove the fallback once accepted.

## Deferred to later versions

### Dialects
- BigQuery, ClickHouse, Snowflake, DuckDB, Redshift, Databricks, Hive — `sqlparser-rs` supports these but v1 does not expose them through `DialectId` to keep the surface focused.
- Oracle — same reasoning.
- ANSI strict mode.

### Parsing depth
- Stored procedure / function / trigger bodies parsed beyond statement boundaries (currently treated as opaque blocks where the upstream parser does so).
- Schema-aware semantic checks (table/column existence, type checking) — out of scope; this is a parser/translator, not a linter.
- Query plan analysis or optimization hints rewriting.

### Translation
- Complex `MERGE` ↔ `INSERT ... ON CONFLICT/DUPLICATE` rewriting beyond the simple cases.
- Stored-procedure dialect translation (PL/SQL ↔ T-SQL ↔ PL/pgSQL).
- Comment preservation across translation (round-trip within one dialect preserves them via raw-SQL fallback; cross-dialect drops them).
- Index / constraint name normalization.

### CLI / UX
- A formatting/pretty-printer with knobs beyond `--pretty` (line width, indent style, keyword case).
- Watch mode / continuous translation.
- A library API stable enough for external crates to depend on (v1 keeps `lib.rs` re-exports for tests but no compatibility guarantee).
- Error messages with byte positions when `sqlparser-rs` does not surface them.

### Tooling
- Pre-built binaries / release pipeline (`cargo install --git` is the v1 distribution path).
- Homebrew / package manager publication.
- Web playground.
