use sqlparser::tokenizer::Span;

use crate::dialect::DialectId;
use crate::lint::schema::Schema;

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
    /// Schema model derived from `CREATE TABLE` statements in the input.
    /// Empty when no CREATE TABLE was present. Rules consult this to
    /// refine schema-blind heuristics (NOT NULL info to suppress
    /// SQLT0505/SQLT0400, column-existence checks for SQLT0900).
    pub schema: &'a Schema,
}
