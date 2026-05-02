pub mod ctx;
pub mod diagnostic;
pub mod format;
pub mod registry;
pub mod rule;
pub mod rules;
pub mod walk;

pub use diagnostic::Diagnostic;
pub use rule::{Category, Rule, RuleId, RuleMeta, Severity};

use crate::ast::SqltStatement;
use crate::dialect::DialectId;
use crate::error::Result;
use crate::lint::ctx::LintCtx;

#[derive(Debug, Clone, Default)]
pub struct LintOptions {
    pub enable: Vec<String>,
    pub disable: Vec<String>,
}

/// Run the configured rules over a parsed batch.
pub fn lint(
    stmts: &[SqltStatement],
    source_text: &str,
    src: DialectId,
    dst: Option<DialectId>,
    opts: &LintOptions,
) -> Result<Vec<Diagnostic>> {
    let rules = registry::select_rules(&opts.enable, &opts.disable)?;
    let mut diagnostics = Vec::new();
    for (i, stmt) in stmts.iter().enumerate() {
        let stmt_span = match stmt {
            SqltStatement::Std(boxed) => walk::statement_span(boxed),
            SqltStatement::Raw(r) => match r.start_line {
                Some(line) => sqlparser::tokenizer::Span::new(
                    sqlparser::tokenizer::Location { line, column: 1 },
                    sqlparser::tokenizer::Location { line, column: 1 },
                ),
                None => sqlparser::tokenizer::Span::empty(),
            },
        };
        let ctx = LintCtx {
            src,
            dst,
            stmt_index: i,
            stmt_span,
            source_text,
        };
        walk::walk_statement(stmt, &rules, &ctx, &mut diagnostics);
    }
    Ok(diagnostics)
}

/// Sort diagnostics deterministically: by stmt_index, then span start, then rule id.
pub fn sort(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        (a.stmt_index, a.span.start.line, a.span.start.column, a.rule).cmp(&(
            b.stmt_index,
            b.span.start.line,
            b.span.start.column,
            b.rule,
        ))
    });
}
