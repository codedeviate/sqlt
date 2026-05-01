pub mod emit;
pub mod parse;
pub mod translate;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::dialect::DialectId;
use crate::error::Result;

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
}

#[derive(Debug, clap::Args)]
pub struct ParseArgs {
    /// Source SQL dialect.
    #[arg(long = "from", value_parser = parse_dialect)]
    pub from: DialectId,

    /// Pretty-print JSON output.
    #[arg(long)]
    pub pretty: bool,

    /// Input file (use `-` or omit for stdin).
    pub input: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct EmitArgs {
    /// Target SQL dialect. Defaults to the dialect recorded in the JSON envelope.
    #[arg(long = "to", value_parser = parse_dialect)]
    pub to: Option<DialectId>,

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

    /// Input file (use `-` or omit for stdin).
    pub input: Option<PathBuf>,
}

fn parse_dialect(s: &str) -> std::result::Result<DialectId, String> {
    s.parse::<DialectId>().map_err(|e| e.to_string())
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Parse(args) => parse::run(args),
        Command::Emit(args) => emit::run(args),
        Command::Translate(args) => translate::run(args),
    }
}

pub(crate) fn read_input(path: Option<&std::path::Path>) -> Result<String> {
    use std::io::Read;
    let mut buf = String::new();
    match path {
        Some(p) if p.as_os_str() != "-" => {
            buf = std::fs::read_to_string(p)?;
        }
        _ => {
            std::io::stdin().read_to_string(&mut buf)?;
        }
    }
    Ok(buf)
}
