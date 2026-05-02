pub mod emit;
pub mod lint;
pub mod parse;
pub mod translate;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::dialect::DialectId;
use crate::encoding::Encoding;
use crate::error::Result;
use crate::lint::format::Format as LintFormat;

#[derive(Debug, Parser)]
#[command(
    name = "sqlt",
    version,
    about = "Multi-dialect SQL parser and translator"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Parse SQL into a JSON AST.
    Parse(ParseArgs),
    /// Emit SQL from a JSON AST.
    Emit(EmitArgs),
    /// Translate SQL between dialects.
    Translate(TranslateArgs),
    /// Lint SQL for common pitfalls and improvement suggestions.
    Lint(LintArgs),
}

#[derive(Debug, clap::Args)]
pub struct ParseArgs {
    /// Source SQL dialect.
    #[arg(long = "from", value_parser = parse_dialect)]
    pub from: DialectId,

    /// Pretty-print JSON output.
    #[arg(long)]
    pub pretty: bool,

    /// Encoding of the input bytes. JSON output is always UTF-8.
    #[arg(long, short = 'e', value_parser = parse_encoding, default_value = "utf-8")]
    pub encoding: Encoding,

    /// Input file (use `-` or omit for stdin).
    pub input: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct EmitArgs {
    /// Target SQL dialect. Defaults to the dialect recorded in the JSON envelope.
    #[arg(long = "to", value_parser = parse_dialect)]
    pub to: Option<DialectId>,

    /// Encoding of the SQL output bytes. JSON input is always read as UTF-8.
    #[arg(long, short = 'e', value_parser = parse_encoding, default_value = "utf-8")]
    pub encoding: Encoding,

    /// Input file (use `-` or omit for stdin).
    pub input: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct TranslateArgs {
    /// Source SQL dialect.
    #[arg(long = "from", value_parser = parse_dialect)]
    pub from: DialectId,

    /// Target SQL dialect.
    #[arg(long = "to", value_parser = parse_dialect)]
    pub to: DialectId,

    /// Treat translation warnings as errors (exit code 3).
    #[arg(long)]
    pub strict: bool,

    /// Encoding of the input and output SQL bytes.
    #[arg(long, short = 'e', value_parser = parse_encoding, default_value = "utf-8")]
    pub encoding: Encoding,

    /// Input file (use `-` or omit for stdin).
    pub input: Option<PathBuf>,
}

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

#[derive(Debug, clap::Args)]
pub struct LintArgs {
    /// Source SQL dialect (required unless --explain is given).
    #[arg(long = "from", value_parser = parse_dialect)]
    pub from: Option<DialectId>,

    /// Target SQL dialect (enables translation pre-flight rules).
    #[arg(long = "to", value_parser = parse_dialect)]
    pub to: Option<DialectId>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = LintFormat::Text)]
    pub format: LintFormat,

    /// Enable a rule (repeatable). Accepts SQLT0500, 0500, 500, or slug.
    #[arg(long = "rule")]
    pub rule: Vec<String>,

    /// Disable a rule (repeatable). Same id forms as --rule.
    #[arg(long = "no-rule")]
    pub no_rule: Vec<String>,

    /// Minimum severity to include in output (does not affect --exit-on).
    #[arg(long = "severity", value_enum, default_value_t = ExitOn::Info)]
    pub severity: ExitOn,

    /// Exit non-zero when any diagnostic is at or above this severity.
    #[arg(long = "exit-on", value_enum, default_value_t = ExitOn::Error)]
    pub exit_on: ExitOn,

    /// Print rule documentation and exit.
    #[arg(long = "explain")]
    pub explain: Option<String>,

    /// Encoding of the input bytes.
    #[arg(long, short = 'e', value_parser = parse_encoding, default_value = "utf-8")]
    pub encoding: Encoding,

    /// Input file (use `-` or omit for stdin).
    pub input: Option<PathBuf>,
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
