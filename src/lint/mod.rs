pub mod ctx;
pub mod diagnostic;
pub mod format;
pub mod registry;
pub mod rule;
pub mod rules;
pub mod schema;
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
///
/// `external_schema` is an optionally pre-built schema (typically loaded
/// from `--schema <file>` arguments). When `Some`, it is used as the base
/// and the input's CREATE TABLE statements augment it via `apply_statement`
/// — preserving the v0.3 behaviour where inline CREATE TABLEs in the
/// query file still feed the schema model. When `None`, behaviour is
/// identical to today: the schema is built from the input's CREATE TABLE
/// statements only.
pub fn lint(
    stmts: &[SqltStatement],
    source_text: &str,
    src: DialectId,
    dst: Option<DialectId>,
    opts: &LintOptions,
    external_schema: Option<schema::Schema>,
) -> Result<Vec<Diagnostic>> {
    let rules = registry::select_rules(&opts.enable, &opts.disable)?;
    let schema = match external_schema {
        Some(mut s) => {
            // Augment with CREATE TABLEs from the lint input itself, so a
            // user with both --schema and inline CREATE TABLE in queries
            // gets the union.
            let synthetic = std::path::Path::new("<input>");
            let mut throwaway_skips = Vec::new();
            for stmt in stmts {
                if let SqltStatement::Std(boxed) = stmt
                    && matches!(&**boxed, sqlparser::ast::Statement::CreateTable(_))
                {
                    s.apply_statement(stmt, synthetic, &mut throwaway_skips);
                }
            }
            s
        }
        None => schema::Schema::from_statements(stmts),
    };
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
            schema: &schema,
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
