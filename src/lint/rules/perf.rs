use sqlparser::ast::{
    BinaryOperator, Expr, FunctionArg, FunctionArgExpr, FunctionArguments, Select, SelectItem,
    Value,
};
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

// ───────────────────────────── SQLT0501 select-star-qualified ───────────────

pub struct SelectStarQualified;

const META_SELECT_STAR_Q: RuleMeta = RuleMeta {
    id: RuleId("SQLT0501"),
    name: "select-star-qualified",
    category: Category::Perf,
    default_severity: Severity::Info,
    default_enabled: false,
    summary: "`SELECT t.*` after a JOIN can hide which columns the query actually needs.",
    explanation: "Disabled by default — qualified wildcards after a JOIN are often legitimate. \
                  Enable with `--rule SQLT0501` to flag every qualified wildcard for review.",
};

impl Rule for SelectStarQualified {
    fn meta(&self) -> &'static RuleMeta {
        &META_SELECT_STAR_Q
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        for item in &select.projection {
            if matches!(item, SelectItem::QualifiedWildcard(_, _)) {
                out.push(simple_diag(
                    &META_SELECT_STAR_Q,
                    ctx,
                    "qualified wildcard `t.*` — consider enumerating columns",
                    Some("list the columns the query needs".into()),
                    ctx.stmt_span,
                ));
            }
        }
    }
}

// ───────────────────────────── SQLT0502 leading-wildcard-like ───────────────

pub struct LeadingWildcardLike;

const META_LEADING_LIKE: RuleMeta = RuleMeta {
    id: RuleId("SQLT0502"),
    name: "leading-wildcard-like",
    category: Category::Perf,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "`LIKE '%foo'` defeats normal B-tree indexes — every row must be scanned.",
    explanation: "A leading `%` means the optimiser cannot use an index range scan. For real \
                  text-search use a full-text index (FTS, MATCH AGAINST, ts_vector, FULLTEXT INDEX) \
                  or a trigram index. If you need substring search on a small table, document the \
                  intent so the next maintainer doesn't think it's an oversight.",
};

impl Rule for LeadingWildcardLike {
    fn meta(&self) -> &'static RuleMeta {
        &META_LEADING_LIKE
    }
    fn check_expr(&self, expr: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let (Expr::Like { pattern, .. } | Expr::ILike { pattern, .. }) = expr else {
            return;
        };
        let Expr::Value(v) = pattern.as_ref() else {
            return;
        };
        let s = match &v.value {
            Value::SingleQuotedString(s)
            | Value::DoubleQuotedString(s)
            | Value::TripleSingleQuotedString(s)
            | Value::TripleDoubleQuotedString(s)
            | Value::EscapedStringLiteral(s)
            | Value::NationalStringLiteral(s) => s.as_str(),
            _ => return,
        };
        if s.starts_with('%') {
            out.push(simple_diag(
                &META_LEADING_LIKE,
                ctx,
                "LIKE pattern starts with `%` — defeats B-tree index lookups",
                Some("use a full-text or trigram index, or anchor the pattern".into()),
                expr_span(expr).unwrap_or(ctx.stmt_span),
            ));
        }
    }
}

// ───────────────────────────── SQLT0503 function-on-column-in-where ─────────

pub struct FunctionOnColumnInWhere;

const META_FN_ON_COL: RuleMeta = RuleMeta {
    id: RuleId("SQLT0503"),
    name: "function-on-column-in-where",
    category: Category::Perf,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "Wrapping a column in a function inside WHERE typically defeats index plans.",
    explanation: "`WHERE LOWER(name) = 'alice'` cannot use a btree index on `name`; the planner \
                  has to evaluate the function for every row. Either store the lowercased value \
                  in a generated column (or trigger), build a functional index, or rewrite the \
                  predicate to leave the column bare. Schema-blind heuristic — false positives \
                  occur when the column isn't indexed in the first place.",
};

impl Rule for FunctionOnColumnInWhere {
    fn meta(&self) -> &'static RuleMeta {
        &META_FN_ON_COL
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if let Some(where_expr) = &select.selection {
            walk_where(where_expr, ctx, out);
        }
    }
}

fn walk_where(e: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
    match e {
        Expr::BinaryOp { op, left, right } if is_comparison(op) => {
            if function_with_column_arg(left) || function_with_column_arg(right) {
                out.push(simple_diag(
                    &META_FN_ON_COL,
                    ctx,
                    "comparison wraps a column in a function — defeats most index plans",
                    Some(
                        "rewrite to keep the column bare, or add a functional/generated column"
                            .into(),
                    ),
                    expr_span(e).unwrap_or(ctx.stmt_span),
                ));
            }
            walk_where(left, ctx, out);
            walk_where(right, ctx, out);
        }
        Expr::BinaryOp { left, right, .. } => {
            walk_where(left, ctx, out);
            walk_where(right, ctx, out);
        }
        Expr::UnaryOp { expr, .. } => walk_where(expr, ctx, out),
        Expr::Nested(e) => walk_where(e, ctx, out),
        _ => {}
    }
}

fn is_comparison(op: &BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::Eq
            | BinaryOperator::NotEq
            | BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
    )
}

fn function_with_column_arg(e: &Expr) -> bool {
    let Expr::Function(f) = e else {
        return false;
    };
    let FunctionArguments::List(list) = &f.args else {
        return false;
    };
    list.args.iter().any(|a| match a {
        FunctionArg::Unnamed(FunctionArgExpr::Expr(e))
        | FunctionArg::ExprNamed {
            arg: FunctionArgExpr::Expr(e),
            ..
        }
        | FunctionArg::Named {
            arg: FunctionArgExpr::Expr(e),
            ..
        } => {
            matches!(e, Expr::Identifier(_) | Expr::CompoundIdentifier(_))
        }
        _ => false,
    })
}

// ───────────────────────────── SQLT0504 distinct-as-join-fix ────────────────

pub struct DistinctAsJoinFix;

const META_DISTINCT_JOIN: RuleMeta = RuleMeta {
    id: RuleId("SQLT0504"),
    name: "distinct-as-join-fix",
    category: Category::Perf,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "`SELECT DISTINCT` after a JOIN often masks a missing aggregation or EXISTS.",
    explanation: "Tagging DISTINCT onto the projection is a common workaround for joins that \
                  duplicate rows. Usually a GROUP BY (or EXISTS for a semi-join) is cheaper and \
                  more accurate. Worth a closer look if the query is in a hot path.",
};

impl Rule for DistinctAsJoinFix {
    fn meta(&self) -> &'static RuleMeta {
        &META_DISTINCT_JOIN
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if select.distinct.is_none() {
            return;
        }
        if select.from.iter().any(|t| !t.joins.is_empty()) {
            out.push(simple_diag(
                &META_DISTINCT_JOIN,
                ctx,
                "DISTINCT used together with a JOIN — often a workaround for a missing aggregation",
                Some("review whether GROUP BY or EXISTS expresses the intent more directly".into()),
                ctx.stmt_span,
            ));
        }
    }
}

// ───────────────────────────── SQLT0505 count-of-nullable-column ────────────

pub struct CountOfNullableColumn;

const META_COUNT_COL: RuleMeta = RuleMeta {
    id: RuleId("SQLT0505"),
    name: "count-of-nullable-column",
    category: Category::Perf,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "`COUNT(col)` skips NULL values; `COUNT(*)` counts every row.",
    explanation: "If the intent is `how many rows`, use `COUNT(*)` (or `COUNT(1)`). If the intent \
                  is `how many rows have a non-NULL value in this column`, keep `COUNT(col)` and \
                  consider adding a comment so the next reader knows it's deliberate. Schema-blind: \
                  the linter doesn't know if the column is declared NOT NULL.",
};

impl Rule for CountOfNullableColumn {
    fn meta(&self) -> &'static RuleMeta {
        &META_COUNT_COL
    }
    fn check_expr(&self, expr: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let Expr::Function(f) = expr else {
            return;
        };
        if !function_name_is(f, "count") {
            return;
        }
        let FunctionArguments::List(list) = &f.args else {
            return;
        };
        if list.args.len() != 1 {
            return;
        }
        if let FunctionArg::Unnamed(FunctionArgExpr::Expr(inner)) = &list.args[0]
            && matches!(inner, Expr::Identifier(_) | Expr::CompoundIdentifier(_))
        {
            out.push(simple_diag(
                &META_COUNT_COL,
                ctx,
                "COUNT(col) skips NULL values; if you want all rows use COUNT(*)",
                Some("use COUNT(*) for `how many rows`, COUNT(col) only when filtering NULLs is intentional".into()),
                expr_span(expr).unwrap_or(ctx.stmt_span),
            ));
        }
    }
}

fn function_name_is(f: &sqlparser::ast::Function, name: &str) -> bool {
    let parts = &f.name.0;
    let Some(last) = parts.last() else {
        return false;
    };
    let ident = match last {
        sqlparser::ast::ObjectNamePart::Identifier(i) => i,
        _ => return false,
    };
    ident.value.eq_ignore_ascii_case(name)
}

// ───────────────────────────── SQLT0506 implicit-string-numeric-compare ─────

pub struct ImplicitStringNumericCompare;

const META_IMPLICIT_CAST: RuleMeta = RuleMeta {
    id: RuleId("SQLT0506"),
    name: "implicit-string-numeric-compare",
    category: Category::Perf,
    default_severity: Severity::Warning,
    default_enabled: false,
    summary: "Comparing a column to a string literal that looks numeric (or vice versa) may force an implicit cast.",
    explanation: "Disabled by default. Implicit casts on column sides of comparisons can defeat \
                  index usage and on MySQL silently coerce string columns to numbers (with surprising \
                  results). The linter cannot know the column type without schema info; expect \
                  false positives. Enable with `--rule SQLT0506` for a focused review pass.",
};

impl Rule for ImplicitStringNumericCompare {
    fn meta(&self) -> &'static RuleMeta {
        &META_IMPLICIT_CAST
    }
    fn check_expr(&self, expr: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let Expr::BinaryOp { op, left, right } = expr else {
            return;
        };
        if !is_comparison(op) {
            return;
        }
        let pair = (looks_like_column(left), looks_like_string(right));
        let symm = (looks_like_column(right), looks_like_string(left));
        let mismatch = pair == (true, true) || symm == (true, true);
        if mismatch {
            out.push(simple_diag(
                &META_IMPLICIT_CAST,
                ctx,
                "comparison between a column and a string literal — possible implicit cast",
                Some("if the column is numeric, drop the quotes; if it's text, use `=` with a string literal of matching shape".into()),
                expr_span(expr).unwrap_or(ctx.stmt_span),
            ));
        }
    }
}

fn looks_like_column(e: &Expr) -> bool {
    matches!(e, Expr::Identifier(_) | Expr::CompoundIdentifier(_))
}

fn looks_like_string(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Value(v) if matches!(
            v.value,
            Value::SingleQuotedString(_) | Value::DoubleQuotedString(_)
        )
    )
}

// ───────────────────────────── SQLT0507 or-across-columns ───────────────────

pub struct OrAcrossColumns;

const META_OR_COLS: RuleMeta = RuleMeta {
    id: RuleId("SQLT0507"),
    name: "or-across-columns",
    category: Category::Perf,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "`WHERE a = ? OR b = ?` rarely uses indexes — `UNION ALL` or `IN` is often faster.",
    explanation: "Most planners can use one index per pass. An OR across two columns prevents the \
                  planner from picking either index, leading to a full scan. Rewriting as `UNION ALL` \
                  of two single-column predicates lets each branch use its own index.",
};

impl Rule for OrAcrossColumns {
    fn meta(&self) -> &'static RuleMeta {
        &META_OR_COLS
    }
    fn check_expr(&self, expr: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let Expr::BinaryOp {
            op: BinaryOperator::Or,
            left,
            right,
        } = expr
        else {
            return;
        };
        if let Some(l) = compared_column(left)
            && let Some(r) = compared_column(right)
            && l != r
        {
            out.push(simple_diag(
                &META_OR_COLS,
                ctx,
                "OR over different columns rarely uses indexes",
                Some("rewrite as UNION ALL with one branch per column".into()),
                expr_span(expr).unwrap_or(ctx.stmt_span),
            ));
        }
    }
}

fn compared_column(e: &Expr) -> Option<String> {
    let Expr::BinaryOp { op, left, right } = e else {
        return None;
    };
    if !is_comparison(op) {
        return None;
    }
    column_name(left).or_else(|| column_name(right))
}

fn column_name(e: &Expr) -> Option<String> {
    match e {
        Expr::Identifier(i) => Some(i.value.clone()),
        Expr::CompoundIdentifier(parts) => parts.last().map(|p| p.value.clone()),
        _ => None,
    }
}

// ───────────────────────────── helpers ──────────────────────────────────────

fn expr_span(e: &Expr) -> Option<Span> {
    use sqlparser::ast::Spanned;
    let s = e.span();
    if s.start.line == 0 && s.start.column == 0 {
        None
    } else {
        Some(s)
    }
}

fn simple_diag(
    meta: &'static RuleMeta,
    ctx: &LintCtx,
    msg: &str,
    suggestion: Option<String>,
    span: Span,
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
