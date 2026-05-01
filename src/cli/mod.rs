pub mod parse;

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

fn parse_dialect(s: &str) -> std::result::Result<DialectId, String> {
    s.parse::<DialectId>().map_err(|e| e.to_string())
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Parse(args) => parse::run(args),
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
