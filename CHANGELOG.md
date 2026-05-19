# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `sqlt man` subcommand prints a man-page-style manual (NAME, SYNOPSIS, DESCRIPTION, COMMANDS, DIALECTS, ENCODINGS, LINT RULE CATEGORIES, TRANSLATION WARNINGS, EXAMPLES, EXIT STATUS, FILES, ENVIRONMENT, SEE ALSO, BUGS, AUTHOR). Output is ANSI-colored on a TTY; pipe to `less -R` for paging.
- Colorized `--examples` output across every subcommand, matching the recon-style scheme (yellow-bold section headers, bold sub-headings, cyan commands, green shell comments, dimmed dividers and notes). Respects `NO_COLOR` / `CLICOLOR_FORCE`.
- `README.md` header now ships a badge row (GitHub, latest release, crates.io, Homebrew tap, Rust edition / MSRV, MIT license).

### Changed
- Default SIGPIPE behaviour is restored at startup on Unix, so `sqlt man | less` (or any other pager / `head` / `grep`) closing early ends the process quietly instead of panicking on the next `println!`.

## [0.3.2] - 2026-05-08

### Added
- `LICENSE` file containing the full MIT License text. Previously the repo declared a license in `Cargo.toml` but shipped no LICENSE file.
- `Cargo.toml` metadata required for `cargo publish` to crates.io: `authors`, `readme`, `homepage`, `documentation`, `keywords` (`sql`, `parser`, `translator`, `dialect`, `lint`), `categories` (`command-line-utilities`, `database`, `parser-implementations`), and an `exclude` list (`.github/**`, `.idea/**`, `target/**`, `/.gitignore`, `OUT-OF-SCOPE.md`, `CLAUDE.md`, `BREW.md`) that keeps maintainer-facing files out of the published tarball.
- `[profile.release]` settings (`lto = true`, `codegen-units = 1`, `strip = "symbols"`) so binaries built by Homebrew (and anyone running `cargo install --path .`) are smaller. Trade-off: longer release build (~3 minutes vs. seconds).
- `README.md` install section now documents Homebrew (`brew install codedeviate/cli/sqlt`) and crates.io (`cargo install sqlt`) alongside the existing `cargo install --path .` source build.

### Changed
- Homebrew distribution now lives in the consolidated `codedeviate/homebrew-cli` tap; the in-repo `Formula/sqlt.rb` template was retired in favour of the tap's authoritative copy. The crate tarball no longer carries a Homebrew formula.
- `Cargo.toml` license field narrowed from `MIT OR Apache-2.0` to `MIT` to match the single LICENSE file in the repository.
- `Cargo.toml` `repository` field corrected from `https://github.com/thomasbjork/sqlt` to `https://github.com/codedeviate/sqlt` (the actual remote).
- `LICENSE` copyright holder set to `codedeviate` to match the publishing identity used on crates.io and Homebrew.
- `README.md` License section updated from the stale "Dual-licensed under MIT or Apache-2.0" text to a single MIT note that links to `LICENSE`.

## [0.3.1] - 2026-05-03

### Added
- Top-level `sqlt --examples` flag. Prints a cross-cutting overview with example invocations of every subcommand. Complements the existing per-subcommand `sqlt <COMMAND> --examples`.

### Changed
- `sqlt build-schema --help` now renders the full long-form description (use cases, what's tracked, version handling) via a `BUILD_SCHEMA_LONG_ABOUT` constant. Previously only the one-line summary was visible from `--help`, requiring users to guess the syntax.
- Per-flag descriptions on `BuildSchemaArgs` (every flag) describe accepted values, defaults, and interactions in line with the help-text policy in `CLAUDE.md`.
- `sqlt build-schema --examples` rewritten to a four-section format: Flag reference (every flag, with defaults), Common workflows (covering each commonly-used flag combination), What gets tracked, Versioning, Exit codes. Includes process-substitution and jq-piping recipes.
- `CLAUDE.md` "Help & examples maintenance rule" expanded with concrete standards: every subcommand variant must have a `long_about` (not just a one-line `///`), every flag mentioned in `--help` must appear in `--examples`, both `sqlt --examples` and `sqlt <COMMAND> --examples` must work. Listed as mandatory for every change that affects how the utility is used.

## [0.3.0] - 2026-05-02

### Added
- **`--schema <file>` flag on `sqlt lint`** (repeatable). Parses each file as schema input — `CREATE TABLE`, `ALTER TABLE`, `DROP TABLE`, `CREATE INDEX`, foreign-key constraints, `CREATE DATABASE`, `USE`. Replays them in CLI order so the schema reflects its current state, not its initial CREATE. Schema files are NOT linted (only feed the model). Statements that don't affect the schema (INSERT, GRANT, DELIMITER, stored-procedure bodies, …) emit `note: skipping <kind> at <file>:<line>` on stderr. Files with `.json` extension are loaded as a previously built artifact (see `build-schema`).
- **`sqlt build-schema` subcommand.** Compiles one or more `--schema` files into a JSON artifact that can be consumed by `--schema schema.json` later. Use case: a long migration history that takes time to parse + replay can be compiled once, checked into the repo, and re-used by every CI lint run. Mixing `.json` + `.sql` is supported: `sqlt lint --schema base.json --schema migrations/late.sql query.sql`.
- **Per-database namespacing.** Schemas now track `CREATE DATABASE` and `USE`; tables live in per-database namespaces. Queries can use 3-part `db.table.col` references; the `USE` cursor persists across `--schema` files in CLI order. Same-named tables in different databases no longer collide (e.g. `shop_db.orders` and `global_db.orders`).
- **Full DDL replay** in `Schema::apply_statement`: CREATE/ALTER/DROP TABLE, ADD/DROP/MODIFY/CHANGE/RENAME COLUMN, RENAME TO (within and across DB), CREATE [UNIQUE] INDEX, ALTER TABLE ADD INDEX/UNIQUE/PRIMARY KEY/FULLTEXT/SPATIAL, ADD CONSTRAINT FOREIGN KEY, DROP INDEX/CONSTRAINT/FOREIGN KEY/PRIMARY KEY, CREATE TABLE LIKE (column copy from referenced table), CREATE OR REPLACE TABLE. `IF EXISTS`/`IF NOT EXISTS` guards are honored. Missing-target ops emit a stderr `note:` but never error.
- `Schema` is now serializable via `serde` for the JSON artifact path. Indexes and foreign keys are tracked in the model (no rule consumes them yet, but the data is available for future SQLT0503-style refinements).
- ~17 new schema unit tests covering each DDL shape, plus 6 new CLI integration tests for `--schema`, multi-DB namespacing, late-migration mixing, and the build-schema round-trip.

### Added (earlier this cycle, still in 0.3.0)
- **Schema-aware lint pass.** `lint::lint` now builds a `Schema` model from every `CREATE TABLE` statement in the input before running rules and passes it to each rule via `LintCtx::schema`. Lookups are case-insensitive; `NOT NULL` and `PRIMARY KEY` are tracked per column.
- New rule **SQLT0900 unknown-column** (Schema category, error, default-on). Fires when a `table.col` reference resolves to a `CREATE TABLE` in the same input but the column doesn't exist on that table. Also catches typos in the `INSERT INTO Foo (col1, col2)` column list. Conservative — never warns about CTEs, derived-table aliases, or tables defined elsewhere.
- New rule category `Schema` (`SQLT09xx` namespace) for rules that consult the schema model.

### Changed
- **SQLT0505 count-of-nullable-column** is now schema-aware: when a `CREATE TABLE` for the referenced column is present and the column is declared `NOT NULL`, the warning is suppressed (since `COUNT(col)` and `COUNT(*)` are guaranteed equivalent).
- **SQLT0400 not-in-subquery-null-pitfall** is now schema-aware: when the inner subquery projects a single column that resolves to a known `NOT NULL` column, the warning is suppressed (the NULL pitfall cannot trigger).

### Fixed
- MariaDB parsing now uses `MySqlDialect` directly instead of a forwarding wrapper. The wrapper failed `dialect_of!(MySqlDialect)` downcast checks scattered through sqlparser, which silently disabled MySQL-superset features that real MariaDB grammar relies on (`ON UPDATE` timestamp column option, table hints, `LIMIT a, b`, `LOCK/UNLOCK TABLES`, etc.). Real-world `mariadb-dump` output now parses to typed AST instead of falling back to raw passthrough — verified on a 73 KB production schema where 49 raw fragments collapsed to 17 (only legitimately-tricky DELIMITER directives and stored-program bodies remain).
- MariaDB inputs are pre-processed to inject a space after bare `--` at end-of-line. `mariadb-dump` emits `--` on a line by itself; sqlparser tokenized that as two minus operators because `MySqlDialect::requires_single_line_comment_whitespace` is `true`. The preprocessor tracks string-literal / quoted-identifier / block-comment / line-comment state so the substitution never corrupts data.
- `RawStatement` carries a `start_line` so SQLT0001 (raw-passthrough) lint diagnostics report the actual source line of each fragment instead of all firing at `1:1`. The field is metadata only — `PartialEq` for `RawStatement` ignores it so round-trip parse → emit → parse still produces equal ASTs.
- Raw classifier recognises `delimiter` and `stored_program_body` (CREATE TRIGGER/FUNCTION/PROCEDURE) reasons so the diagnostic message is more actionable.
- Raw classifier additionally recognises `optimization_hint` (`ALTER TABLE … DISABLE/ENABLE KEYS`, the standard mariadb-dump wrapper around per-table INSERT blocks), `definer_clause` (`CREATE DEFINER=…`), and `create_event`. The classifier peeks through a leading conditional comment (`/*!NNN …*/`, `/*M!NNN …*/`) before pattern-matching, so wrapped statements are recognised by their inner content.
- MariaDB/MySQL conditional comments (`/*!NNN … */`, `/*M!NNN … */`, also their unversioned forms) are unwrapped during MariaDB preprocessing so the inner SQL parses to typed AST. Marker characters are replaced with spaces of equal length so source line/column positions are preserved end-to-end.
- Per-fragment AST line numbers are now rebased to the original file's line space. The MariaDB fallback path prepends `start_line - 1` newlines to each fragment before re-parsing, so every `Location.line` in the resulting AST refers to the actual source file (e.g. lint findings on a `CREATE TABLE` at file line 17981 now report `17981`, not `1`).
- DDL rules (`SQLT0801 float-for-money`, `SQLT0802 varchar-without-length`) now attach diagnostics to the column's identifier span (`col.name.span`) rather than the statement span. `Statement::CreateTable` has no `Spanned` impl upstream, so the previous span fell back to `1:1`; the column-name span is always populated.

### Changed
- Text-format `help:` lines are deduplicated by default. The same suggestion is emitted only once per `(rule_id, suggestion)` pair within a render — for SQLT0001 with hundreds of identical raw-passthrough findings the help shows once at the first occurrence. Output volume on a 7 MB MariaDB dump dropped from 1371 lines to 688 (~50%) with no information lost.

### Added
- `--help-mode auto|always|never` flag on `sqlt lint` controlling text-format help rendering. `auto` (default) deduplicates per `(rule_id, suggestion)`. `always` restores the per-finding rendering. `never` suppresses help entirely.
- `--no-help` flag as a shorthand for `--help-mode never`.
- `--verbose` / `-v` flag on `sqlt lint`. SQLT0001 (raw-passthrough) is now off by default — real `mariadb-dump` output emits hundreds of fragments sqlparser can't parse (DISABLE/ENABLE KEYS, DELIMITER directives, CREATE DEFINER prefixes), and the flood of identical warnings drowned the typed-AST rule findings. `--verbose` enables SQLT0001 (equivalent to `--rule SQLT0001`).
- `--examples` flag on every subcommand (`parse`, `emit`, `translate`, `lint`). Prints in-depth examples and exits 0 without parsing any input. Examples cover the basic invocation, every commonly-used flag, file-vs-stdin handling, encoding handling, and (for `lint`) at least one workflow per rule category. Source lives in `src/cli/examples.rs`.
- `--list-rules` flag on `sqlt lint`. Prints a sortable table of every registered rule with id, slug, category, default severity, default-enabled state, and one-line summary, plus a footer counting total / on / off. Exits 0 without parsing input.
- Greatly expanded long-help text on every subcommand and flag. Each flag now documents its accepted values, default, and interaction with other flags. The terse short-help (`-h`) is unchanged; the full long-help (`--help`) is the new in-depth surface.

### Removed
- `src/dialect/mariadb.rs` (the `MariaDbDialect` wrapper struct). It was the source of the silent feature-flag disablement above. MariaDB-specific parser tweaks now live in `src/parse/mod.rs::preprocess_mariadb`.

## [0.2.0] - 2026-05-02

### Added
- `--encoding`/`-e` flag on `parse`, `emit`, and `translate` for non-UTF-8 input/output. Supported values: `utf-8` (default), `iso-8859-1` (alias `latin1`), `windows-1252` (alias `cp1252`). Decoding is strict — invalid byte sequences are rejected with exit code 1 rather than substituted with `U+FFFD`. JSON I/O is always UTF-8 (per spec); the flag governs SQL bytes only.
- `Encoding` type in `src/encoding.rs` wrapping `encoding_rs` with strict decode/encode semantics and aliases.
- CLI tests for Latin-1 round-trip (high-bit byte preservation through `translate`), default-mode rejection of non-UTF-8 input, and unknown-encoding exit code.
- `sqlt lint` subcommand with 38 active rules across 8 categories: raw passthrough, dialect cross-contamination, translation pre-flight (driven by `dialect/caps.rs` when `--to` is set), join hygiene, subquery improvements, performance pitfalls, correctness pitfalls, style/readability, and DDL hygiene. Every rule has a stable id (`SQLT0500`), a slug (`select-star`), inline summary + long-form explanation accessible via `sqlt lint --explain`, and per-rule fixtures under `tests/fixtures/lint/`.
- Four lint output formats: `text` (default, single-line grep-friendly), `pretty` (grouped per file with snippet pointer and inline rule explanation on first occurrence), `json`, and SARIF 2.1.0 for GitHub code-scanning integration.
- Lint CLI flags: `--rule`/`--no-rule` (repeatable, accept full id / short numeric / slug), `--severity` (output filter), `--exit-on` (exit-code threshold, default `error`), `--explain <id>` (print rule docs and exit 0).
- Lint architecture: `Rule` trait whose implementors register only the AST-shape callbacks they need; a shared driver in `src/lint/walk.rs` does one traversal per statement (sqlparser's `Visitor` doesn't fire on `Select`, so the driver does manual descent into `SetExpr::Select`/`TableWithJoins`). `check_query` receives a depth parameter so rules like `SQLT0403 order-by-in-subquery-without-limit` only fire on nested queries.
- `tests/lint.rs` per-rule fixture walker plus `insta` snapshot tests for json/sarif output.

## [0.1.0] - 2026-05-02

### Added
- Initial project scaffolding: Cargo manifest, module layout, conventions documentation.
- `sqlt parse --from <dialect> [--pretty] [file|-]` subcommand. Reads SQL from a file or stdin and emits a JSON envelope `{ sqlt_version, dialect, statements }` using the upstream sqlparser AST's serde representation.
- Dialects supported by `parse`: `mysql`, `postgres` (aliases `postgresql`, `pg`), `mssql` (aliases `tsql`, `sqlserver`), `sqlite`, `generic`.
- Smoke parse fixtures under `tests/fixtures/<dialect>/*.sql` covering `SELECT`, `INSERT`, `RETURNING`, `ON CONFLICT`, `TOP`, and bracketed identifiers.
- `sqlt emit --to <dialect> [file|-]` subcommand. Reads a JSON envelope and emits SQL using the upstream sqlparser `Display` impls (per-dialect overrides land later as round-trip tests find infidelities).
- Round-trip integration suite (`tests/roundtrip.rs`) asserting `parse → emit → parse` produces an identical AST for every fixture across mysql/postgres/mssql/sqlite, plus JSON serde round-trip equivalence.
- MariaDB dialect (`mariadb`, alias `maria`) as a first-class target. `MariaDbDialect` wraps the upstream `MySqlDialect` and the parser falls back to a raw-passthrough representation (`SqltStatement::Raw { sqlt_raw, reason }`) for MariaDB-specific syntax with no upstream AST node — `WITH SYSTEM VERSIONING`, `FOR SYSTEM_TIME`, `CREATE PACKAGE`, MariaDB sequence option ordering, vector types. Same-dialect round-trip (parse → emit) preserves the original SQL verbatim for these.
- `SqltStatement` enum (`Std(Box<Statement>) | Raw(RawStatement)`) with `#[serde(untagged)]` so the JSON wire format for typed statements is unchanged.
- Heuristic statement splitter (`parse::split`) used by the MariaDB fallback path. Respects single/double quotes, backticks, and line/block comments.

- `sqlt translate --from <src> --to <dst> [--strict] [file|-]` subcommand. Parses the input, rewrites the AST against the target dialect's capability table, and emits SQL. Warnings are printed to stderr; `--strict` makes any warning a non-zero exit (code 3).
- Per-dialect capability tables (`dialect/caps.rs`) covering `RETURNING` on INSERT/UPDATE/DELETE, `CREATE SEQUENCE`, `ON DUPLICATE KEY UPDATE`, `ON CONFLICT`, and MariaDB raw fallback support.
- Translation rewriter (`translate/rewrite.rs`) that drops `RETURNING` when the target lacks it, warns when `CREATE SEQUENCE` cannot be represented, and warns when a MariaDB raw fragment passes through to a non-MariaDB target.
- `WarnCode` enum (`RETURNING_DROPPED`, `SEQUENCE_DROPPED`, `ON_DUPLICATE_KEY_UNSUPPORTED`, `RAW_PASSTHROUGH`) with `WarnSink` trait, `StderrSink` for the CLI, and `CollectingSink` for golden tests.
- Golden translation test harness (`tests/translate.rs`) walking `tests/fixtures/translations/<src>__<dst>/<case>.{in.sql,expected.sql,expected.warn}`. Covers `mariadb→mysql` (RETURNING dropped), `mariadb→postgres` (RETURNING through, system-versioning passes raw with warning), `postgres→mariadb` (RETURNING through cleanly).
- End-to-end CLI integration suite (`tests/cli.rs`) exercising the built `sqlt` binary: parse→emit pipe round-trip, translate warning emission, `--strict` exit code 3, parse error exit code 1, unknown dialect exit code 2, and multi-statement input parsing.

### Changed
- The `statements` field of the JSON envelope is now `Vec<SqltStatement>` instead of `Vec<Statement>`. For typed statements the on-the-wire shape is unchanged thanks to `#[serde(untagged)]`; only raw fallback fragments introduce a new shape.

[Unreleased]: https://github.com/thomasbjork/sqlt/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/thomasbjork/sqlt/releases/tag/v0.3.0
[0.2.0]: https://github.com/thomasbjork/sqlt/releases/tag/v0.2.0
[0.1.0]: https://github.com/thomasbjork/sqlt/releases/tag/v0.1.0
