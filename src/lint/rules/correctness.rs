//! Correctness pitfalls — definitely-wrong patterns.

use sqlparser::ast::{
    BinaryOperator, Expr, GroupByExpr, OrderBy, OrderByExpr, OrderByKind, Select, Statement, Value,
};

use crate::ast::SqltStatement;
use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Category, Rule, RuleId, RuleMeta, Severity};

// ───────────────────────────── SQLT0600 equals-null ─────────────────────────

pub struct EqualsNull;

const META_EQ_NULL: RuleMeta = RuleMeta {
    id: RuleId("SQLT0600"),
    name: "equals-null",
    category: Category::Correctness,
    default_severity: Severity::Error,
    default_enabled: true,
    summary: "Comparing a column to NULL with `=` or `!=` always evaluates to UNKNOWN.",
    explanation: "SQL three-valued logic: `col = NULL` is neither true nor false — it's UNKNOWN, \
                  so the row is filtered out. Use `IS NULL` or `IS NOT NULL` instead.",
};

impl Rule for EqualsNull {
    fn meta(&self) -> &'static RuleMeta {
        &META_EQ_NULL
    }
    fn check_expr(&self, expr: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if let Expr::BinaryOp { op, left, right } = expr
            && (matches!(op, BinaryOperator::Eq | BinaryOperator::NotEq))
            && (is_null_value(left) || is_null_value(right))
        {
            out.push(diagnostic(
                &META_EQ_NULL,
                ctx,
                "comparison with NULL using `=` or `!=` always evaluates to UNKNOWN",
                Some("use `IS NULL` or `IS NOT NULL` instead".into()),
                expr_span(expr).unwrap_or(ctx.stmt_span),
            ));
        }
    }
}

// ───────────────────────────── SQLT0601 update-without-where ────────────────

pub struct UpdateWithoutWhere;

const META_UPDATE_NO_WHERE: RuleMeta = RuleMeta {
    id: RuleId("SQLT0601"),
    name: "update-without-where",
    category: Category::Correctness,
    default_severity: Severity::Error,
    default_enabled: true,
    summary: "`UPDATE` without a `WHERE` clause modifies every row in the table.",
    explanation: "Almost always a mistake. If you really mean to touch every row, add `WHERE TRUE` \
                  or otherwise show your intent.",
};

impl Rule for UpdateWithoutWhere {
    fn meta(&self) -> &'static RuleMeta {
        &META_UPDATE_NO_WHERE
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let SqltStatement::Std(boxed) = stmt else {
            return;
        };
        if let Statement::Update {
            selection: None, ..
        } = &**boxed
        {
            out.push(diagnostic(
                &META_UPDATE_NO_WHERE,
                ctx,
                "UPDATE without a WHERE clause modifies every row",
                Some("add a WHERE clause, or `WHERE TRUE` to make the intent explicit".into()),
                ctx.stmt_span,
            ));
        }
    }
}

// ───────────────────────────── SQLT0602 delete-without-where ────────────────

pub struct DeleteWithoutWhere;

const META_DELETE_NO_WHERE: RuleMeta = RuleMeta {
    id: RuleId("SQLT0602"),
    name: "delete-without-where",
    category: Category::Correctness,
    default_severity: Severity::Error,
    default_enabled: true,
    summary: "`DELETE` without a `WHERE` clause removes every row in the table.",
    explanation: "Almost always a mistake. Use `TRUNCATE` for intentional whole-table wipes, \
                  or add `WHERE TRUE` to show intent.",
};

impl Rule for DeleteWithoutWhere {
    fn meta(&self) -> &'static RuleMeta {
        &META_DELETE_NO_WHERE
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let SqltStatement::Std(boxed) = stmt else {
            return;
        };
        if let Statement::Delete(d) = &**boxed
            && d.selection.is_none()
        {
            out.push(diagnostic(
                &META_DELETE_NO_WHERE,
                ctx,
                "DELETE without a WHERE clause removes every row",
                Some(
                    "add a WHERE clause, or use TRUNCATE if you really want to wipe the table"
                        .into(),
                ),
                ctx.stmt_span,
            ));
        }
    }
}

// ───────────────────────────── SQLT0603 mixed-and-or-no-parens ──────────────

pub struct MixedAndOrNoParens;

const META_MIXED_AND_OR: RuleMeta = RuleMeta {
    id: RuleId("SQLT0603"),
    name: "mixed-and-or-no-parens",
    category: Category::Correctness,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "An `OR` whose operand is an unparenthesized `AND` is a precedence trap.",
    explanation: "`AND` binds tighter than `OR`, so `a OR b AND c` parses as `a OR (b AND c)`. \
                  Reviewers and reading tools commonly misread it. Parenthesize the AND-side \
                  explicitly to make precedence visible.",
};

impl Rule for MixedAndOrNoParens {
    fn meta(&self) -> &'static RuleMeta {
        &META_MIXED_AND_OR
    }
    fn check_expr(&self, expr: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if let Expr::BinaryOp {
            op: BinaryOperator::Or,
            left,
            right,
        } = expr
            && (is_unparen_and(left) || is_unparen_and(right))
        {
            out.push(diagnostic(
                &META_MIXED_AND_OR,
                ctx,
                "mixed AND/OR without explicit parentheses",
                Some("parenthesize the AND side to make precedence visible".into()),
                expr_span(expr).unwrap_or(ctx.stmt_span),
            ));
        }
    }
}

// ───────────────────────────── SQLT0604/0606 positional ORDER/GROUP BY ──────

pub struct OrderByPositional;
pub struct GroupByPositional;

const META_ORDER_POS: RuleMeta = RuleMeta {
    id: RuleId("SQLT0604"),
    name: "order-by-positional",
    category: Category::Correctness,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "`ORDER BY <number>` references a projection column by position.",
    explanation: "Positional ORDER BY is brittle: any change to the SELECT list shifts the meaning. \
                  Refer to the column by name or alias instead.",
};

const META_GROUP_POS: RuleMeta = RuleMeta {
    id: RuleId("SQLT0606"),
    name: "group-by-positional",
    category: Category::Correctness,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "`GROUP BY <number>` references a projection column by position.",
    explanation: "Positional GROUP BY is brittle: any change to the SELECT list shifts the meaning. \
                  Refer to the column by name or expression instead.",
};

impl Rule for OrderByPositional {
    fn meta(&self) -> &'static RuleMeta {
        &META_ORDER_POS
    }
    fn check_query(&self, query: &sqlparser::ast::Query, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if let Some(OrderBy {
            kind: OrderByKind::Expressions(exprs),
            ..
        }) = &query.order_by
        {
            for OrderByExpr { expr, .. } in exprs {
                if is_number_literal(expr) {
                    out.push(diagnostic(
                        &META_ORDER_POS,
                        ctx,
                        "ORDER BY references a projection column by position",
                        Some("use the column name or alias instead".into()),
                        expr_span(expr).unwrap_or(ctx.stmt_span),
                    ));
                }
            }
        }
    }
}

impl Rule for GroupByPositional {
    fn meta(&self) -> &'static RuleMeta {
        &META_GROUP_POS
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if let GroupByExpr::Expressions(exprs, _) = &select.group_by {
            for expr in exprs {
                if is_number_literal(expr) {
                    out.push(diagnostic(
                        &META_GROUP_POS,
                        ctx,
                        "GROUP BY references a projection column by position",
                        Some("use the column name or expression instead".into()),
                        expr_span(expr).unwrap_or(ctx.stmt_span),
                    ));
                }
            }
        }
    }
}

// ───────────────────────────── SQLT0605 having-without-group-by ─────────────

pub struct HavingWithoutGroupBy;

const META_HAVING_NO_GROUP: RuleMeta = RuleMeta {
    id: RuleId("SQLT0605"),
    name: "having-without-group-by",
    category: Category::Correctness,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "`HAVING` without `GROUP BY` is almost always a mistake.",
    explanation: "HAVING is meant to filter aggregated groups. Without GROUP BY there is exactly \
                  one implicit group, so HAVING usually means `WHERE`. Some dialects accept this; \
                  most readers will misread it.",
};

impl Rule for HavingWithoutGroupBy {
    fn meta(&self) -> &'static RuleMeta {
        &META_HAVING_NO_GROUP
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if select.having.is_none() {
            return;
        }
        let no_group = matches!(
            &select.group_by,
            GroupByExpr::Expressions(v, _) if v.is_empty()
        );
        if no_group {
            out.push(diagnostic(
                &META_HAVING_NO_GROUP,
                ctx,
                "HAVING used without a GROUP BY clause",
                Some("if you mean to filter rows, use WHERE; otherwise add a GROUP BY".into()),
                ctx.stmt_span,
            ));
        }
    }
}

// ───────────────────────────── helpers ──────────────────────────────────────

fn is_null_value(e: &Expr) -> bool {
    matches!(e, Expr::Value(v) if matches!(v.value, Value::Null))
}

fn is_number_literal(e: &Expr) -> bool {
    matches!(e, Expr::Value(v) if matches!(v.value, Value::Number(_, _)))
}

fn is_unparen_and(e: &Expr) -> bool {
    matches!(
        e,
        Expr::BinaryOp {
            op: BinaryOperator::And,
            ..
        }
    )
}

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
