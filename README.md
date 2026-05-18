# sqlt

[![GitHub](https://img.shields.io/badge/github-codedeviate%2Fsqlt-181717?logo=github)](https://github.com/codedeviate/sqlt)
[![Latest release](https://img.shields.io/badge/release-v0.3.2-blue)](https://github.com/codedeviate/sqlt/releases)
[![crates.io](https://img.shields.io/crates/v/sqlt?logo=rust&label=crates.io)](https://crates.io/crates/sqlt)
[![Homebrew](https://img.shields.io/badge/homebrew-codedeviate%2Fcli%2Fsqlt-fbb040?logo=homebrew)](https://github.com/codedeviate/homebrew-cli)
[![Rust edition 2024](https://img.shields.io/badge/rust-2024_edition_(MSRV_1.85)-dea584?logo=rust)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)

Multi-dialect SQL parser and translator in Rust.

Parses SQL from MySQL, MariaDB, PostgreSQL, MSSQL (T-SQL), and SQLite into a JSON AST, emits SQL back from JSON, and translates between dialects.

> Status: **early development** — see `CHANGELOG.md` for what's shipped and `OUT-OF-SCOPE.md` for what isn't.

## Install

```bash
# Homebrew (macOS / Linux) — via the codedeviate tap
brew install codedeviate/cli/sqlt

# crates.io
cargo install sqlt

# From a clone of this repo
cargo install --path .
```

## Usage

```bash
# Parse SQL → JSON AST
echo "SELECT 1" | sqlt parse --from mysql --pretty

# Emit JSON AST → SQL
sqlt parse --from postgres schema.sql | sqlt emit --to mysql

# Translate SQL between dialects
sqlt translate --from mariadb --to postgres input.sql

# Lint SQL for common pitfalls and improvement suggestions
sqlt lint --from mariadb schema.sql
sqlt lint --from mariadb --to postgres schema.sql        # adds translation pre-flight checks
sqlt lint --from mariadb --format json schema.sql        # JSON output
sqlt lint --from mariadb --format sarif schema.sql       # SARIF for GitHub code scanning
sqlt lint --explain SQLT0300                             # rule documentation
```

Reads from a file path or stdin (use `-` or omit the path).

### Dialects

`mysql`, `mariadb`, `postgres` (alias `postgresql`), `mssql` (alias `tsql`), `sqlite`, `generic`.

MariaDB is a first-class target — features that diverge from MySQL (`RETURNING` on DML, `CREATE SEQUENCE`, system versioning, `FOR SYSTEM_TIME`, Oracle-compat packages) are recognized rather than silently aliased.

### Non-UTF-8 input

Pass `--encoding`/`-e` to read or write SQL in a non-UTF-8 code page. Useful when one of your systems still emits ISO-8859-1 / Latin-1.

```bash
sqlt parse     --from mysql -e latin1 export.sql        # decode latin1, emit UTF-8 JSON
sqlt translate --from mysql --to mariadb -e latin1 in.sql > out.sql   # latin1 in and out
sqlt emit      --to mysql -e windows-1252 tree.json     # JSON (UTF-8) -> windows-1252 SQL
```

Supported encodings: `utf-8` (default), `iso-8859-1` / `latin1`, `windows-1252` / `cp1252`. Decoding is strict — invalid bytes for the declared encoding are rejected with exit 1 rather than silently replaced.

### Translation gaps

When a construct has no faithful equivalent in the target dialect (e.g. translating `RETURNING` to MySQL), `sqlt translate` emits the closest equivalent and prints a warning to stderr. Pass `--strict` to make any warning a non-zero exit.

### Lint

`sqlt lint` runs ~38 rules across seven categories: dialect cross-contamination, translation pre-flight (when `--to` is set), join hygiene, subquery improvements, performance pitfalls, correctness pitfalls, style/readability, and DDL hygiene. Every rule has a stable id (`SQLT0500`), a slug (`select-star`), and inline documentation (`sqlt lint --explain SQLT0500`).

Common flags:

```
--from <dialect>          required; the source SQL dialect
--to   <dialect>          optional; enables translation pre-flight rules
--format text|pretty|json|sarif      output format (default text)
--rule <id>...            enable a specific rule (repeatable)
--no-rule <id>...         disable a specific rule (repeatable)
--severity error|warning|info        filter the output (rules still run)
--exit-on  error|warning|info        controls exit code 1 (default error)
--explain <id>            print rule documentation and exit 0
-e, --encoding            same encoding handling as parse/translate
```

Exit codes match the rest of the CLI: `0` clean, `1` lint findings at or above `--exit-on` (or a parse error), `2` usage error, `3` reserved.

## Development

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

See `CLAUDE.md` for project conventions (semver, conventional commits, changelog, module map).

## License

MIT — see [`LICENSE`](LICENSE).
