pub mod build_schema;
pub mod emit;
pub mod examples;
pub mod lint;
pub mod parse;
pub mod style;
pub mod translate;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::dialect::DialectId;
use crate::encoding::Encoding;
use crate::error::Result;
use crate::lint::format::Format as LintFormat;

const BUILD_SCHEMA_LONG_ABOUT: &str = "\
Compile a reusable schema artifact from one or more SQL files.

Reads each `--schema` file in CLI order, replays the DDL surface
(CREATE/ALTER/DROP TABLE, CREATE INDEX, foreign-key constraints,
CREATE DATABASE / USE for per-database namespacing), and emits a JSON
artifact that captures the *current* state of the schema — not just the
initial CREATE.

Use cases:
 - Compile a long migration history once, lint many times against the
   compiled artifact (cheap on every CI run).
 - Check the artifact into the repo so contributors lint against the
   same schema without each running the full migration replay.
 - Mix `.json` + late `.sql` migrations on top:
     sqlt lint --from mariadb \\
         --schema schema.json \\
         --schema migrations/late.sql query.sql

What's tracked:
 - tables (per database)
 - columns (name, data type, nullable)
 - indexes (named, unique, primary, fulltext, spatial; functional via
   the rendered SQL expression)
 - primary keys, foreign keys (resolved through the USE cursor)

Statements that don't affect the schema (INSERT, GRANT, DELIMITER +
stored procedure bodies, …) emit `note: skipping <kind> at <file>:<line>`
on stderr but never error.

The artifact records the sqlt version it was built with — `sqlt lint`
warns on major.minor mismatch but still tries to load.

For real-world examples (multi-database, latin1, late-migration mixing,
etc.) run:
   sqlt build-schema --examples";

const LINT_LONG_ABOUT: &str = "\
Analyze SQL for pitfalls and improvement suggestions.

Runs a curated ruleset over the parsed AST and reports diagnostics with
stable rule IDs (e.g. `SQLT0500`), short slugs (`select-star`), and inline
suggestions.

Rule categories:
 - raw          (SQLT00xx)  Raw passthrough (off by default; see -v)
 - dialect-xc   (SQLT01xx)  Dialect cross-contamination
 - pre-flight   (SQLT02xx)  Translation pre-flight (only with --to)
 - joins        (SQLT03xx)  Implicit cross joins, NATURAL JOIN, ON 1=1
 - subquery     (SQLT04xx)  IN (SELECT) -> EXISTS, correlated subqueries
 - perf         (SQLT05xx)  SELECT *, leading-wildcard LIKE, fn-on-column
 - correctness  (SQLT06xx)  = NULL, UPDATE/DELETE without WHERE
 - style        (SQLT07xx)  Unaliased derived tables, LIMIT without ORDER BY
 - ddl          (SQLT08xx)  Float-for-money, VARCHAR without length

Common discoverability flags:
 - sqlt lint --examples         in-depth usage examples
 - sqlt lint --list-rules       every registered rule with one-line summary
 - sqlt lint --explain <ID>     long-form documentation for a rule

Output formats (--format):
 - text     grep-friendly single-line per finding (default)
 - pretty   grouped per file with snippet pointer and inline rule explanation
 - json     structured for tooling / CI ingestion
 - sarif    SARIF 2.1.0 for GitHub code-scanning integration

Exit-code controls:
 - --exit-on error     (default) exit 1 only on errors
 - --exit-on warning   exit 1 on errors and warnings
 - --exit-on info      exit 1 on any finding
 - --severity is OUTPUT-only and does NOT affect --exit-on";

const TOP_LEVEL_LONG_ABOUT: &str = "\
Multi-dialect SQL parser, translator, and linter.

Supported dialects:
 - mysql                                MySQL 5.7+ / 8.0
 - mariadb                              MariaDB — see note below
 - postgres (aliases: postgresql, pg)   PostgreSQL
 - mssql    (aliases: tsql, sqlserver)  Microsoft SQL Server / T-SQL
 - sqlite                               SQLite
 - generic                              Permissive fallback dialect

About `--from mariadb`:
 At the parser layer MariaDB uses sqlparser's MySqlDialect (a wrapper
 fails the dialect_of!(MySqlDialect) downcast checks scattered through
 sqlparser, silently disabling MySQL-superset features MariaDB needs).
 MariaDB-specific behaviour lives one layer up:
   - input preprocessor unwraps mariadb-dump conditional comments
     (/*!NNN ... */, /*M!NNN ... */) and relaxes bare --<EOL>
   - per-statement fallback wraps unparseable MariaDB syntax as
     `Raw` fragments classified by reason (system_versioning,
     create_package, optimization_hint, delimiter, ...)
   - capability table treats MariaDB distinctly from MySQL for
     RETURNING, CREATE SEQUENCE, system versioning, etc.
   - lint rules can branch on the source dialect (e.g. SQLT0104
     fires for --from mysql but not --from mariadb)

Supported encodings (--encoding / -e):
 - utf-8         (default; always used for JSON I/O)
 - iso-8859-1    (alias: latin1)
 - windows-1252  (alias: cp1252, win1252)

Reads input from:
 - a file path (positional argument)
 - stdin (when no path is given, or when `-` is passed)

Discoverability:
 - sqlt --examples              top-level overview + per-command examples
 - sqlt <COMMAND> --help        full long-form help for any subcommand
 - sqlt <COMMAND> --examples    in-depth usage examples
 - man sqlt                     full system man page (installed by package)
 - sqlt lint --list-rules       every registered lint rule with id + summary
 - sqlt lint --explain <ID>     long-form documentation for one rule

Exit codes:
 - 0   clean
 - 1   parse error, encoding error, or lint findings >= --exit-on threshold
 - 2   usage error (unknown dialect, unknown rule, bad flag combination)
 - 3   `translate --strict` saw at least one warning";

/// `sqlt` — multi-dialect SQL parser, translator, and linter.
#[derive(Debug, Parser)]
#[command(
    name = "sqlt",
    version,
    about = "Multi-dialect SQL parser, translator, and linter",
    long_about = TOP_LEVEL_LONG_ABOUT,
    arg_required_else_help = false,
    subcommand_required = false,
)]
pub struct Cli {
    /// Print a top-level overview with examples covering every
    /// subcommand and the most common flag combinations, then exit.
    /// For per-subcommand examples use `sqlt <COMMAND> --examples`.
    #[arg(long = "examples", global = false)]
    pub examples: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Parse SQL into a JSON AST envelope.
    ///
    /// Reads SQL from a file path or stdin, parses it with the given source
    /// dialect, and writes a JSON envelope `{ sqlt_version, dialect,
    /// statements }` where `statements` is an array of typed sqlparser AST
    /// nodes (or raw-passthrough fragments for MariaDB-only syntax with no
    /// upstream node). JSON output is always UTF-8 per spec.
    ///
    /// See `sqlt parse --examples` for in-depth examples.
    Parse(ParseArgs),
    /// Emit SQL from a JSON AST envelope.
    ///
    /// Reads a JSON envelope (typically the output of `sqlt parse`) and
    /// writes SQL using the upstream sqlparser `Display` impls. Use the
    /// `--to` flag to override the dialect recorded in the envelope.
    ///
    /// See `sqlt emit --examples` for in-depth examples.
    Emit(EmitArgs),
    /// Translate SQL between dialects via the AST.
    ///
    /// Parses with `--from`, runs the per-dialect rewriter that converts
    /// source-only constructs into target equivalents (or warns when no
    /// equivalent exists), and emits SQL in `--to`. Warnings go to stderr;
    /// `--strict` makes any warning a non-zero exit (code 3).
    ///
    /// See `sqlt translate --examples` for in-depth examples.
    Translate(TranslateArgs),
    /// Analyze SQL for pitfalls and improvement suggestions.
    #[command(long_about = LINT_LONG_ABOUT)]
    Lint(LintArgs),
    /// Compile a reusable schema artifact from one or more SQL files.
    #[command(long_about = BUILD_SCHEMA_LONG_ABOUT)]
    BuildSchema(BuildSchemaArgs),
}

// ─────────────────────────────────────────────────────────────────────────────
// Parse subcommand
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, clap::Args)]
pub struct ParseArgs {
    /// Source SQL dialect.
    ///
    /// Accepted values: `mysql`, `mariadb` (alias `maria`), `postgres`
    /// (aliases `postgresql`, `pg`), `mssql` (aliases `tsql`, `sqlserver`),
    /// `sqlite`, `generic`.
    #[arg(long = "from", value_parser = parse_dialect, required_unless_present = "examples")]
    pub from: Option<DialectId>,

    /// Pretty-print the JSON output (indented for readability).
    ///
    /// Default is compact JSON on a single line. Pretty output is more
    /// useful for grepping or diffing; compact is better for piping to
    /// other tooling.
    #[arg(long)]
    pub pretty: bool,

    /// Encoding of the input bytes (file or stdin).
    ///
    /// Accepted values: `utf-8` (default), `iso-8859-1` (alias `latin1`),
    /// `windows-1252` (aliases `cp1252`, `win1252`). Decoding is strict —
    /// invalid byte sequences are rejected with exit code 1 rather than
    /// silently substituted with U+FFFD. JSON output is ALWAYS emitted as
    /// UTF-8 (per the JSON spec) regardless of this flag.
    #[arg(long, short = 'e', value_parser = parse_encoding, default_value = "utf-8")]
    pub encoding: Encoding,

    /// Input file path. Use `-` or omit to read from stdin.
    pub input: Option<PathBuf>,

    /// Print in-depth examples for this subcommand and exit.
    #[arg(long = "examples")]
    pub examples: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Emit subcommand
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, clap::Args)]
pub struct EmitArgs {
    /// Target SQL dialect. Defaults to the dialect recorded in the JSON
    /// envelope.
    ///
    /// Accepted values: same as `parse --from`. Override the envelope's
    /// dialect to render the same AST in a different dialect (note: this
    /// does not run translation rewrites — for that use
    /// `sqlt translate`).
    #[arg(long = "to", value_parser = parse_dialect)]
    pub to: Option<DialectId>,

    /// Encoding of the SQL output bytes.
    ///
    /// JSON input is always read as UTF-8. This flag selects how the SQL
    /// bytes are encoded on the way out, so you can write back to a
    /// Latin-1 system unchanged.
    #[arg(long, short = 'e', value_parser = parse_encoding, default_value = "utf-8")]
    pub encoding: Encoding,

    /// Input file path. Use `-` or omit to read from stdin.
    pub input: Option<PathBuf>,

    /// Print in-depth examples for this subcommand and exit.
    #[arg(long = "examples")]
    pub examples: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Translate subcommand
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, clap::Args)]
pub struct TranslateArgs {
    /// Source SQL dialect.
    ///
    /// Accepted values: same as `parse --from`.
    #[arg(long = "from", value_parser = parse_dialect, required_unless_present = "examples")]
    pub from: Option<DialectId>,

    /// Target SQL dialect.
    #[arg(long = "to", value_parser = parse_dialect, required_unless_present = "examples")]
    pub to: Option<DialectId>,

    /// Treat translation warnings as errors. Exits with code 3 if any
    /// warning was emitted (RETURNING_DROPPED, SEQUENCE_DROPPED, etc.).
    /// Useful in CI when you want a clean port or a hard failure.
    #[arg(long)]
    pub strict: bool,

    /// Encoding of both input and output SQL bytes. Same set as
    /// `parse --encoding`.
    #[arg(long, short = 'e', value_parser = parse_encoding, default_value = "utf-8")]
    pub encoding: Encoding,

    /// Input file path. Use `-` or omit to read from stdin.
    pub input: Option<PathBuf>,

    /// Print in-depth examples for this subcommand and exit.
    #[arg(long = "examples")]
    pub examples: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Lint subcommand
// ─────────────────────────────────────────────────────────────────────────────

fn parse_dialect(s: &str) -> std::result::Result<DialectId, String> {
    s.parse::<DialectId>().map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum ExitOn {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum CliHelpMode {
    Auto,
    Always,
    Never,
}

impl From<CliHelpMode> for crate::lint::format::HelpMode {
    fn from(m: CliHelpMode) -> Self {
        match m {
            CliHelpMode::Auto => Self::Auto,
            CliHelpMode::Always => Self::Always,
            CliHelpMode::Never => Self::Never,
        }
    }
}

#[derive(Debug, clap::Args)]
pub struct LintArgs {
    /// Source SQL dialect.
    ///
    /// Required for normal lint runs. Optional when `--explain`,
    /// `--examples`, or `--list-rules` is given (those don't parse input).
    #[arg(long = "from", value_parser = parse_dialect)]
    pub from: Option<DialectId>,

    /// Target SQL dialect (enables translation pre-flight rules SQLT02xx).
    ///
    /// When set, the linter additionally reports things that would break
    /// or be sub-optimal when this SQL is run against the target dialect
    /// (e.g. RETURNING dropped if target lacks it, ON DUPLICATE KEY rewrite
    /// hint when target supports ON CONFLICT instead).
    #[arg(long = "to", value_parser = parse_dialect)]
    pub to: Option<DialectId>,

    /// Output format.
    ///
    /// `text` (default): grep-friendly single-line per finding; `pretty`:
    /// grouped per file with snippet pointer and inline rule explanation
    /// on first occurrence; `json`: structured for tooling/CI; `sarif`:
    /// SARIF 2.1.0 for GitHub code-scanning.
    #[arg(long, value_enum, default_value_t = LintFormat::Text)]
    pub format: LintFormat,

    /// How to render `help:` lines in text format.
    ///
    /// `auto` (default): show once per (rule, suggestion) pair within a
    /// render — for SQLT0001 with hundreds of identical raw-passthrough
    /// findings the help shows once and the rest are implied. `always`:
    /// show under every diagnostic (legacy behaviour). `never`: suppress
    /// entirely.
    #[arg(long = "help-mode", value_enum, default_value_t = CliHelpMode::Auto)]
    pub help_mode: CliHelpMode,

    /// Shorthand for `--help-mode never`.
    #[arg(long = "no-help", conflicts_with = "help_mode")]
    pub no_help: bool,

    /// Show diagnostics from rules that are off by default — currently
    /// just SQLT0001 (raw-passthrough), which floods real `mariadb-dump`
    /// output with hundreds of identical warnings about
    /// optimization-hint / DELIMITER / DEFINER fragments sqlparser can't
    /// parse. Equivalent to passing `--rule SQLT0001`.
    #[arg(long = "verbose", short = 'v')]
    pub verbose: bool,

    /// Enable a rule (repeatable).
    ///
    /// Accepts the full id (`SQLT0500`), short numeric (`0500` or `500`),
    /// or slug (`select-star`). Use to opt in to default-off rules
    /// (SQLT0001, SQLT0501, SQLT0506) or to re-enable a rule disabled
    /// elsewhere.
    #[arg(long = "rule")]
    pub rule: Vec<String>,

    /// Disable a rule (repeatable). Same id forms as `--rule`.
    #[arg(long = "no-rule")]
    pub no_rule: Vec<String>,

    /// Minimum severity to include in output. Does NOT affect `--exit-on`
    /// — rules still run because their findings may be needed for the
    /// exit threshold.
    ///
    /// Values: `error`, `warning`, `info`. Default: `info` (everything
    /// shown).
    #[arg(long = "severity", value_enum, default_value_t = ExitOn::Info)]
    pub severity: ExitOn,

    /// Exit non-zero when any diagnostic is at or above this severity.
    ///
    /// Values: `error` (default), `warning`, `info`. Independent of
    /// `--severity` — the exit code reflects ALL diagnostics that ran,
    /// not just the ones shown.
    #[arg(long = "exit-on", value_enum, default_value_t = ExitOn::Error)]
    pub exit_on: ExitOn,

    /// Print rule documentation and exit.
    ///
    /// Accepts the full id (`SQLT0801`), short numeric (`0801`/`801`),
    /// or slug (`float-for-money`). The output includes the rule's
    /// summary, long-form explanation, default severity, and default
    /// enabled state.
    #[arg(long = "explain")]
    pub explain: Option<String>,

    /// List every registered rule with its id, slug, category, default
    /// severity, default-enabled state, and one-line summary. Exits 0
    /// without parsing any input.
    #[arg(long = "list-rules")]
    pub list_rules: bool,

    /// Schema input file (repeatable). Accepts .sql (parsed and replayed)
    /// or .json (a previously built artifact from `sqlt build-schema`).
    /// Files are processed in CLI order; the `USE` cursor and CREATE
    /// DATABASE state persist across files. Schema files are NOT linted —
    /// they only feed the schema model. Statements that don't affect the
    /// schema produce a stderr `note:` line.
    #[arg(long = "schema")]
    pub schemas: Vec<PathBuf>,

    /// Encoding of the input bytes. Same set as `parse --encoding`.
    #[arg(long, short = 'e', value_parser = parse_encoding, default_value = "utf-8")]
    pub encoding: Encoding,

    /// Input file path. Use `-` or omit to read from stdin.
    pub input: Option<PathBuf>,

    /// Print in-depth examples for this subcommand and exit.
    #[arg(long = "examples")]
    pub examples: bool,
}

fn parse_encoding(s: &str) -> std::result::Result<Encoding, String> {
    s.parse::<Encoding>().map_err(|e| e.to_string())
}

#[derive(Debug, clap::Args)]
pub struct BuildSchemaArgs {
    /// Source SQL dialect — parses every `--schema` file with this
    /// dialect. Same accepted values as `parse --from`. Required for
    /// normal runs; optional when `--examples` is given.
    #[arg(long = "from", value_parser = parse_dialect)]
    pub from: Option<DialectId>,

    /// Schema input file (repeatable). Each is parsed and replayed in CLI
    /// order; the `USE` cursor and CREATE DATABASE state persist across
    /// files. Files with a `.json` extension are loaded as a previously
    /// built artifact and merged in (so you can layer `.sql` migrations
    /// on top of a compiled `.json` base).
    #[arg(long = "schema")]
    pub schemas: Vec<PathBuf>,

    /// Encoding of the schema input bytes. Same accepted values as
    /// `parse --encoding`. JSON output is always written as UTF-8 per
    /// the JSON spec; this flag governs how `.sql` schema files are
    /// decoded.
    #[arg(long, short = 'e', value_parser = parse_encoding, default_value = "utf-8")]
    pub encoding: Encoding,

    /// Output file path for the JSON artifact. Omit to write to stdout.
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Pretty-print the JSON output (indented for readability and diff
    /// stability). Default is compact JSON on one line.
    #[arg(long)]
    pub pretty: bool,

    /// Print in-depth examples for this subcommand and exit.
    #[arg(long = "examples")]
    pub examples: bool,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    if cli.examples {
        examples::print(examples::TOP_LEVEL);
        return Ok(());
    }
    let Some(command) = cli.command else {
        // No subcommand and no `--examples` — replicate clap's default
        // help-on-empty behaviour. `clap::Command::print_help` would need
        // us to keep the `Command` instance around; the simplest path is
        // to re-invoke ourselves with `--help`.
        let mut cmd = <Cli as clap::CommandFactory>::command();
        cmd.print_help().ok();
        println!();
        return Ok(());
    };
    match command {
        Command::Parse(args) => parse::run(args),
        Command::Emit(args) => emit::run(args),
        Command::Translate(args) => translate::run(args),
        Command::Lint(args) => lint::run(args),
        Command::BuildSchema(args) => build_schema::run(args),
    }
}

/// Read raw input bytes (file or stdin).
pub(crate) fn read_input_bytes(path: Option<&std::path::Path>) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut buf = Vec::new();
    match path {
        Some(p) if p.as_os_str() != "-" => {
            buf = std::fs::read(p)?;
        }
        _ => {
            std::io::stdin().read_to_end(&mut buf)?;
        }
    }
    Ok(buf)
}

/// Read input and decode as the given encoding.
pub(crate) fn read_input_text(path: Option<&std::path::Path>, enc: Encoding) -> Result<String> {
    let bytes = read_input_bytes(path)?;
    enc.decode(&bytes)
}

/// Write SQL output. Encodes via `enc` and writes raw bytes to stdout so a
/// non-UTF-8 result is preserved when piped or redirected to a file.
pub(crate) fn write_sql(s: &str, enc: Encoding) -> Result<()> {
    use std::io::Write;
    let bytes = enc.encode(s)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    out.write_all(&bytes)?;
    out.write_all(b"\n")?;
    Ok(())
}
