//! Style / readability rules.

use sqlparser::ast::{Query, Select, TableFactor};

use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Category, Rule, RuleId, RuleMeta, Severity};

// ───────────────────────────── SQLT0701 unaliased-derived-table ─────────────

pub struct UnaliasedDerivedTable;

const META_DERIVED_NO_ALIAS: RuleMeta = RuleMeta {
    id: RuleId("SQLT0701"),
    name: "unaliased-derived-table",
    category: Category::Style,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "A derived table without an alias is rejected by most dialects at runtime.",
    explanation: "Postgres and MSSQL require every subquery in FROM to have an alias. \
                  MySQL/MariaDB allow it but the resulting plan is hard to read. Add an alias.",
};

impl Rule for UnaliasedDerivedTable {
    fn meta(&self) -> &'static RuleMeta {
        &META_DERIVED_NO_ALIAS
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        for tw in &select.from {
            if matches!(tw.relation, TableFactor::Derived { alias: None, .. }) {
                out.push(simple_diag(
                    &META_DERIVED_NO_ALIAS,
                    ctx,
                    "derived table (subquery in FROM) has no alias",
                    Some("add `AS <alias>` after the subquery".into()),
                    ctx.stmt_span,
                ));
            }
        }
    }
}

// ───────────────────────────── SQLT0704 non-deterministic-pagination ────────

pub struct NonDeterministicPagination;

const META_PAG: RuleMeta = RuleMeta {
    id: RuleId("SQLT0704"),
    name: "non-deterministic-pagination",
    category: Category::Style,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "`LIMIT` without `ORDER BY` returns an arbitrary subset of rows.",
    explanation: "SQL guarantees no row order in the absence of ORDER BY. A LIMIT without ORDER BY \
                  may return different rows on different runs (and almost certainly will across \
                  replicas, vacuum cycles, or storage layouts). Add an explicit ORDER BY for any \
                  page-able query.",
};

impl Rule for NonDeterministicPagination {
    fn meta(&self) -> &'static RuleMeta {
        &META_PAG
    }
    fn check_query(&self, query: &Query, _depth: usize, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if query.limit_clause.is_some() && query.order_by.is_none() {
            out.push(simple_diag(
                &META_PAG,
                ctx,
                "LIMIT without ORDER BY — the returned rows are not deterministic",
                Some("add an ORDER BY that matches the pagination key".into()),
                ctx.stmt_span,
            ));
        }
    }
}

// ───────────────────────────── helpers ──────────────────────────────────────

fn simple_diag(
    meta: &'static RuleMeta,
    ctx: &LintCtx,
    msg: &str,
    suggestion: Option<String>,
    span: sqlparser::tokenizer::Span,
) -> Diagnostic {
    Diagnostic {
        rule: meta.id,
        rule_name: meta.name,
        category: meta.category,
        severity: meta.default_severity,
        message: msg.to_string(),
        suggestion,
        span,
        stmt_index: ctx.stmt_index,
        source_dialect: ctx.src,
        target_dialect: ctx.dst,
    }
}
