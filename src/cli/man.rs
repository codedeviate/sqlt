//! `sqlt man` — prints a man-page-style manual to stdout.
//!
//! Maintenance rule: every flag, subcommand, exit code, dialect, encoding,
//! or environment variable the binary exposes MUST be reflected here. This
//! is the single discoverable place that combines `--help`, `--examples`,
//! and the rule taxonomy into one document. Keep it in sync with:
//!     * the clap doc comments in `src/cli/mod.rs`
//!     * the per-command constants in `src/cli/examples.rs`
//!     * `README.md`
//!
//! See `CLAUDE.md` for the full update protocol.

use crate::error::Result;

pub fn run() -> Result<()> {
    crate::cli::style::print_colored(MANUAL);
    Ok(())
}

pub const MANUAL: &str = r#"SQLT(1)                       sqlt manual                       SQLT(1)

────────────────────────────────────────────────────────────────────
NAME
────────────────────────────────────────────────────────────────────

  sqlt — multi-dialect SQL parser, translator, and linter.

────────────────────────────────────────────────────────────────────
SYNOPSIS
────────────────────────────────────────────────────────────────────

  sqlt [--examples] [--help]
  sqlt parse        --from <DIALECT> [--pretty] [-e <ENC>] [FILE|-]
  sqlt emit         [--to <DIALECT>] [-e <ENC>] [FILE|-]
  sqlt translate    --from <SRC> --to <DST> [--strict] [-e <ENC>] [FILE|-]
  sqlt lint         --from <SRC> [--to <DST>] [--format <FMT>]
                    [--rule <ID>]... [--no-rule <ID>]...
                    [--severity <LVL>] [--exit-on <LVL>]
                    [--help-mode <MODE> | --no-help]
                    [--schema <FILE>]... [--verbose] [-e <ENC>] [FILE|-]
  sqlt lint         --explain <ID>
  sqlt lint         --list-rules
  sqlt build-schema --from <DIALECT> --schema <FILE>... [-o <PATH>]
                    [--pretty] [-e <ENC>]
  sqlt man

────────────────────────────────────────────────────────────────────
DESCRIPTION
────────────────────────────────────────────────────────────────────

  sqlt parses SQL written in any of the supported dialects into a JSON
  AST envelope, emits SQL back from that envelope, translates between
  dialects via the AST, and lints SQL for portability, performance, and
  correctness pitfalls.

  Each subcommand reads input from a positional FILE argument or from
  stdin when FILE is omitted or given as `-`. All JSON I/O is UTF-8 per
  the JSON spec; the `--encoding` flag governs only the SQL byte
  encoding.

  MariaDB is treated as a first-class target distinct from MySQL:
  mariadb-dump-only constructs (`/*!NNN ... */` conditional comments,
  DELIMITER directives, system versioning, CREATE PACKAGE, CREATE
  SEQUENCE, FOR SYSTEM_TIME, optimization hints, DEFINER clauses) are
  recognised, preprocessed, and either parsed or wrapped as `Raw`
  fragments rather than silently rejected.

────────────────────────────────────────────────────────────────────
COMMANDS
────────────────────────────────────────────────────────────────────

  parse
      SQL → JSON AST. Reads SQL with `--from <DIALECT>`, emits a JSON
      envelope `{ sqlt_version, dialect, statements }`. `--pretty`
      pretty-prints the JSON. See `sqlt parse --examples` for the full
      set of examples.

  emit
      JSON AST → SQL. Reads the envelope produced by `sqlt parse`,
      writes SQL via the sqlparser `Display` impl. `--to <DIALECT>`
      overrides the dialect recorded in the envelope. See
      `sqlt emit --examples`.

  translate
      SQL → SQL via AST. Parses with `--from`, runs the per-dialect
      rewriter that converts source-only constructs to target
      equivalents (or warns when no equivalent exists), and emits SQL
      in `--to`. Warnings go to stderr; `--strict` turns any warning
      into exit code 3. See `sqlt translate --examples`.

  lint
      Analyzes SQL for pitfalls and improvement suggestions. Runs a
      curated ruleset (≈38 rules across 8 categories) over the parsed
      AST. Output formats: `text` (default), `pretty`, `json`, `sarif`.
      `--explain <ID>` and `--list-rules` document the ruleset
      offline. See `sqlt lint --examples`.

  build-schema
      Compiles one or more `--schema` files (CREATE/ALTER/DROP TABLE,
      CREATE INDEX, foreign-key constraints, USE/CREATE DATABASE) into
      a reusable JSON artifact. `sqlt lint --schema artifact.json`
      reloads the compiled state without re-parsing the migration
      history. See `sqlt build-schema --examples`.

  man
      Prints this manual. Pipe to a pager for navigation:
      `sqlt man | less -R`.

────────────────────────────────────────────────────────────────────
DIALECTS
────────────────────────────────────────────────────────────────────

  mysql                                  MySQL 5.7+ / 8.0
  mariadb     | maria                    MariaDB (first-class)
  postgres    | postgresql | pg          PostgreSQL
  mssql       | tsql       | sqlserver   Microsoft SQL Server (T-SQL)
  sqlite                                 SQLite
  generic                                Permissive fallback dialect

────────────────────────────────────────────────────────────────────
ENCODINGS (--encoding / -e)
────────────────────────────────────────────────────────────────────

  utf-8                  default (always used for JSON I/O)
  iso-8859-1 | latin1    Latin-1 / ISO-8859-1 8-bit code page
  windows-1252 | cp1252  Windows-1252 (Latin-1 superset)

  Decoding is strict — invalid byte sequences are rejected with exit
  code 1, never silently substituted with U+FFFD.

────────────────────────────────────────────────────────────────────
LINT RULE CATEGORIES
────────────────────────────────────────────────────────────────────

  raw           SQLT00xx  Raw passthrough (off by default; see -v)
  dialect-xc    SQLT01xx  Dialect cross-contamination
  pre-flight    SQLT02xx  Translation pre-flight (only with --to)
  joins         SQLT03xx  Implicit cross joins, NATURAL JOIN, ON 1=1
  subquery      SQLT04xx  IN (SELECT) → EXISTS, correlated subqueries
  perf          SQLT05xx  SELECT *, leading-wildcard LIKE, fn-on-column
  correctness   SQLT06xx  = NULL, UPDATE/DELETE without WHERE
  style         SQLT07xx  Unaliased derived tables, LIMIT w/o ORDER BY
  ddl           SQLT08xx  Float-for-money, VARCHAR without length

  `--rule <ID>` and `--no-rule <ID>` accept the full id (SQLT0500),
  short numeric (0500 / 500), or slug (select-star). `--explain <ID>`
  prints a rule's summary, long-form explanation, default severity,
  and default-enabled state.

────────────────────────────────────────────────────────────────────
TRANSLATION WARNINGS
────────────────────────────────────────────────────────────────────

  Stable codes emitted on stderr by `sqlt translate`:

  RETURNING_DROPPED              Target dialect lacks RETURNING.
  SEQUENCE_DROPPED               Target dialect lacks CREATE SEQUENCE.
  ON_DUPLICATE_KEY_UNSUPPORTED   Target dialect lacks ON DUPLICATE KEY.
  RAW_PASSTHROUGH                Source had a Raw fragment with no
                                  faithful target equivalent.

  Use `--strict` to make any warning a non-zero exit (code 3).

────────────────────────────────────────────────────────────────────
EXAMPLES
────────────────────────────────────────────────────────────────────

  # Parse SQL → JSON AST
  echo "SELECT 1" | sqlt parse --from mysql --pretty

  # Emit JSON AST → SQL
  sqlt parse --from postgres schema.sql | sqlt emit --to mysql

  # Translate between dialects
  sqlt translate --from mariadb --to postgres input.sql

  # Strict translation: fail the build on any warning
  sqlt translate --from mariadb --to mysql --strict schema.sql

  # Lint a MariaDB schema (Latin-1)
  sqlt lint --from mariadb -e iso-8859-1 schema.sql

  # Lint with translation pre-flight against Postgres
  sqlt lint --from mariadb --to postgres schema.sql

  # JSON output for tooling
  sqlt lint --from mariadb --format json schema.sql > findings.json

  # SARIF for GitHub code-scanning
  sqlt lint --from mariadb --format sarif schema.sql > out.sarif

  # Compile a migration history once, lint against it on every PR
  sqlt build-schema --from mariadb \
      --schema shop/bootstrap.sql --schema shop/migrations/*.sql \
      -o shop_schema.json
  sqlt lint --from mariadb --schema shop_schema.json query.sql

  # Documentation
  sqlt lint --list-rules                                 # full ruleset
  sqlt lint --explain SQLT0801                           # one rule
  sqlt <command> --examples                              # in-depth
  sqlt man | less -R                                     # this page

────────────────────────────────────────────────────────────────────
EXIT STATUS
────────────────────────────────────────────────────────────────────

  0   clean run
  1   parse error, encoding error, I/O error, or lint findings ≥
      `--exit-on` threshold
  2   usage error (unknown dialect, unknown rule, bad flag combination)
  3   `translate --strict` saw at least one warning

────────────────────────────────────────────────────────────────────
FILES
────────────────────────────────────────────────────────────────────

  Input is read from a positional path or from stdin (omit the path or
  pass `-`). `sqlt build-schema --output <PATH>` writes the JSON
  artifact; `sqlt lint --schema <PATH>` accepts both `.sql` and `.json`
  artifacts.

────────────────────────────────────────────────────────────────────
ENVIRONMENT
────────────────────────────────────────────────────────────────────

  NO_COLOR        When set (any value), disables ANSI colour in
                  `--examples`, `sqlt man`, and lint output.
  CLICOLOR_FORCE  When set, forces ANSI colour even when stdout is
                  not a TTY. Recognised by the `colored` crate.

────────────────────────────────────────────────────────────────────
SEE ALSO
────────────────────────────────────────────────────────────────────

  sqlt --examples                Top-level overview with examples.
  sqlt <command> --help          Long-form help for one subcommand.
  sqlt <command> --examples      In-depth examples for one subcommand.
  README.md                      Project README with install info.
  CHANGELOG.md                   Per-release notes.
  https://github.com/codedeviate/sqlt

────────────────────────────────────────────────────────────────────
BUGS
────────────────────────────────────────────────────────────────────

  Report bugs at https://github.com/codedeviate/sqlt/issues.

────────────────────────────────────────────────────────────────────
AUTHOR
────────────────────────────────────────────────────────────────────

  codedeviate <codedv8@gmail.com>
"#;
