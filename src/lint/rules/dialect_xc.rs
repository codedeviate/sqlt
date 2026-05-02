//! Dialect cross-contamination rules.
//!
//! Each rule fires when the SQL uses an idiom from a dialect *other* than
//! the one declared by `--from`. The `--to` flag is irrelevant here; that's
//! handled by the SQLT02xx pre-flight rules.

use sqlparser::ast::{CastKind, Expr, Ident, Statement};

use crate::ast::SqltStatement;
use crate::dialect::DialectId;
use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Category, Rule, RuleId, RuleMeta, Severity};

// ───────────────────────────── SQLT0100 mysql-backtick-in-non-mysql ─────────

pub struct MysqlBacktickInNonMysql;

const META_BACKTICK: RuleMeta = RuleMeta {
    id: RuleId("SQLT0100"),
    name: "mysql-backtick-in-non-mysql",
    category: Category::DialectXc,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "Backtick-quoted identifiers (`` `id` ``) are MySQL/MariaDB-only.",
    explanation: "PostgreSQL, MSSQL, and SQLite use double quotes (or square brackets, in MSSQL). \
                  Backticks here suggest the SQL was originally written for MySQL and ported \
                  without rewriting the quoting style. Pass the SQL through `sqlt translate` to \
                  rewrite quoting, or update the source.",
};

impl Rule for MysqlBacktickInNonMysql {
    fn meta(&self) -> &'static RuleMeta {
        &META_BACKTICK
    }
    fn check_expr(&self, expr: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if matches!(ctx.src, DialectId::MySql | DialectId::MariaDb) {
            return;
        }
        for ident in idents_in_expr(expr) {
            if ident.quote_style == Some('`') {
                out.push(diagnostic(
                    &META_BACKTICK,
                    ctx,
                    "backtick quoting is MySQL/MariaDB-specific",
                    Some("use double quotes (or [brackets] for MSSQL)".into()),
                    ident.span,
                ));
            }
        }
    }
}

// ───────────────────────────── SQLT0101 mssql-bracket-in-non-mssql ──────────

pub struct MssqlBracketInNonMssql;

const META_BRACKET: RuleMeta = RuleMeta {
    id: RuleId("SQLT0101"),
    name: "mssql-bracket-in-non-mssql",
    category: Category::DialectXc,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "Square-bracket-quoted identifiers (`[id]`) are MSSQL-specific.",
    explanation: "Other dialects use double quotes or backticks. Most non-MSSQL parsers reject \
                  brackets outright; this rule fires under the Generic dialect, which accepts \
                  them, and serves as a heads-up that translation to a real target dialect will \
                  fail or silently mis-quote.",
};

impl Rule for MssqlBracketInNonMssql {
    fn meta(&self) -> &'static RuleMeta {
        &META_BRACKET
    }
    fn check_expr(&self, expr: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if ctx.src == DialectId::MsSql {
            return;
        }
        for ident in idents_in_expr(expr) {
            if ident.quote_style == Some('[') {
                out.push(diagnostic(
                    &META_BRACKET,
                    ctx,
                    "[bracket] quoting is MSSQL-specific",
                    Some("use double quotes (or backticks for MySQL/MariaDB)".into()),
                    ident.span,
                ));
            }
        }
    }
}

// ───────────────────────────── SQLT0102 postgres-double-colon-cast ──────────

pub struct PostgresDoubleColonCastInMysql;

const META_DOUBLE_COLON: RuleMeta = RuleMeta {
    id: RuleId("SQLT0102"),
    name: "postgres-double-colon-cast-in-mysql",
    category: Category::DialectXc,
    default_severity: Severity::Info,
    default_enabled: true,
    summary: "`x::int` double-colon cast is PostgreSQL-specific syntax.",
    explanation: "MySQL and MariaDB use `CAST(x AS INT)`. SQLite is loose and accepts both, MSSQL \
                  uses `CAST` or `CONVERT`. This rule fires when the AST shows a DoubleColon cast \
                  — usually only reachable from Generic or Postgres source dialects.",
};

impl Rule for PostgresDoubleColonCastInMysql {
    fn meta(&self) -> &'static RuleMeta {
        &META_DOUBLE_COLON
    }
    fn check_expr(&self, expr: &Expr, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if !matches!(
            ctx.src,
            DialectId::MySql | DialectId::MariaDb | DialectId::MsSql
        ) {
            return;
        }
        if let Expr::Cast {
            kind: CastKind::DoubleColon,
            ..
        } = expr
        {
            out.push(diagnostic(
                &META_DOUBLE_COLON,
                ctx,
                "double-colon cast is PostgreSQL syntax",
                Some("rewrite as CAST(x AS type)".into()),
                expr_span(expr).unwrap_or(ctx.stmt_span),
            ));
        }
    }
}

// ───────────────────────────── SQLT0103 mysql-on-duplicate-key ──────────────

pub struct MysqlOnDuplicateKeyInNonMysql;

const META_ON_DUP: RuleMeta = RuleMeta {
    id: RuleId("SQLT0103"),
    name: "mysql-on-duplicate-key-in-non-mysql",
    category: Category::DialectXc,
    default_severity: Severity::Error,
    default_enabled: true,
    summary: "`ON DUPLICATE KEY UPDATE` is MySQL/MariaDB-only.",
    explanation: "PostgreSQL and SQLite use `ON CONFLICT (...) DO UPDATE SET ...`. MSSQL uses \
                  `MERGE`. The MySQL clause will not parse against any other real dialect. \
                  `sqlt translate --from mysql --to postgres` rewrites the simple cases.",
};

impl Rule for MysqlOnDuplicateKeyInNonMysql {
    fn meta(&self) -> &'static RuleMeta {
        &META_ON_DUP
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let SqltStatement::Std(boxed) = stmt else {
            return;
        };
        let Statement::Insert(i) = &**boxed else {
            return;
        };
        let Some(sqlparser::ast::OnInsert::DuplicateKeyUpdate(_)) = &i.on else {
            return;
        };
        if matches!(ctx.src, DialectId::MySql | DialectId::MariaDb) {
            return;
        }
        out.push(diagnostic(
            &META_ON_DUP,
            ctx,
            "ON DUPLICATE KEY UPDATE is MySQL/MariaDB-only",
            Some("rewrite as `ON CONFLICT (...) DO UPDATE SET ...` for postgres/sqlite".into()),
            ctx.stmt_span,
        ));
    }
}

// ───────────────────────────── SQLT0104 returning-in-mysql ──────────────────

pub struct ReturningInMysql;

const META_RETURNING_MYSQL: RuleMeta = RuleMeta {
    id: RuleId("SQLT0104"),
    name: "returning-in-mysql",
    category: Category::DialectXc,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "MySQL does not support RETURNING on INSERT/UPDATE/DELETE (MariaDB does).",
    explanation: "MariaDB added RETURNING in 10.5+ for INSERT/REPLACE/UPDATE/DELETE; MySQL has \
                  not. If you're targeting MySQL, the clause will be a parse error against the \
                  real server (sqlparser parses it leniently). MSSQL uses an `OUTPUT` clause; \
                  Postgres/SQLite/MariaDB use RETURNING.",
};

impl Rule for ReturningInMysql {
    fn meta(&self) -> &'static RuleMeta {
        &META_RETURNING_MYSQL
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if ctx.src != DialectId::MySql {
            return;
        }
        let SqltStatement::Std(boxed) = stmt else {
            return;
        };
        let returning = match &**boxed {
            Statement::Insert(i) => i.returning.is_some(),
            Statement::Update { returning, .. } => returning.is_some(),
            Statement::Delete(d) => d.returning.is_some(),
            _ => false,
        };
        if returning {
            out.push(diagnostic(
                &META_RETURNING_MYSQL,
                ctx,
                "RETURNING is not supported by MySQL (MariaDB 10.5+ does support it)",
                Some("if the target server is MariaDB, switch --from to mariadb; otherwise drop RETURNING".into()),
                ctx.stmt_span,
            ));
        }
    }
}

// ───────────────────────────── helpers ──────────────────────────────────────

/// Walk an `Expr` and yield every `Ident` it transitively contains. Used by
/// the quote-style rules. Not exhaustive — only the variants we care about
/// for v1.
fn idents_in_expr(expr: &Expr) -> Vec<Ident> {
    let mut out = Vec::new();
    walk_idents(expr, &mut out);
    out
}

fn walk_idents(e: &Expr, out: &mut Vec<Ident>) {
    match e {
        Expr::Identifier(i) => out.push(i.clone()),
        Expr::CompoundIdentifier(parts) => out.extend(parts.iter().cloned()),
        Expr::BinaryOp { left, right, .. } => {
            walk_idents(left, out);
            walk_idents(right, out);
        }
        Expr::UnaryOp { expr, .. } | Expr::Nested(expr) => walk_idents(expr, out),
        Expr::Like { expr, pattern, .. } | Expr::ILike { expr, pattern, .. } => {
            walk_idents(expr, out);
            walk_idents(pattern, out);
        }
        Expr::Function(f) => {
            for part in &f.name.0 {
                if let sqlparser::ast::ObjectNamePart::Identifier(i) = part {
                    out.push(i.clone());
                }
            }
        }
        _ => {}
    }
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
