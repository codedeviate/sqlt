pub mod json;
pub mod pretty;
pub mod sarif;
pub mod text;

pub use text::HelpMode;

use crate::error::Result;
use crate::lint::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum Format {
    Text,
    Pretty,
    Json,
    Sarif,
}

pub fn render(
    format: Format,
    source: &str,
    source_text: &str,
    diagnostics: &[Diagnostic],
) -> Result<String> {
    render_with(format, source, source_text, diagnostics, HelpMode::Auto)
}

pub fn render_with(
    format: Format,
    source: &str,
    source_text: &str,
    diagnostics: &[Diagnostic],
    help: HelpMode,
) -> Result<String> {
    Ok(match format {
        Format::Text => text::render_with(source, diagnostics, help),
        Format::Pretty => pretty::render(source, source_text, diagnostics),
        Format::Json => json::render(source, diagnostics),
        Format::Sarif => sarif::render(source, diagnostics),
    })
}
