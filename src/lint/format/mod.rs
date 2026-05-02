pub mod text;

use crate::error::Result;
use crate::lint::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum Format {
    Text,
}

pub fn render(format: Format, source: &str, diagnostics: &[Diagnostic]) -> Result<String> {
    match format {
        Format::Text => Ok(text::render(source, diagnostics)),
    }
}
