//! Subquery improvement suggestions.

use sqlparser::ast::{Expr, Query, Select, SelectItem, SetExpr};

use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Category, Rule, RuleId, RuleMeta, Severity};

// ───────────────────────────── SQLT0400 not-in-subquery-null-pitfall ────────

pub struct NotInSubqueryNullPitfall;

const META_NOT_IN: RuleMeta = RuleMeta {
    id: RuleId("SQLT0400"),
    name: "not-in-subquery-null-pitfall",
    category: Category::Subquery,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "`NOT IN (SELECT ...)` returns no rows if the subquery yields any NULL.",
    explanation: "SQL three-valued logic strikes again. `x NOT IN (1, NULL)` evaluates to UNKNOWN \
                  for every x, not TRUE. Prefer `NOT EXISTS (SELECT 1 ... WHERE col = outer.x)`. \
                  Schema-aware: when a `CREATE TABLE` for the inner subquery's projected column \
                  is present in the same input AND that column is declared `NOT NULL`, the \
                  warning is suppressed because the NULL pitfall cannot trigger.",
};

impl Rule for NotInSubqueryNullPitfall {
    fn meta(&self) -> &'static RuleMeta {
        &META_NOT_IN
    }
    fn check_expr(&self, expr: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let Expr::InSubquery {
            negated: true,
            subquery,
            ..
        } = expr
        else {
            return;
        };
        // Schema-aware suppression: if the subquery selects a single column
        // that resolves to a known NOT NULL column, NULL can't appear.
        if subquery_projects_not_null(subquery, ctx) {
            return;
        }
        out.push(diagnostic(
            &META_NOT_IN,
            ctx,
            "NOT IN (SELECT ...) returns no rows if the subquery yields any NULL",
            Some("rewrite as `NOT EXISTS (SELECT 1 ... WHERE col = outer.col)`".into()),
            expr_span(expr).unwrap_or(ctx.stmt_span),
        ));
    }
}

/// Returns true if the subquery projects exactly one column reference whose
/// schema-declared type is NOT NULL. Conservative: any other shape returns
/// false (and the warning fires).
fn subquery_projects_not_null(q: &Query, ctx: &LintCtx) -> bool {
    if ctx.schema.is_empty() {
        return false;
    }
    let SetExpr::Select(s) = q.body.as_ref() else {
        return false;
    };
    if s.projection.len() != 1 {
        return false;
    }
    let inner = match &s.projection[0] {
        SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => e,
        _ => return false,
    };
    // Resolve the projected column to a (table, column) using the FROM
    // clause's table, if exactly one is present.
    let from_table = if s.from.len() == 1 && s.from[0].joins.is_empty() {
        match &s.from[0].relation {
            sqlparser::ast::TableFactor::Table { name, .. } => {
                name.0.last().and_then(|p| match p {
                    sqlparser::ast::ObjectNamePart::Identifier(i) => Some(i.value.clone()),
                    _ => None,
                })
            }
            _ => None,
        }
    } else {
        None
    };
    let (table, column) = match inner {
        Expr::Identifier(i) => match &from_table {
            Some(t) => (t.clone(), i.value.clone()),
            None => return false,
        },
        Expr::CompoundIdentifier(parts) if parts.len() >= 2 => (
            parts[parts.len() - 2].value.clone(),
            parts[parts.len() - 1].value.clone(),
        ),
        _ => return false,
    };
    ctx.schema
        .column(&table, &column)
        .is_some_and(|c| !c.nullable)
}

// ───────────────────────────── SQLT0401 in-subquery-prefer-exists ───────────

pub struct InSubqueryPreferExists;

const META_IN_EXISTS: RuleMeta = RuleMeta {
    id: RuleId("SQLT0401"),
    name: "in-subquery-prefer-exists",
    category: Category::Subquery,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "`IN (SELECT ...)` is often clearer and faster as `EXISTS`.",
    explanation: "A correlated EXISTS short-circuits as soon as it finds a match, while many \
                  planners materialize an IN subquery into a temporary set first. For \
                  single-column subqueries the two are equivalent in semantics; EXISTS is usually \
                  preferable. (Schema-blind: the suggestion may be unnecessary if your planner \
                  already optimizes IN to a semi-join — measure first if performance matters.)",
};

impl Rule for InSubqueryPreferExists {
    fn meta(&self) -> &'static RuleMeta {
        &META_IN_EXISTS
    }
    fn check_expr(&self, expr: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if let Expr::InSubquery {
            negated: false,
            subquery,
            ..
        } = expr
            && let SetExpr::Select(s) = subquery.body.as_ref()
            && s.projection.len() == 1
        {
            out.push(diagnostic(
                &META_IN_EXISTS,
                ctx,
                "IN (SELECT col FROM ...) is often clearer/faster as EXISTS",
                Some("rewrite as `EXISTS (SELECT 1 ... WHERE col = outer.col)`".into()),
                expr_span(expr).unwrap_or(ctx.stmt_span),
            ));
        }
    }
}

// ───────────────────────────── SQLT0402 scalar-subquery-in-select ───────────

pub struct ScalarSubqueryInSelect;

const META_SCALAR_SUB: RuleMeta = RuleMeta {
    id: RuleId("SQLT0402"),
    name: "scalar-subquery-in-select",
    category: Category::Subquery,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "Scalar subquery in the SELECT list runs once per row — N+1 risk.",
    explanation: "`SELECT (SELECT count(*) FROM orders WHERE user_id = u.id) FROM users u` runs \
                  the inner subquery once per row in the outer FROM. A LEFT JOIN with GROUP BY or \
                  a lateral join is usually faster. Modern planners may rewrite this themselves; \
                  measure before assuming the rewrite is a win.",
};

impl Rule for ScalarSubqueryInSelect {
    fn meta(&self) -> &'static RuleMeta {
        &META_SCALAR_SUB
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        for item in &select.projection {
            let expr = match item {
                SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => e,
                _ => continue,
            };
            if matches!(expr, Expr::Subquery(_)) {
                out.push(diagnostic(
                    &META_SCALAR_SUB,
                    ctx,
                    "scalar subquery in SELECT list runs once per outer row",
                    Some("consider a LEFT JOIN + GROUP BY (or LATERAL) instead".into()),
                    expr_span(expr).unwrap_or(ctx.stmt_span),
                ));
            }
        }
    }
}

// ───────────────────────────── SQLT0403 order-by-in-subquery-without-limit ──

pub struct OrderByInSubqueryWithoutLimit;

const META_ORDER_NO_LIMIT: RuleMeta = RuleMeta {
    id: RuleId("SQLT0403"),
    name: "order-by-in-subquery-without-limit",
    category: Category::Subquery,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "ORDER BY inside a subquery / CTE / derived table without LIMIT is wasted work.",
    explanation: "Most planners ignore ORDER BY in a subquery whose ordering does not affect the \
                  outer result; the few that honor it spend cycles sorting only to throw the \
                  ordering away when joining. If you want a top-N within the subquery, add LIMIT.",
};

impl Rule for OrderByInSubqueryWithoutLimit {
    fn meta(&self) -> &'static RuleMeta {
        &META_ORDER_NO_LIMIT
    }
    fn check_query(&self, query: &Query, depth: usize, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if depth == 0 {
            return; // outermost — top-level ORDER BY is observable to the caller
        }
        if query.order_by.is_some() && query.limit_clause.is_none() {
            out.push(diagnostic(
                &META_ORDER_NO_LIMIT,
                ctx,
                "ORDER BY in a subquery / CTE / derived table without LIMIT is usually wasted",
                Some("add LIMIT, or drop ORDER BY if the inner row order isn't observable".into()),
                ctx.stmt_span,
            ));
        }
    }
}

// ───────────────────────────── SQLT0404 correlated-subquery-in-where ────────

pub struct CorrelatedSubqueryInWhere;

const META_CORRELATED: RuleMeta = RuleMeta {
    id: RuleId("SQLT0404"),
    name: "correlated-subquery-in-where",
    category: Category::Subquery,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "A subquery in WHERE that references an outer column may be a JOIN candidate.",
    explanation: "Correlated subqueries run per outer row. Many of them rewrite cleanly to a JOIN \
                  with GROUP BY or DISTINCT. Schema-blind heuristic: the linter flags any \
                  EXISTS/IN/Subquery in WHERE where the inner SELECT references a compound \
                  identifier (e.g. `outer.col`). False positives are possible; treat as a hint.",
};

impl Rule for CorrelatedSubqueryInWhere {
    fn meta(&self) -> &'static RuleMeta {
        &META_CORRELATED
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let Some(where_expr) = &select.selection else {
            return;
        };
        // Walk the WHERE expression tree looking for any subquery whose body
        // mentions a compound identifier (heuristic for "outer.col").
        let mut found = false;
        scan_for_correlated(where_expr, &mut found);
        if found {
            out.push(diagnostic(
                &META_CORRELATED,
                ctx,
                "correlated subquery in WHERE — may be a JOIN candidate",
                Some(
                    "consider rewriting as a JOIN + GROUP BY / DISTINCT, or EXISTS for semi-join intent"
                        .into(),
                ),
                expr_span(where_expr).unwrap_or(ctx.stmt_span),
            ));
        }
    }
}

fn scan_for_correlated(e: &Expr, found: &mut bool) {
    if *found {
        return;
    }
    match e {
        Expr::Subquery(q) | Expr::Exists { subquery: q, .. } if subquery_has_compound_ident(q) => {
            *found = true;
        }
        Expr::InSubquery { subquery, .. } if subquery_has_compound_ident(subquery) => {
            *found = true;
        }
        Expr::BinaryOp { left, right, .. } => {
            scan_for_correlated(left, found);
            scan_for_correlated(right, found);
        }
        Expr::UnaryOp { expr, .. } => scan_for_correlated(expr, found),
        Expr::Nested(e) => scan_for_correlated(e, found),
        _ => {}
    }
}

fn subquery_has_compound_ident(q: &Query) -> bool {
    let SetExpr::Select(s) = q.body.as_ref() else {
        return false;
    };
    let Some(where_expr) = &s.selection else {
        return false;
    };
    let mut hit = false;
    scan_for_compound_ident(where_expr, &mut hit);
    hit
}

fn scan_for_compound_ident(e: &Expr, hit: &mut bool) {
    if *hit {
        return;
    }
    match e {
        Expr::CompoundIdentifier(_) => *hit = true,
        Expr::BinaryOp { left, right, .. } => {
            scan_for_compound_ident(left, hit);
            scan_for_compound_ident(right, hit);
        }
        Expr::UnaryOp { expr, .. } => scan_for_compound_ident(expr, hit),
        Expr::Nested(e) => scan_for_compound_ident(e, hit),
        _ => {}
    }
}

// ───────────────────────────── helpers ──────────────────────────────────────

fn expr_span(e: &Expr) -> Option<sqlparser::tokenizer::Span> {
    use sqlparser::ast::Spanned;
    let s = e.span();
    if s.start.line == 0 && s.start.column == 0 {
        None
    } else {
        Some(s)
    }
}

fn diagnostic(
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
