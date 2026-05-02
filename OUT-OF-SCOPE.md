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

### Lint
- `.sqlt.toml` config file. v1 is CLI-flag-driven only. When this lands, the planned shape is `[lint] disabled = ["SQLT0500"]` and `[lint] severity = { SQLT0500 = "warning" }` overrides.
- `SQLT0700` keyword-case-mixed — sqlparser strips raw token case in the AST; the rule cannot be implemented without re-tokenizing the source. We deliberately do *not* register this rule so `--rule SQLT0700` returns "unknown rule" rather than silently doing nothing.
- Schema-aware semantic checks. The linter knows nothing about column types, indexes, NOT NULL constraints, foreign keys, etc. Rules with ⚠ in their `--explain` text rely on heuristics that produce false positives.
- Auto-fix / fix mode. v1 reports only.
- Per-line `-- sqlt:disable SQLT0500` suppression comments.
- Per-rule severity overrides on the CLI (`--rule SQLT0500=warning` style).

### Encodings
- UTF-16 (LE/BE) input/output and BOM detection.
- Auto-detection of input encoding (`chardetng` integration). Today the user must pass `--encoding` explicitly; we deliberately don't guess because heuristic detection silently corrupts data on short inputs.
- Mixed-encoding input (e.g. SQL files with comments in one encoding and string literals in another).
- Encoding negotiation between `--from` and `--to` (e.g. translating Latin-1 source SQL to UTF-8 output as a single command).

### CLI / UX
- A formatting/pretty-printer with knobs beyond `--pretty` (line width, indent style, keyword case).
- Watch mode / continuous translation.
- A library API stable enough for external crates to depend on (v1 keeps `lib.rs` re-exports for tests but no compatibility guarantee).
- Error messages with byte positions when `sqlparser-rs` does not surface them.

### Tooling
- Pre-built binaries / release pipeline (`cargo install --git` is the v1 distribution path).
- Homebrew / package manager publication.
- Web playground.
