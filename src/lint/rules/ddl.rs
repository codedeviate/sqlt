//! DDL hygiene rules.

use sqlparser::ast::{ColumnDef, DataType, Statement};

use crate::ast::SqltStatement;
use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Category, Rule, RuleId, RuleMeta, Severity};

// ───────────────────────────── SQLT0801 float-for-money ─────────────────────

pub struct FloatForMoney;

const META_FLOAT_MONEY: RuleMeta = RuleMeta {
    id: RuleId("SQLT0801"),
    name: "float-for-money",
    category: Category::Ddl,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "FLOAT/REAL/DOUBLE for a money-shaped column loses cents to rounding.",
    explanation: "Floating-point types cannot represent 0.10 exactly. For money use a fixed-point \
                  type — DECIMAL/NUMERIC with explicit precision and scale. Heuristic: the rule \
                  fires when the column name matches /(price|amount|total|balance|cost|fee)/i. \
                  False positives are possible.",
};

const MONEY_KEYWORDS: &[&str] = &["price", "amount", "total", "balance", "cost", "fee"];

impl Rule for FloatForMoney {
    fn meta(&self) -> &'static RuleMeta {
        &META_FLOAT_MONEY
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let SqltStatement::Std(boxed) = stmt else {
            return;
        };
        let Statement::CreateTable(t) = &**boxed else {
            return;
        };
        for col in &t.columns {
            if !is_money_name(&col.name.value) {
                continue;
            }
            if is_float_type(&col.data_type) {
                out.push(diag(
                    &META_FLOAT_MONEY,
                    ctx,
                    &format!(
                        "column `{}` is FLOAT/REAL/DOUBLE but its name suggests a money value",
                        col.name.value
                    ),
                    Some("use DECIMAL(p, s) for money-shaped columns".into()),
                    col.name.span,
                ));
            }
        }
    }
}

fn is_money_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    MONEY_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

fn is_float_type(t: &DataType) -> bool {
    matches!(
        t,
        DataType::Float(_)
            | DataType::FloatUnsigned(_)
            | DataType::Real
            | DataType::Double(_)
            | DataType::Float4
            | DataType::Float8
            | DataType::Float32
            | DataType::Float64
    )
}

// ───────────────────────────── SQLT0802 varchar-without-length ──────────────

pub struct VarcharWithoutLength;

const META_VARCHAR_NO_LEN: RuleMeta = RuleMeta {
    id: RuleId("SQLT0802"),
    name: "varchar-without-length",
    category: Category::Ddl,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "`VARCHAR` without an explicit length means different things across dialects.",
    explanation: "PostgreSQL allows `VARCHAR` with no length and treats it as unlimited. \
                  MySQL requires a length. MSSQL silently truncates to 1 character — a \
                  classic data-loss bug. Always declare an explicit length: `VARCHAR(255)`.",
};

impl Rule for VarcharWithoutLength {
    fn meta(&self) -> &'static RuleMeta {
        &META_VARCHAR_NO_LEN
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let SqltStatement::Std(boxed) = stmt else {
            return;
        };
        let Statement::CreateTable(t) = &**boxed else {
            return;
        };
        for col in &t.columns {
            if matches!(col.data_type, DataType::Varchar(None)) {
                out.push(diag(
                    &META_VARCHAR_NO_LEN,
                    ctx,
                    &format!(
                        "column `{}` is declared `VARCHAR` with no length",
                        col.name.value
                    ),
                    Some("specify a length, e.g. VARCHAR(255)".into()),
                    col.name.span,
                ));
            }
        }
    }
}

// ───────────────────────────── helpers ──────────────────────────────────────

fn diag(
    meta: &'static RuleMeta,
    ctx: &LintCtx,
    msg: &str,
    suggestion: Option<String>,
    span: sqlparser::tokenizer::Span,
) -> Diagnostic {
    let s = if span.start.line == 0 && span.start.column == 0 {
        ctx.stmt_span
    } else {
        span
    };
    Diagnostic {
        rule: meta.id,
        rule_name: meta.name,
        category: meta.category,
        severity: meta.default_severity,
        message: msg.to_string(),
        suggestion,
        span: s,
        stmt_index: ctx.stmt_index,
        source_dialect: ctx.src,
        target_dialect: ctx.dst,
    }
}

#[allow(dead_code)]
fn _force_used(_: &ColumnDef) {}
