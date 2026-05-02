//! Join hygiene rules.

use sqlparser::ast::{BinaryOperator, Expr, Join, JoinConstraint, JoinOperator, Select, Value};

use crate::dialect::DialectId;
use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Category, Rule, RuleId, RuleMeta, Severity};

// ───────────────────────────── SQLT0300 implicit-cross-join ─────────────────

pub struct ImplicitCrossJoin;

const META_IMPLICIT_CROSS: RuleMeta = RuleMeta {
    id: RuleId("SQLT0300"),
    name: "implicit-cross-join",
    category: Category::Joins,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "`FROM a, b` is a comma cross join — usually unintended.",
    explanation: "A comma-separated FROM clause produces a cartesian product unless the WHERE \
                  clause filters it down. Even when correct, modern explicit `JOIN ... ON` syntax \
                  is much more readable. Mixing comma joins with explicit JOINs (see SQLT0307) is \
                  a precedence bug magnet.",
};

impl Rule for ImplicitCrossJoin {
    fn meta(&self) -> &'static RuleMeta {
        &META_IMPLICIT_CROSS
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if select.from.len() > 1 {
            out.push(diagnostic(
                &META_IMPLICIT_CROSS,
                ctx,
                &format!(
                    "FROM has {} comma-separated tables; this is an implicit cross join",
                    select.from.len()
                ),
                Some("rewrite as `a CROSS JOIN b` or add an `ON` clause".into()),
                ctx.stmt_span,
            ));
        }
    }
}

// ───────────────────────────── SQLT0301 cross-join-without-where ────────────

pub struct CrossJoinWithoutWhere;

const META_CROSS_NO_WHERE: RuleMeta = RuleMeta {
    id: RuleId("SQLT0301"),
    name: "cross-join-without-where",
    category: Category::Joins,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "`CROSS JOIN` with no WHERE clause produces a full cartesian product.",
    explanation: "Cartesian products on real tables are almost always wrong and almost always slow. \
                  If you really mean it, add a comment, otherwise add a WHERE clause or convert to \
                  an INNER JOIN with an ON predicate.",
};

impl Rule for CrossJoinWithoutWhere {
    fn meta(&self) -> &'static RuleMeta {
        &META_CROSS_NO_WHERE
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if select.selection.is_some() {
            return;
        }
        for tw in &select.from {
            for j in &tw.joins {
                if matches!(j.join_operator, JoinOperator::CrossJoin(_)) {
                    out.push(diagnostic(
                        &META_CROSS_NO_WHERE,
                        ctx,
                        "CROSS JOIN with no WHERE clause is a full cartesian product",
                        Some("filter the result with a WHERE clause".into()),
                        ctx.stmt_span,
                    ));
                    return;
                }
            }
        }
    }
}

// ───────────────────────────── SQLT0302 natural-join ────────────────────────

pub struct NaturalJoin;

const META_NATURAL: RuleMeta = RuleMeta {
    id: RuleId("SQLT0302"),
    name: "natural-join",
    category: Category::Joins,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "`NATURAL JOIN` is fragile — adding a column to either table silently changes the join.",
    explanation: "Natural joins use whatever columns happen to share names today. A future schema \
                  change adds an unrelated column with a colliding name and the join semantics \
                  change without warning. Always use an explicit `ON` or `USING` clause.",
};

impl Rule for NaturalJoin {
    fn meta(&self) -> &'static RuleMeta {
        &META_NATURAL
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        for tw in &select.from {
            for j in &tw.joins {
                if uses_natural_constraint(&j.join_operator) {
                    out.push(diagnostic(
                        &META_NATURAL,
                        ctx,
                        "NATURAL JOIN auto-binds on every same-named column",
                        Some("use explicit ON or USING".into()),
                        ctx.stmt_span,
                    ));
                }
            }
        }
    }
}

// ───────────────────────────── SQLT0303 join-without-on ─────────────────────

pub struct JoinWithoutOn;

const META_NO_ON: RuleMeta = RuleMeta {
    id: RuleId("SQLT0303"),
    name: "join-without-on",
    category: Category::Joins,
    default_severity: Severity::Error,
    default_enabled: true,
    summary: "An INNER/LEFT/RIGHT/FULL JOIN without an ON or USING clause is almost always a bug.",
    explanation: "Older parsers tolerated `... JOIN tbl` without a constraint and fell back to a \
                  cross join. Modern code should always supply an explicit join predicate.",
};

impl Rule for JoinWithoutOn {
    fn meta(&self) -> &'static RuleMeta {
        &META_NO_ON
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        for tw in &select.from {
            for j in &tw.joins {
                if join_op_constraint_is_none(&j.join_operator) {
                    out.push(diagnostic(
                        &META_NO_ON,
                        ctx,
                        "JOIN without ON or USING — this is effectively a cross join",
                        Some("add an ON predicate or USING clause".into()),
                        ctx.stmt_span,
                    ));
                }
            }
        }
    }
}

// ───────────────────────────── SQLT0304 on-tautology ────────────────────────

pub struct OnTautology;

const META_TAUTOLOGY: RuleMeta = RuleMeta {
    id: RuleId("SQLT0304"),
    name: "on-tautology",
    category: Category::Joins,
    default_severity: Severity::Error,
    default_enabled: true,
    summary: "`ON 1=1` (or `ON TRUE`) turns any join into a cross join.",
    explanation: "If you want a cross join, write `CROSS JOIN`. A tautological ON predicate is \
                  a code smell that usually means the author forgot to fill in the real predicate.",
};

impl Rule for OnTautology {
    fn meta(&self) -> &'static RuleMeta {
        &META_TAUTOLOGY
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        for tw in &select.from {
            for j in &tw.joins {
                if let Some(constraint) = join_constraint(&j.join_operator)
                    && let JoinConstraint::On(expr) = constraint
                    && is_tautology(expr)
                {
                    out.push(diagnostic(
                        &META_TAUTOLOGY,
                        ctx,
                        "JOIN ... ON 1=1 / ON TRUE is a cross join in disguise",
                        Some("write CROSS JOIN explicitly, or supply a real predicate".into()),
                        ctx.stmt_span,
                    ));
                }
            }
        }
    }
}

// ───────────────────────────── SQLT0305 using-with-quoted-ident ─────────────

pub struct UsingWithQuotedIdent;

const META_USING_QUOTED: RuleMeta = RuleMeta {
    id: RuleId("SQLT0305"),
    name: "using-with-quoted-ident",
    category: Category::Joins,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "`USING(\"col\")` with quoted identifiers can disagree across dialects on case folding.",
    explanation: "Some dialects fold unquoted identifiers to lowercase; others to uppercase. \
                  Quoted identifiers preserve case. Mixing quoted USING column names with \
                  unquoted table-side columns can produce subtle case-mismatch bugs. Prefer ON.",
};

impl Rule for UsingWithQuotedIdent {
    fn meta(&self) -> &'static RuleMeta {
        &META_USING_QUOTED
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        for tw in &select.from {
            for j in &tw.joins {
                if let Some(JoinConstraint::Using(idents)) = join_constraint(&j.join_operator) {
                    for name in idents {
                        for part in &name.0 {
                            if let Some(part) = part.as_ident()
                                && part.quote_style.is_some()
                            {
                                out.push(diagnostic(
                                    &META_USING_QUOTED,
                                    ctx,
                                    "USING with quoted identifier risks case-folding mismatches",
                                    Some(
                                        "rewrite as ON a.col = b.col with explicit aliases".into(),
                                    ),
                                    ctx.stmt_span,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

// ───────────────────────────── SQLT0306 full-outer-mysql ────────────────────

pub struct FullOuterMysql;

const META_FULL_OUTER_MYSQL: RuleMeta = RuleMeta {
    id: RuleId("SQLT0306"),
    name: "full-outer-in-mysql",
    category: Category::Joins,
    default_severity: Severity::Error,
    default_enabled: true,
    summary: "MySQL does not support `FULL OUTER JOIN`.",
    explanation: "MariaDB also lacks native FULL OUTER JOIN as of recent releases. Emulate with \
                  `LEFT JOIN ... UNION ALL ... RIGHT JOIN ... WHERE l.id IS NULL`. PostgreSQL, \
                  MSSQL, and SQLite (3.39+) support it natively.",
};

impl Rule for FullOuterMysql {
    fn meta(&self) -> &'static RuleMeta {
        &META_FULL_OUTER_MYSQL
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if !matches!(ctx.src, DialectId::MySql | DialectId::MariaDb) {
            return;
        }
        for tw in &select.from {
            for j in &tw.joins {
                if matches!(j.join_operator, JoinOperator::FullOuter(_)) {
                    out.push(diagnostic(
                        &META_FULL_OUTER_MYSQL,
                        ctx,
                        "FULL OUTER JOIN is not supported in MySQL/MariaDB",
                        Some("emulate with LEFT JOIN UNION RIGHT JOIN".into()),
                        ctx.stmt_span,
                    ));
                }
            }
        }
    }
}

// ───────────────────────────── SQLT0307 comma-join-with-on-elsewhere ────────

pub struct CommaJoinWithOnElsewhere;

const META_MIXED_JOIN: RuleMeta = RuleMeta {
    id: RuleId("SQLT0307"),
    name: "comma-join-with-on-elsewhere",
    category: Category::Joins,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "Mixing comma joins (`FROM a, b`) with explicit `JOIN ... ON` is a precedence trap.",
    explanation: "Comma has lower precedence than the explicit JOIN keyword in most dialects, so \
                  `FROM a, b JOIN c ON c.id = b.id` joins c with b first and then cartesian-products \
                  the result with a. Always use one style consistently — preferably explicit JOINs.",
};

impl Rule for CommaJoinWithOnElsewhere {
    fn meta(&self) -> &'static RuleMeta {
        &META_MIXED_JOIN
    }
    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if select.from.len() > 1 && select.from.iter().any(|t| !t.joins.is_empty()) {
            out.push(diagnostic(
                &META_MIXED_JOIN,
                ctx,
                "FROM mixes comma joins with explicit JOIN ... ON",
                Some(
                    "rewrite all relations using explicit JOIN ... ON for consistent precedence"
                        .into(),
                ),
                ctx.stmt_span,
            ));
        }
    }
}

// ───────────────────────────── helpers ──────────────────────────────────────

fn join_op_constraint_is_none(op: &JoinOperator) -> bool {
    matches!(
        op,
        JoinOperator::Inner(JoinConstraint::None)
            | JoinOperator::Join(JoinConstraint::None)
            | JoinOperator::Left(JoinConstraint::None)
            | JoinOperator::LeftOuter(JoinConstraint::None)
            | JoinOperator::Right(JoinConstraint::None)
            | JoinOperator::RightOuter(JoinConstraint::None)
            | JoinOperator::FullOuter(JoinConstraint::None)
    )
}

fn uses_natural_constraint(op: &JoinOperator) -> bool {
    matches!(join_constraint(op), Some(JoinConstraint::Natural))
}

fn join_constraint(op: &JoinOperator) -> Option<&JoinConstraint> {
    match op {
        JoinOperator::Join(c)
        | JoinOperator::Inner(c)
        | JoinOperator::Left(c)
        | JoinOperator::LeftOuter(c)
        | JoinOperator::Right(c)
        | JoinOperator::RightOuter(c)
        | JoinOperator::FullOuter(c)
        | JoinOperator::CrossJoin(c)
        | JoinOperator::Semi(c)
        | JoinOperator::LeftSemi(c)
        | JoinOperator::RightSemi(c)
        | JoinOperator::Anti(c)
        | JoinOperator::LeftAnti(c)
        | JoinOperator::RightAnti(c)
        | JoinOperator::StraightJoin(c) => Some(c),
        JoinOperator::AsOf { constraint, .. } => Some(constraint),
        JoinOperator::CrossApply | JoinOperator::OuterApply => None,
    }
}

fn is_tautology(e: &Expr) -> bool {
    match e {
        Expr::Value(v) => matches!(v.value, Value::Boolean(true)),
        Expr::BinaryOp {
            op: BinaryOperator::Eq,
            left,
            right,
        } => {
            // `1 = 1` etc.
            number_equal(left, right)
        }
        Expr::Nested(inner) => is_tautology(inner),
        _ => false,
    }
}

fn number_equal(l: &Expr, r: &Expr) -> bool {
    let (Expr::Value(lv), Expr::Value(rv)) = (l, r) else {
        return false;
    };
    let (Value::Number(ln, _), Value::Number(rn, _)) = (&lv.value, &rv.value) else {
        return false;
    };
    ln == rn
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

// `Join` import only for documentation cross-reference
#[allow(dead_code)]
fn _join_type_check(_: &Join) {}
