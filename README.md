# sqlt

Multi-dialect SQL parser and translator in Rust.

Parses SQL from MySQL, MariaDB, PostgreSQL, MSSQL (T-SQL), and SQLite into a JSON AST, emits SQL back from JSON, and translates between dialects.

> Status: **early development** — see `CHANGELOG.md` for what's shipped and `OUT-OF-SCOPE.md` for what isn't.

## Install

```bash
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
```

Reads from a file path or stdin (use `-` or omit the path).

### Dialects

`mysql`, `mariadb`, `postgres` (alias `postgresql`), `mssql` (alias `tsql`), `sqlite`, `generic`.

MariaDB is a first-class target — features that diverge from MySQL (`RETURNING` on DML, `CREATE SEQUENCE`, system versioning, `FOR SYSTEM_TIME`, Oracle-compat packages) are recognized rather than silently aliased.

### Translation gaps

When a construct has no faithful equivalent in the target dialect (e.g. translating `RETURNING` to MySQL), `sqlt translate` emits the closest equivalent and prints a warning to stderr. Pass `--strict` to make any warning a non-zero exit.

## Development

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

See `CLAUDE.md` for project conventions (semver, conventional commits, changelog, module map).

## License

Dual-licensed under MIT or Apache-2.0 at your option.
