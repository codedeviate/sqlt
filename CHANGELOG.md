# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `--encoding`/`-e` flag on `parse`, `emit`, and `translate` for non-UTF-8 input/output. Supported values: `utf-8` (default), `iso-8859-1` (alias `latin1`), `windows-1252` (alias `cp1252`). Decoding is strict — invalid byte sequences are rejected with exit code 1 rather than substituted with `U+FFFD`. JSON I/O is always UTF-8 (per spec); the flag governs SQL bytes only.
- `Encoding` type in `src/encoding.rs` wrapping `encoding_rs` with strict decode/encode semantics and aliases.
- CLI tests for Latin-1 round-trip (high-bit byte preservation through `translate`), default-mode rejection of non-UTF-8 input, and unknown-encoding exit code.

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

[Unreleased]: https://github.com/thomasbjork/sqlt/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/thomasbjork/sqlt/releases/tag/v0.1.0
