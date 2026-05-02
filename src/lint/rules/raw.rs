//! SQLT0001 — diagnostics for `SqltStatement::Raw` fragments.

use crate::ast::SqltStatement;
use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Category, Rule, RuleId, RuleMeta, Severity};

pub struct RawPassthrough;

const META_RAW: RuleMeta = RuleMeta {
    id: RuleId("SQLT0001"),
    name: "raw-passthrough",
    category: Category::Raw,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "A statement fell back to raw passthrough — no typed AST in sqlparser 0.59.",
    explanation: "MariaDB constructs that don't yet have a typed sqlparser node — system \
                  versioning, FOR SYSTEM_TIME, CREATE PACKAGE, MariaDB sequence option ordering, \
                  vector types — are kept as raw text. Same-dialect round-trip preserves them; \
                  cross-dialect translation will reject them with a RAW_PASSTHROUGH warning. The \
                  `reason` field on the parsed JSON envelope identifies which class of construct \
                  triggered the fallback.",
};

impl Rule for RawPassthrough {
    fn meta(&self) -> &'static RuleMeta {
        &META_RAW
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if let SqltStatement::Raw(r) = stmt {
            out.push(Diagnostic {
                rule: META_RAW.id,
                rule_name: META_RAW.name,
                category: META_RAW.category,
                severity: META_RAW.default_severity,
                message: format!(
                    "raw {} fragment — no typed AST representation in sqlparser 0.59",
                    r.reason
                ),
                suggestion: Some(
                    "ensure the construct survives the round-trip you intend; cross-dialect translation will reject it"
                        .into(),
                ),
                span: ctx.stmt_span,
                stmt_index: ctx.stmt_index,
                source_dialect: ctx.src,
                target_dialect: ctx.dst,
            });
        }
    }
}
