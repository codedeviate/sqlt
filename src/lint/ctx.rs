use sqlparser::tokenizer::Span;

use crate::dialect::DialectId;

/// Per-statement context passed to every rule callback.
pub struct LintCtx<'a> {
    pub src: DialectId,
    pub dst: Option<DialectId>,
    pub stmt_index: usize,
    /// Best-effort span covering the whole statement; used as a fallback
    /// when a rule emits a diagnostic on a node whose own `.span()` is
    /// `Span::empty()`. May itself be empty for statements whose
    /// `Spanned` impl is unimplemented upstream (Drop/Set/Comment/...).
    pub stmt_span: Span,
    /// Original source text — formatters use this for snippet rendering.
    pub source_text: &'a str,
}
