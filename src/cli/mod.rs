pub mod emit;
pub mod examples;
pub mod lint;
pub mod parse;
pub mod translate;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::dialect::DialectId;
use crate::encoding::Encoding;
use crate::error::Result;
use crate::lint::format::Format as LintFormat;

/// `sqlt` — multi-dialect SQL parser, translator, and linter.
///
/// Supports MySQL, MariaDB (first-class, not aliased to MySQL), PostgreSQL,
/// MSSQL (T-SQL), SQLite, and a permissive Generic dialect. Reads SQL from
/// a file or stdin, decodes any of utf-8 / iso-8859-1 / windows-1252.
///
/// Pass `--examples` to any subcommand for in-depth examples, e.g.
/// `sqlt parse --examples`.
#[derive(Debug, Parser)]
#[command(
    name = "sqlt",
    version,
    about = "Multi-dialect SQL parser, translator, and linter",
    long_about = "Multi-dialect SQL parser, translator, and linter.\n\n\
                  Supports MySQL, MariaDB (first-class), PostgreSQL, MSSQL (T-SQL), \
                  SQLite, and a Generic fallback dialect. Reads from a file or stdin, \
                  decodes any of utf-8 / iso-8859-1 / windows-1252.\n\n\
                  Run `sqlt <SUBCOMMAND> --examples` for in-depth usage of any subcommand."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
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
    ///
    /// Runs ~38 lint rules across 8 categories: dialect cross-contamination,
    /// translation pre-flight (when `--to` is set), join hygiene, subquery
    /// improvements, performance pitfalls, correctness pitfalls, style, DDL
    /// hygiene. Every rule has a stable id (e.g. `SQLT0500`), a short slug
    /// (`select-star`), and inline documentation accessible via
    /// `--explain`.
    ///
    /// See `sqlt lint --examples` for in-depth examples and
    /// `sqlt lint --list-rules` for the full ruleset.
    Lint(LintArgs),
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

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Parse(args) => parse::run(args),
        Command::Emit(args) => emit::run(args),
        Command::Translate(args) => translate::run(args),
        Command::Lint(args) => lint::run(args),
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
