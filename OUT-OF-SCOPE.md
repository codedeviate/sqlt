# Out of Scope & Wishlist

A living list of items raised during design, implementation, or feature sweeps
that are either explicitly deferred, decided against, or noted as "maybe
later". Also doubles as a wishlist — items under "Waiting" are things worth
building once someone explicitly asks. Kept here so ideas don't disappear
into the black hole of spec files after each release.

Organized into four buckets by reason for non-inclusion. When an item ships,
remove it from this file and note the shipping version in the CHANGELOG
entry rather than leaving a crossed-out line here.

- **Waiting** — can be done; nobody's asked for it.
- **Deferred** — possible to implement; actively put off (scope/complexity
  trade-off or waiting on a concrete use case).
- **Not yet supported** — blocked by upstream / ecosystem maturity; may ship
  when the blocker clears.
- **Out of scope** — fundamentally can't be implemented, architecturally
  mismatched, or intentionally declined by policy.

---

## Waiting

### CLI / UX
- Watch mode / continuous translation.
- Web playground.
- A library API stable enough for external crates to depend on (today `lib.rs`
  re-exports exist for tests but carry no compatibility guarantee).

### Lint ergonomics
- Per-line `-- sqlt:disable SQLT0500` suppression comments.
- Per-rule severity overrides on the CLI (`--rule SQLT0500=warning` style).
- Auto-fix / fix mode. Today `sqlt lint` reports only.

## Deferred

### Dialects
- BigQuery, ClickHouse, Snowflake, DuckDB, Redshift, Databricks, Hive —
  `sqlparser-rs` supports these but the CLI does not expose them through
  `DialectId` to keep the surface focused.
- Oracle — same reasoning.
- ANSI strict mode.

### Translation
- Complex `MERGE` ↔ `INSERT ... ON CONFLICT/DUPLICATE` rewriting beyond the
  simple cases.
- Stored-procedure dialect translation (PL/SQL ↔ T-SQL ↔ PL/pgSQL).
- Comment preservation across translation (round-trip within one dialect
  preserves them via the raw-SQL fallback; cross-dialect drops them).
- Index / constraint name normalization.

### Lint depth
- `.sqlt.toml` config file. Today the lint surface is CLI-flag-driven only.
  When this lands, the planned shape is `[lint] disabled = ["SQLT0500"]` and
  `[lint] severity = { SQLT0500 = "warning" }` overrides.
- Schema-aware features beyond the v0.3 baseline. **Already shipped:**
  `Schema` model with full DDL replay, `--schema <file>` (SQL and JSON),
  per-database namespacing, `sqlt build-schema` artifact compilation,
  SQLT0900 unknown-column, schema-aware refinement of SQLT0505 and SQLT0400.
  Indexes and foreign keys are recorded but not yet consumed.
  **Still deferred:** an SQLT0503 refinement that consults the index list
  (would require deciding what counts as "indexed by `LOWER(col)`" for
  functional indexes), full type checking on every comparison (richer
  SQLT0506), CTE/VIEW expansion, ambiguous-column detection on multi-table
  joins, unknown-table warnings (too many false positives when the schema
  lives in a different file from the queries), and FK-driven JOIN suggestions.
- Other ⚠ schema-blind heuristic rules (SQLT0506 implicit-string-numeric-compare,
  SQLT0503 function-on-column-in-where) still rely on heuristics that produce
  false positives. They are gated behind opt-in defaults until full schema
  awareness lands.

### Parsing depth
- Query plan analysis or optimization-hints rewriting.

### CLI / UX
- A formatting/pretty-printer with knobs beyond `--pretty` (line width,
  indent style, keyword case).
- Error messages with byte positions when `sqlparser-rs` does not surface them.

### Encodings
- UTF-16 (LE/BE) input/output and BOM detection.
- Mixed-encoding input (e.g. SQL files with comments in one encoding and
  string literals in another).
- Encoding negotiation between `--from` and `--to` (e.g. translating Latin-1
  source SQL to UTF-8 output as a single command).

## Not yet supported

### MariaDB raw-SQL fallback for unrepresented constructs
`sqlparser-rs` v0.59 lacks AST nodes for several MariaDB-specific constructs.
The current implementation captures these as a raw-SQL fallback variant:
parsing preserves the original text so that round-tripping (parse → emit) is
lossless **for the same dialect**, but cross-dialect translation cannot
rewrite them and will emit a warning + the original SQL.

Affected constructs:
- `WITH SYSTEM VERSIONING` table option.
- `PERIOD FOR SYSTEM_TIME (start_col, end_col)` column-level / table-level
  construct.
- `FOR SYSTEM_TIME AS OF | BETWEEN | FROM ... TO | ALL` query-level temporal
  predicates.
- `CREATE PACKAGE` / `CREATE PACKAGE BODY` (Oracle-compat mode).
- MariaDB vector types (`VECTOR(N)`) and vector functions (`VEC_DISTANCE_*`,
  `VEC_FROMTEXT`, etc.).
- Application-time period definitions (`PERIOD FOR <name> (start, end)`
  non-system-time).

The intent is to upstream typed AST support for these to
`apache/datafusion-sqlparser-rs` and remove the fallback once accepted.

### Stored program bodies
Stored procedure / function / trigger bodies parsed beyond statement
boundaries. Today they are treated as opaque blocks where the upstream parser
does so; removing the opacity requires upstream parser support.

## Out of scope

### Encoding auto-detection
Auto-detection of input encoding (e.g. `chardetng` integration). The user
must pass `--encoding` explicitly. We deliberately don't guess because
heuristic detection silently corrupts data on short inputs.

### `SQLT0700` keyword-case-mixed
`sqlparser-rs` strips raw token case in the AST; the rule cannot be
implemented without re-tokenizing the source. The rule is intentionally not
registered, so `--rule SQLT0700` returns "unknown rule" rather than silently
doing nothing.
