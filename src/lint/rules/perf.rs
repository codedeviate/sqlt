use sqlparser::ast::{Select, SelectItem};
use sqlparser::tokenizer::Span;

use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Category, Rule, RuleId, RuleMeta, Severity};

pub struct SelectStar;

const META_SELECT_STAR: RuleMeta = RuleMeta {
    id: RuleId("SQLT0500"),
    name: "select-star",
    category: Category::Perf,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "`SELECT *` returns every column, including ones the query does not need.",
    explanation: "Selecting all columns ships unused data over the wire, blocks covering-index plans, \
                  and silently picks up new columns when the schema evolves. Enumerate the columns \
                  you actually use. Qualified wildcards (`t.*`) after a join are exempt — see SQLT0501. \
                  Wildcards narrowed by EXCEPT/EXCLUDE/REPLACE/RENAME also do not fire this rule.",
};

impl Rule for SelectStar {
    fn meta(&self) -> &'static RuleMeta {
        &META_SELECT_STAR
    }

    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        for item in &select.projection {
            if let SelectItem::Wildcard(opts) = item {
                // If the wildcard is narrowed by EXCEPT/EXCLUDE/REPLACE/RENAME/ILIKE,
                // the user has clearly thought about the projection — skip.
                if opts.opt_except.is_some()
                    || opts.opt_exclude.is_some()
                    || opts.opt_replace.is_some()
                    || opts.opt_rename.is_some()
                    || opts.opt_ilike.is_some()
                {
                    continue;
                }
                let span = wildcard_span(opts).unwrap_or(ctx.stmt_span);
                out.push(Diagnostic {
                    rule: META_SELECT_STAR.id,
                    rule_name: META_SELECT_STAR.name,
                    category: META_SELECT_STAR.category,
                    severity: META_SELECT_STAR.default_severity,
                    message:
                        "SELECT * returns every column, including ones the query does not need"
                            .into(),
                    suggestion: Some("enumerate the columns the query actually uses".into()),
                    span,
                    stmt_index: ctx.stmt_index,
                    source_dialect: ctx.src,
                    target_dialect: ctx.dst,
                });
            }
        }
    }
}

fn wildcard_span(opts: &sqlparser::ast::WildcardAdditionalOptions) -> Option<Span> {
    let token = opts.wildcard_token.0.clone();
    if token.span.start.line == 0 && token.span.start.column == 0 {
        None
    } else {
        Some(token.span)
    }
}
