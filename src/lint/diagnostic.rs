use sqlparser::tokenizer::Span;

use crate::dialect::DialectId;
use crate::lint::rule::{RuleId, Severity};

/// One finding from a rule. Carries the most specific span the rule could
/// attach; `Span::empty()` is allowed and the formatter falls back to the
/// owning statement's span (cached on `LintCtx`).
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub rule: RuleId,
    pub rule_name: &'static str,
    pub category: crate::lint::rule::Category,
    pub severity: Severity,
    pub message: String,
    pub suggestion: Option<String>,
    pub span: Span,
    pub stmt_index: usize,
    pub source_dialect: DialectId,
    pub target_dialect: Option<DialectId>,
}

impl Diagnostic {
    pub fn has_span(&self) -> bool {
        let s = &self.span;
        !(s.start.line == 0 && s.start.column == 0 && s.end.line == 0 && s.end.column == 0)
    }
}
