# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project scaffolding: Cargo manifest, module layout, conventions documentation.
- `sqlt parse --from <dialect> [--pretty] [file|-]` subcommand. Reads SQL from a file or stdin and emits a JSON envelope `{ sqlt_version, dialect, statements }` using the upstream sqlparser AST's serde representation.
- Dialects supported by `parse`: `mysql`, `postgres` (aliases `postgresql`, `pg`), `mssql` (aliases `tsql`, `sqlserver`), `sqlite`, `generic`.
- Smoke parse fixtures under `tests/fixtures/<dialect>/*.sql` covering `SELECT`, `INSERT`, `RETURNING`, `ON CONFLICT`, `TOP`, and bracketed identifiers.

[Unreleased]: https://github.com/thomasbjork/sqlt/compare/HEAD...HEAD
