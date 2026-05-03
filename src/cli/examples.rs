//! In-depth `--examples` text for each subcommand.
//!
//! Maintenance rule: any time a flag, subcommand, or user-visible behavior
//! changes, update both the `clap` doc comment that drives `--help` AND the
//! relevant constant here. The two are separate — `--help` summarises;
//! `--examples` shows how to use it on real input.
//!
//! This is documented in `CLAUDE.md` so future sessions stay aligned.

pub const TOP_LEVEL: &str = r#"sqlt — multi-dialect SQL parser, translator, and linter.

Each subcommand has its own `--examples` page with detailed examples
for every flag:

  sqlt parse        --examples    SQL → JSON AST
  sqlt emit         --examples    JSON AST → SQL
  sqlt translate    --examples    SQL → SQL via AST
  sqlt lint         --examples    Analyze SQL for pitfalls
  sqlt build-schema --examples    Compile a reusable schema artifact

Quick tour by subcommand:

  # parse — SQL into a JSON AST envelope.
  sqlt parse --from mariadb -e iso-8859-1 schema.sql > tree.json
  echo "SELECT 1" | sqlt parse --from mysql --pretty

  # emit — JSON AST back to SQL (typically the second half of a pipe).
  sqlt parse --from mariadb tree.sql | sqlt emit --to postgres

  # translate — rewrite SQL between dialects via the AST.
  sqlt translate --from mariadb --to postgres schema.sql > pg.sql
  sqlt translate --from mysql --to mariadb --strict input.sql

  # lint — analyze for pitfalls.
  sqlt lint --from mariadb -e iso-8859-1 schema.sql
  sqlt lint --from mariadb --schema bootstrap.sql --schema migrations/*.sql query.sql
  sqlt lint --from mariadb --to postgres queries.sql   # pre-flight rules
  sqlt lint -v schema.sql                              # surface raw-passthrough warnings
  sqlt lint --explain SQLT0801                         # rule documentation
  sqlt lint --list-rules                               # full ruleset

  # build-schema — compile reusable JSON schema artifact.
  sqlt build-schema --from mariadb \
      --schema shop/bootstrap.sql --schema shop/migrations/*.sql \
      -o shop_schema.json --pretty
  sqlt lint --from mariadb --schema shop_schema.json query.sql

Dialect aliases:
  mysql                                  MySQL 5.7+ / 8.0
  mariadb     | maria                    MariaDB (first-class; not aliased to MySQL)
  postgres    | postgresql | pg          PostgreSQL
  mssql       | tsql       | sqlserver   Microsoft SQL Server (T-SQL)
  sqlite                                 SQLite
  generic                                Permissive fallback dialect

Encoding aliases:
  utf-8                  default (always used for JSON I/O)
  iso-8859-1 | latin1    Latin-1 / ISO-8859-1 8-bit code page
  windows-1252 | cp1252  Windows-1252 (Latin-1 superset)

Exit codes:
  0   clean
  1   parse error, encoding error, or lint findings ≥ --exit-on threshold
  2   usage error (unknown dialect, unknown rule, bad flag combination)
  3   `translate --strict` saw at least one warning
"#;

pub const PARSE: &str = r#"sqlt parse — read SQL, emit a JSON AST envelope.

Reads SQL from a file (positional argument) or stdin (no path or `-`),
parses it with the source dialect, and emits a JSON envelope:

  { "sqlt_version": "0.2.0",
    "dialect": "mariadb",
    "statements": [ { "Insert": { ... } }, ... ] }

JSON output is always UTF-8 (per the JSON spec); `--encoding` only affects
how input bytes are decoded into text.

Examples:

  # From a file
  sqlt parse --from mysql schema.sql

  # From stdin
  echo "SELECT * FROM users" | sqlt parse --from postgres

  # Multi-statement input — produces an array of statements
  printf 'CREATE TABLE t(id INT); INSERT INTO t VALUES (1)' \
      | sqlt parse --from mysql

  # Pretty-printed JSON for readability
  echo "SELECT id FROM users" | sqlt parse --from mysql --pretty

  # Latin-1 dump file (real mariadb-dump output)
  sqlt parse --from mariadb -e iso-8859-1 dump.sql > tree.json

  # Inspect the structure of a tricky statement
  echo "INSERT INTO t (a) VALUES (1) RETURNING id" \
      | sqlt parse --from mariadb --pretty | head -40

MariaDB syntax that has no typed sqlparser node (system versioning,
DELIMITER directives, CREATE DEFINER prefixes, …) appears as a
`{ "sqlt_raw": "...", "reason": "<class>", "start_line": N }` envelope
entry. See `sqlt lint --explain SQLT0001` for the full list.
"#;

pub const EMIT: &str = r#"sqlt emit — render SQL from a JSON AST envelope.

Reads a JSON envelope (the output of `sqlt parse`), runs the upstream
sqlparser `Display` impl per statement, and writes SQL to stdout. JSON
input is always read as UTF-8; `--encoding` selects the SQL output
encoding so you can write back to a Latin-1 system unchanged.

Examples:

  # Round-trip: parse and re-emit. Whitespace may be normalised.
  echo "select id from users" | sqlt parse --from mysql \
      | sqlt emit --to mysql

  # Override target dialect (the envelope's recorded dialect is the default)
  sqlt parse --from mariadb tree.sql | sqlt emit --to postgres

  # Re-encode SQL output to Latin-1 (JSON input stays UTF-8)
  sqlt parse --from mariadb -e iso-8859-1 dump.sql \
      | sqlt emit --to mariadb -e iso-8859-1 > rebuilt.sql

  # Read a JSON tree from a file
  sqlt emit --to mysql tree.json

Note: `emit` is mostly used as the second half of a `parse | emit`
pipeline or for tooling that constructs JSON envelopes directly. To go
SQL → SQL between dialects use `sqlt translate`.
"#;

pub const TRANSLATE: &str = r#"sqlt translate — rewrite SQL between dialects.

Parses the input as `--from <src>`, runs a per-dialect rewriter that
turns source-only constructs into target-dialect equivalents (or warns
when no equivalent exists), and emits SQL in the target dialect.

Warnings go to stderr with stable codes (RETURNING_DROPPED,
SEQUENCE_DROPPED, ON_DUPLICATE_KEY_UNSUPPORTED, RAW_PASSTHROUGH). Use
`--strict` to make any warning a non-zero exit (code 3).

Examples:

  # Drop RETURNING when targeting MySQL (it doesn't support it)
  echo "INSERT INTO t (a) VALUES (1) RETURNING id" \
      | sqlt translate --from mariadb --to mysql 2>warn.log
  # → emits: INSERT INTO t (a) VALUES (1)
  # → warn.log: RETURNING_DROPPED

  # Same input → Postgres (RETURNING preserved cleanly)
  echo "INSERT INTO t (a) VALUES (1) RETURNING id" \
      | sqlt translate --from mariadb --to postgres
  # → emits: INSERT INTO t (a) VALUES (1) RETURNING id

  # ON DUPLICATE KEY UPDATE → ON CONFLICT
  echo "INSERT INTO t(a) VALUES (1) ON DUPLICATE KEY UPDATE a=2" \
      | sqlt translate --from mysql --to postgres

  # Strict mode: fail the build on any translation warning
  sqlt translate --from mariadb --to mysql --strict schema.sql || \
      echo "translation lossy, fix manually"

  # Translate a Latin-1 file with output also in Latin-1
  sqlt translate --from mariadb --to mysql -e iso-8859-1 input.sql \
      > converted.sql

Translation gaps (cases where no faithful target-dialect equivalent
exists) are reported as warnings, not errors. The emitted SQL is the
closest equivalent. To preview gaps without translating, run
`sqlt lint --from <src> --to <dst>` and look for SQLT02xx warnings.
"#;

pub const LINT: &str = r#"sqlt lint — analyze SQL for pitfalls and improvement suggestions.

Runs a curated ruleset (38 rules across 8 categories) over the parsed
AST and reports diagnostics with stable rule IDs (SQLT0500), short
slugs (`select-star`), and inline suggestions.

Categories:
  raw           SQLT00xx  Raw passthrough (off by default; see -v)
  dialect-xc    SQLT01xx  Dialect cross-contamination (e.g. backtick in postgres)
  pre-flight    SQLT02xx  Translation pre-flight (only when --to is set)
  joins         SQLT03xx  Implicit cross joins, NATURAL JOIN, ON 1=1
  subquery      SQLT04xx  IN (SELECT ...) → EXISTS, correlated subqueries
  perf          SQLT05xx  SELECT *, leading-wildcard LIKE, fn-on-column
  correctness   SQLT06xx  = NULL, UPDATE/DELETE without WHERE
  style         SQLT07xx  Unaliased derived tables, LIMIT without ORDER BY
  ddl           SQLT08xx  Float-for-money, VARCHAR without length

Examples:

  # Lint a MariaDB schema (Latin-1 encoded)
  sqlt lint --from mariadb -e iso-8859-1 schema.sql

  # Add translation pre-flight: report things that would break in Postgres
  sqlt lint --from mariadb --to postgres -e iso-8859-1 schema.sql

  # Surface raw-passthrough warnings (mariadb-dump artifacts)
  sqlt lint --from mariadb -e iso-8859-1 -v schema.sql

  # JSON output for tooling
  sqlt lint --from mariadb --format json schema.sql > findings.json

  # SARIF for GitHub code-scanning
  sqlt lint --from mariadb --format sarif schema.sql > out.sarif

  # Pretty grouped output for human review
  sqlt lint --from mariadb --format pretty schema.sql

  # Disable a noisy rule
  sqlt lint --from mysql --no-rule SQLT0500 schema.sql

  # Enable an opt-in rule (default-off rules: SQLT0501, SQLT0506)
  sqlt lint --from mysql --rule SQLT0506 queries.sql

  # Turn warnings into build failures
  sqlt lint --from mysql --exit-on warning schema.sql

  # Print rule documentation
  sqlt lint --explain SQLT0801
  sqlt lint --explain float-for-money     # slug also works
  sqlt lint --explain 0801                # short numeric also works

  # Suppress all `help:` lines (terse one-line-per-finding)
  sqlt lint --from mysql --no-help schema.sql

  # Restore legacy per-finding help (every diagnostic gets help)
  sqlt lint --from mysql --help-mode always schema.sql

  # Filter to errors only (warnings + info still run, just hidden)
  sqlt lint --from mysql --severity error schema.sql

Schema-aware lint with --schema (real production workflow):

  # Point at the bootstrap and migration files that build your real schema.
  # Replays CREATE/ALTER/DROP TABLE, CREATE INDEX, FK constraints, and
  # tracks USE/CREATE DATABASE for per-database namespacing.
  sqlt lint --from mariadb \
      --schema shop/bootstrap.sql --schema shop/migrations/*.sql \
      query.sql

  # Multi-database: same-named tables in different DBs do not collide.
  sqlt lint --from mariadb \
      --schema shop/bootstrap.sql \
      --schema global/bootstrap.sql \
      queries.sql

  # Compile the schema once into a JSON artifact (`sqlt build-schema`),
  # check it into the repo, lint against it on every PR.
  sqlt build-schema --from mariadb \
      --schema shop/bootstrap.sql --schema shop/migrations/*.sql \
      -o shop_schema.json
  sqlt lint --from mariadb --schema shop_schema.json query.sql

  # Mix .json + late .sql migrations on top.
  sqlt lint --from mariadb \
      --schema shop_schema.json \
      --schema shop/migrations/2026-05-12-add-col.sql \
      query.sql

Common workflow on a real production dump:

  # 1. See actionable findings (raw-passthrough hidden by default)
  sqlt lint --from mariadb -e iso-8859-1 dump.sql

  # 2. Investigate a specific finding's rule
  sqlt lint --explain SQLT0801

  # 3. Once familiar, run with --verbose for parser-coverage info
  sqlt lint --from mariadb -e iso-8859-1 -v dump.sql

  # 4. Pin to JSON for CI ingestion
  sqlt lint --from mariadb -e iso-8859-1 --format json --exit-on error \
      dump.sql > findings.json
"#;

pub const BUILD_SCHEMA: &str = r#"sqlt build-schema — compile a reusable schema artifact.

Reads one or more `--schema` files (CREATE/ALTER/DROP TABLE, CREATE INDEX,
CREATE DATABASE, USE, plus `mariadb-dump`-style noise), replays the DDL,
and emits a JSON file that captures the *current* state of the schema.
The artifact can be reloaded by `sqlt lint --schema schema.json` without
re-parsing or re-replaying the original SQL.

────────────────────────────────────────────────────────────────────
Flag reference
────────────────────────────────────────────────────────────────────

  --from <DIALECT>          (required)
      The SQL dialect used to parse every `--schema` file. Values:
      mysql, mariadb, postgres (alias postgresql), mssql (alias tsql),
      sqlite, generic.
      Optional only when `--examples` is given.

  --schema <FILE>           (repeatable, required)
      Schema input file. Accepts:
        * .sql  — parsed with `--from`, replayed via the DDL engine
        * .json — a previously compiled artifact, merged into the
                   running schema (so you can layer new migrations
                   onto a precompiled base)
      Files are processed in CLI order; the USE cursor and any
      CREATE DATABASE state persist across files.

  -e, --encoding <ENC>
      How to decode the bytes of `.sql` schema files. Default: utf-8.
      Aliases: latin1 = iso-8859-1, cp1252 = windows-1252.
      JSON output is always written as UTF-8 (per JSON spec).

  -o, --output <PATH>
      Path to write the JSON artifact. Omit to write to stdout.

  --pretty
      Pretty-print the JSON output (indented, diff-friendly). Default
      is compact one-line JSON.

  --examples
      Print this page and exit.

────────────────────────────────────────────────────────────────────
Common workflows
────────────────────────────────────────────────────────────────────

  # Most common: compile a migration history into a JSON artifact
  # and lint against it. Cheap on every CI run.
  sqlt build-schema --from mariadb \
      --schema shop/bootstrap.sql \
      --schema shop/migrations/*.sql \
      -o shop_schema.json --pretty
  sqlt lint --from mariadb --schema shop_schema.json query.sql

  # Layer late-arriving migrations on top of the artifact instead of
  # rebuilding the artifact every time.
  sqlt lint --from mariadb \
      --schema shop_schema.json \
      --schema shop/migrations/2026-05-12-add-col.sql \
      query.sql

  # Multi-database project: shop_db + global_db.
  # CREATE DATABASE and USE statements in either file set up the
  # namespaces; same-named tables across DBs do not collide.
  sqlt build-schema --from mariadb \
      --schema shop/bootstrap.sql \
      --schema global/bootstrap.sql \
      -o combined.json
  sqlt lint --from mariadb --schema combined.json queries.sql

  # Latin-1 schema files (real mariadb-dump output is often latin1).
  sqlt build-schema --from mariadb -e iso-8859-1 \
      --schema dump.sql \
      -o schema.json

  # Inspect the artifact: stdout output piped to jq.
  sqlt build-schema --from mysql --schema bootstrap.sql --pretty | jq '.'

  # Quick sanity-check: how many tables / databases did we end up with?
  sqlt build-schema --from mariadb \
      --schema schema.sql -o /tmp/s.json
  jq '[.databases | to_entries[] | .value.tables | keys] | flatten | length' /tmp/s.json

  # Inline schema for a one-off lint run via process substitution.
  sqlt build-schema --from mysql --schema <(echo '
      CREATE TABLE users (id INT NOT NULL, name VARCHAR(50));
      ALTER TABLE users ADD COLUMN email VARCHAR(255);
  ')

────────────────────────────────────────────────────────────────────
What gets tracked
────────────────────────────────────────────────────────────────────

  - tables (per database; CREATE DATABASE / USE namespaces)
  - columns (name, data type, nullable, primary-key membership)
  - indexes (named, unique, primary, fulltext, spatial; functional
    indexes via the rendered SQL expression)
  - primary keys
  - foreign keys (resolved through the USE cursor)

Statements that don't affect the schema (INSERT, UPDATE, DELETE, GRANT,
DELIMITER + stored procedure bodies, ALTER TABLE ENABLE/DISABLE KEYS,
…) emit a `note: skipping <kind>` line on stderr but never error.

────────────────────────────────────────────────────────────────────
Versioning
────────────────────────────────────────────────────────────────────

The artifact records the sqlt version it was built with — `sqlt lint`
emits a `note:` to stderr on a major.minor mismatch but still tries
to load. Patch-version differences are accepted silently.

────────────────────────────────────────────────────────────────────
Exit codes
────────────────────────────────────────────────────────────────────

  0   schema compiled (any skip notes are advisory)
  1   parse error, encoding error, or I/O error on a schema file
  2   usage error (unknown dialect, no --schema given, bad flags)
"#;

pub fn print(text: &str) {
    print!("{text}");
}
