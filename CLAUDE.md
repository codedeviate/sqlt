# sqlt — Project guide for Claude

Multi-dialect SQL parser and translator. Parses SQL from MySQL, MariaDB, PostgreSQL, MSSQL (T-SQL), and SQLite into a JSON AST, emits SQL back from JSON, and translates between dialects.

## Build & test

```bash
cargo build                       # debug build
cargo build --release             # release build (always run alongside debug)
cargo run -- parse --from mysql   # run the binary
cargo test                        # run all tests (unit + integration)
cargo clippy -- -D warnings       # lint (fail on warnings)
cargo fmt --check                 # format check
```

**Always build a release version at the same time as a debug version.** Whenever you run `cargo build`, also run `cargo build --release` (and likewise for any per-milestone build verification). Release builds catch optimizer-only issues, surface real binary sizes, and keep `target/release/sqlt` in sync with what's on the development branch.

MSRV: Rust 1.88 (edition 2024).

## README badges

The header in `README.md` carries a fixed set of badges: GitHub repo, latest release, crates.io, Homebrew tap, Rust edition / MSRV, and license. Keep them current whenever the underlying fact changes — and in the same commit:

- **Release badge** (`release-vX.Y.Z-blue`): bump every time `Cargo.toml`'s `version` changes. Pair this with the release commit, not as a follow-up.
- **Rust edition / MSRV badge**: update if `edition` or `rust-version` in `Cargo.toml` changes (also update the "MSRV: Rust …" line above).
- **License badge**: update if the `license` field in `Cargo.toml` changes.
- **crates.io / Homebrew / GitHub badges**: only change if the crate name, tap path, or repo slug changes.

If you bump the version, update the badge in the same commit. Do not rely on shields.io's dynamic endpoints here — the badges are intentionally static so the README renders correctly on crates.io and in offline mirrors.

## CLI shape

Single binary `sqlt` with these subcommands:

- `sqlt parse --from <dialect> [--pretty] [-e <encoding>] [file|-]` — SQL → JSON AST.
- `sqlt emit --to <dialect> [-e <encoding>] [file|-]` — JSON AST → SQL.
- `sqlt translate --from <src> --to <dst> [--strict] [-e <encoding>] [file|-]` — SQL → SQL via AST.
- `sqlt lint --from <src> [--to <dst>] [--format …] [--rule …] [--no-rule …] [--severity …] [--exit-on …] [--explain <id>] [--list-rules] [-v] [-e <encoding>] [file|-]` — analyze SQL for pitfalls and suggest improvements.
- `sqlt build-schema --from <dialect> --schema <file>... [-o <path>] [--pretty] [-e <encoding>]` — compile a reusable schema artifact.

Every subcommand accepts `--examples` (prints in-depth usage and exits) and `--help` (full long-form docs). The top level (`sqlt --examples`, `sqlt --help`) shows a cross-cutting overview.

A full system man page is shipped in `man/sqlt.1` (groff/troff format) and is installed by the Homebrew tap (`#{man1}/sqlt.1`), so users get `man sqlt` after `brew install codedeviate/cli/sqlt`. The crate tarball includes `man/sqlt.1` automatically (it is NOT in the `Cargo.toml` `exclude` list).

### Help & examples maintenance rule (mandatory)

**Whenever you add or change a flag, subcommand, or user-visible behavior, update both the clap doc comment that drives `--help` AND the matching block in `src/cli/examples.rs` that drives `--examples`.** This is non-negotiable — out-of-date help is the most common source of friction the user has flagged. Both surfaces must describe the new state in the same commit.

The two surfaces are independent:

- **`--help`** is auto-generated from clap doc comments. The standards:
  - Every `LintArgs` / `BuildSchemaArgs` / etc. field needs a `///` comment that describes (a) what the flag does, (b) accepted values, (c) the default, (d) interactions with other flags.
  - Every `Command::*` variant needs either a multi-paragraph `///` comment OR a `#[command(long_about = SOMETHING_LONG_ABOUT)]` reference to a `const &str` at the top of `src/cli/mod.rs`. **Do not rely on a one-liner `///` summary** — clap's top-level `Commands:` table only shows the first line, and that's exactly the failure the user reported on `build-schema`. Subcommand descriptions in the table can be terse, but `<command> --help` must always read like documentation.
  - The first sentence of any `///` is the short help (`-h`); everything after is the long help (`--help`).
  - For top-level prose (long_about on `Cli`), use a `const TOP_LEVEL_LONG_ABOUT: &str` and pull it in via `long_about = …`. Same pattern for any subcommand whose long_about is more than a few lines.

- **`--examples`** is the in-depth manual. The standards:
  - Lives in `src/cli/examples.rs` as a `pub const <NAME>: &str`. There must be a constant for every subcommand (`PARSE`, `EMIT`, `TRANSLATE`, `LINT`, `BUILD_SCHEMA`, …) AND a top-level `TOP_LEVEL` that gives a cross-cutting overview.
  - Every example constant must contain: (1) a "Flag reference" section that documents every flag the subcommand accepts, including default values and interactions, (2) a "Common workflows" section with example invocations of every commonly-used flag combination, (3) edge cases (encoding, multi-file, stdin, large input, etc.), (4) exit codes if non-trivial.
  - Every flag mentioned in `--help` must appear at least once in the matching `--examples` constant. If you add a flag but only update `--help`, you've broken this rule.
  - Both `sqlt --examples` and `sqlt <COMMAND> --examples` must work. The top-level form is implemented by treating `--examples` as a flag on `Cli` (not a subcommand) and short-circuiting in `cli::run`.

- **`lint --list-rules`** is auto-generated from the rule registry; no maintenance needed.
- **`lint --explain <ID>`** reads `RuleMeta.summary` and `RuleMeta.explanation` from each rule's source file. When introducing a rule (especially a heuristic or schema-blind one), `explanation` MUST call out false-positive risk.

**Update order whenever a change touches user-facing behavior:**
1. The field's `///` doc comment (or the `<COMMAND>_LONG_ABOUT` constant if multi-paragraph).
2. The matching `src/cli/examples.rs` constant — including the Flag reference section.
3. `TOP_LEVEL_LONG_ABOUT` if the change introduces a new discoverability surface (a new subcommand, a new top-level flag like `--examples`).
4. **`man/sqlt.1`** — the installed system man page (rendered by `man sqlt` after `brew install codedeviate/cli/sqlt`) is the single combined reference for every flag, dialect, encoding, exit code, environment variable, lint rule category, and translation warning code. Every user-visible change MUST be reflected here in the same commit. The exact sections it owns: `NAME`, `SYNOPSIS`, `DESCRIPTION`, `COMMANDS`, `DIALECTS`, `ENCODINGS`, `LINT RULE CATEGORIES`, `TRANSLATION WARNINGS`, `EXAMPLES`, `EXIT STATUS`, `FILES`, `ENVIRONMENT`, `SEE ALSO`, `BUGS`, `AUTHOR`. Also bump the `.TH` header's date and version when releasing. Verify changes render by running `man -P cat man/sqlt.1` before committing — bad roff (mismatched `.RS`/`.RE`, missing `.fi` after `.nf`) produces silently broken output.
5. README's relevant section if user-facing.
6. `CHANGELOG.md`.

Failing to do (1), (2), and (4) together is grounds for a follow-up commit before the change is considered done.

### Colorized `--examples` (mandatory)

The `--examples` output goes through `src/cli/style.rs::print_colored`, which applies recon-style ANSI styling to the plain-text constants in `src/cli/examples.rs` line-by-line:

- The first non-blank line of each constant is the **title** (bold). Every `--examples` constant must therefore start with a one-line title.
- Lines made entirely of `─` are dividers (dimmed). Section headers (`Flag reference`, `Common workflows`, …) belong between two divider lines so they pick up the yellow-bold treatment.
- Shell comments inside example blocks must start with `# ` and be indented ≥ 2 spaces (`  # From a file`). These render green.
- Command lines must start with one of: `sqlt`, `echo`, `printf`, `brew`, `cargo`, `jq`, `cp`, `mv`, `git` (after ≥ 2 leading spaces). These render cyan. Continuation lines (≥ 4 spaces, inside the same block) inherit the cyan treatment until a blank line resets the block.
- Lines starting with `note:` are notes (dimmed).
- Column-0 short lines ending with `:` become sub-headings (bold).

When you add a new top-level command name to `KEYWORDS` in `style.rs`, do it in the same commit that adds the example. The colorizer is intentionally text-pattern-based — if you change the section divider style or invent a new convention, update `print_colored` first.

**Encoding rules:**
- `--encoding`/`-e` defaults to `utf-8`. Aliases: `latin1` / `iso-8859-1`, `cp1252` / `windows-1252`.
- For `parse`: `--encoding` decodes the input bytes; JSON output is always UTF-8 (per spec).
- For `emit`: JSON input is always UTF-8; `--encoding` selects the SQL output encoding.
- For `translate`: `--encoding` applies to both input and output bytes (round-trip stays in the same code page).
- Decoding is strict — invalid byte sequences are rejected with exit code 1, not silently replaced with `U+FFFD`. Don't change that default.

Dialects: `mysql`, `mariadb`, `postgres` (alias `postgresql`), `mssql` (alias `tsql`), `sqlite`, `generic`.

Reading input: positional path, or `-` / omitted for stdin.

Exit codes: `0` ok, `1` parse error, `2` usage error, `3` strict-warning failure (only when `--strict`).

## Module map

```
src/
├── main.rs                     # binary entry → cli::run()
├── lib.rs                      # re-exports for integration tests
├── error.rs                    # thiserror Error type
├── cli/{mod,parse,emit,translate,lint,build_schema,examples,style}.rs
├── dialect/
│   ├── mod.rs                  # DialectId enum + FromStr + upstream() factory
│   ├── mariadb.rs              # MariaDbDialect — wraps MySqlDialect
│   └── caps.rs                 # DialectCaps tables for translation gap detection
├── parse/mod.rs                # parse(sql, dialect) → Vec<Statement>
├── json/{mod,envelope}.rs      # serde_json with { sqlt_version, dialect, statements } envelope
├── emit/{mod,default,mysql,mariadb,postgres,mssql,sqlite}.rs
├── translate/{mod,rewrite,warn}.rs
└── lint/
    ├── {mod,rule,ctx,diagnostic,walk,registry}.rs
    ├── format/{mod,text,pretty,json,sarif}.rs
    └── rules/
        ├── raw.rs                   # SQLT0001
        ├── dialect_xc.rs            # SQLT01xx
        ├── pre_flight.rs            # SQLT02xx (--to driven)
        ├── joins.rs                 # SQLT03xx
        ├── subquery.rs              # SQLT04xx
        ├── perf.rs                  # SQLT05xx
        ├── correctness.rs           # SQLT06xx
        ├── style.rs                 # SQLT07xx
        └── ddl.rs                   # SQLT08xx
```

**Lint rule architecture rules:**
- Rule IDs are stable forever — never reuse a number, even after a rule is removed/deprecated.
- Each rule defines a `RuleMeta` const at module scope and a unit struct that implements `Rule`.
- The shared driver in `walk.rs` does the AST traversal; rules implement only the callbacks they need (`check_statement`, `check_query` (with depth), `check_select`, `check_expr`).
- `--rule` / `--no-rule` accept the full id (`SQLT0500`), short numeric (`0500`/`500`), or slug (`select-star`).
- Schema-blind heuristic rules (annotated ⚠ in the plan) MUST call out their false-positive risk in `RuleMeta.explanation`.
- Pre-flight rules (SQLT02xx) consult `dialect/caps.rs` rather than re-encoding capability tables.

## Architectural rules

1. **MariaDB is functionally distinct from MySQL — but at the parser-trait layer it uses `MySqlDialect`.** This is a deliberate compromise. We tried wrapping `MySqlDialect` in our own `MariaDbDialect` type but sqlparser uses `dialect_of!(MySqlDialect)` macros sprinkled through the parser that downcast via `Any::is::<MySqlDialect>()`. Any wrapper type fails those checks and silently disables MySQL-superset features that MariaDB needs (timestamp `ON UPDATE`, `LIMIT a, b`, `LOCK/UNLOCK TABLES`, `SET NAMES`, table hints). So `DialectId::MariaDb.upstream()` returns `MySqlDialect` directly. MariaDB-specific behaviour lives one layer up:
   - **`src/parse/mod.rs::preprocess_mariadb`** unwraps mariadb-dump conditional comments (`/*!NNN ... */`, `/*M!NNN ... */`) and injects a space after bare `--<EOL>`.
   - **`src/parse/mod.rs::mariadb_with_fallback`** runs only for `--from mariadb`. Splits the input on `;`, re-parses each piece, and wraps unparseable syntax as `SqltStatement::Raw` with a reason tag.
   - **`classify_mariadb_raw`** assigns reasons: `system_versioning`, `temporal_query`, `create_package`, `sequence_options`, `vector_type`, `delimiter`, `stored_program_body`, `optimization_hint`, `definer_clause`, `create_event`.
   - **`src/dialect/caps.rs::MARIADB`** has `returning_in_*: true`, `create_sequence: true`, `system_versioning: true`, `mariadb_raw_native: true` — MYSQL caps don't.
   - **Lint rules branch on `ctx.src == DialectId::MariaDb`** — `SQLT0104 returning-in-mysql` fires only for MySql, `SQLT0306 full-outer-in-mysql` fires for both, etc.

   Net effect: `--from mariadb` parses real `mariadb-dump` output (which `--from mysql` rejects), classifies its raw fragments differently, runs different rules, and translates differently. The shared parser is implementation detail, not user-visible behaviour.

2. **Emitters stay dumb. Translation lives in the rewriter.** Emitters take an AST and write SQL for one dialect — no semantic decisions. Cross-dialect rewriting (drop `RETURNING` for MySQL target, rewrite `ON DUPLICATE KEY UPDATE` ↔ `ON CONFLICT`) lives in `translate/rewrite.rs` and produces a target-valid AST before emission. This keeps round-trip tests honest.

3. **Capability tables drive translation.** `dialect/caps.rs` defines per-dialect `DialectCaps` consts. The rewriter compares src vs dst caps to decide what to drop, rewrite, or warn about.

4. **Best-effort with warnings.** When a construct can't be translated faithfully, emit closest equivalent and push a `Warning` to the `WarnSink`. Warnings go to stderr by default. `--strict` makes any warning a non-zero exit.

## Conventions

### Semantic Versioning
Start at `0.1.0`. While pre-1.0, breaking CLI/JSON-schema changes bump the minor. After 1.0, breaking → major.

### Conventional Commits
Required commit prefixes:
- `feat:` new user-visible feature
- `fix:` bug fix
- `chore:` tooling, deps, scaffolding
- `docs:` docs only
- `test:` tests only
- `refactor:` no behavior change
- `perf:` performance
- `build:` build system
- `ci:` CI config

Breaking changes: `feat!:` / `fix!:` or `BREAKING CHANGE:` in footer. Scope is optional — use it for targeted areas like `feat(mariadb): …`.

Examples:
```
feat(parse): add parse subcommand for mysql
feat(mariadb): support RETURNING on INSERT/UPDATE/DELETE
fix(emit): preserve backtick quoting for mysql identifiers
docs: document --strict flag in README
```

### Keep a Changelog
`CHANGELOG.md` follows [keepachangelog.com](https://keepachangelog.com/en/1.1.0/) format. Every user-visible change goes under `[Unreleased]` in the same commit. On release: move to a dated `[X.Y.Z] - YYYY-MM-DD` heading and tag `vX.Y.Z`.

Sections in order: Added / Changed / Deprecated / Removed / Fixed / Security.

### Tagging implies a GitHub release
When you push a `vX.Y.Z` tag, immediately create the matching GitHub release:

```sh
gh release create vX.Y.Z --generate-notes
```

A tag without a release leaves the GitHub Releases page out of sync with tag history and hides the version from anyone browsing the repo's front page. Treat the release as part of the tag — same step, no follow-up commits needed.

## Test layout

```
tests/
├── roundtrip.rs                # parse → emit → reparse → AST equal; JSON round-trip
├── translate.rs                # golden tests
└── fixtures/
    ├── {mysql,mariadb,postgres,mssql,sqlite}/*.sql
    └── translations/<src>__<dst>/<case>.{in.sql,expected.sql,expected.warn}
```

Round-trip is the correctness signal — if a dialect's `Display` infidelity breaks it, that's the trigger to add a faithful emitter in `emit/<dialect>.rs`.

## Out-of-scope

See `OUT-OF-SCOPE.md` for deferred items and known v1 limitations (notably the raw-SQL fallback for unrepresented MariaDB syntax).
