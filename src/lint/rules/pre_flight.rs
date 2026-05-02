//! Translation pre-flight rules. All fire only when `--to` is set, by
//! consulting the per-dialect `caps_for(dst)` table that the translator
//! already uses for its rewriter (`src/translate/rewrite.rs`).
//!
//! These rules don't *do* the translation — they just warn that one would
//! either fail or lose information for the chosen target dialect. The user
//! can then choose to drop the construct, switch the target, or run
//! `sqlt translate` to perform the rewrite.

use sqlparser::ast::Statement;

use crate::ast::SqltStatement;
use crate::dialect::DialectId;
use crate::dialect::caps::{DialectCaps, caps_for};
use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Category, Rule, RuleId, RuleMeta, Severity};

// ───────────────────────────── SQLT0200 returning-unsupported ───────────────

pub struct PreflightReturningUnsupported;

const META_PF_RETURNING: RuleMeta = RuleMeta {
    id: RuleId("SQLT0200"),
    name: "preflight-returning-unsupported",
    category: Category::PreFlight,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "Target dialect does not support RETURNING on this DML statement.",
    explanation: "MySQL and MSSQL lack RETURNING. `sqlt translate` will drop the clause; under \
                  `--strict` the translation fails. If the surrounding code consumes the returned \
                  rows, the application will need to fetch them with a separate SELECT.",
};

impl Rule for PreflightReturningUnsupported {
    fn meta(&self) -> &'static RuleMeta {
        &META_PF_RETURNING
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let Some(dst) = ctx.dst else {
            return;
        };
        let dst_caps = caps_for(dst);
        let SqltStatement::Std(boxed) = stmt else {
            return;
        };
        let unsupported = match &**boxed {
            Statement::Insert(i) => i.returning.is_some() && !dst_caps.returning_in_insert,
            Statement::Update { returning, .. } => {
                returning.is_some() && !dst_caps.returning_in_update
            }
            Statement::Delete(d) => d.returning.is_some() && !dst_caps.returning_in_delete,
            _ => false,
        };
        if unsupported {
            out.push(diag(
                &META_PF_RETURNING,
                ctx,
                &format!("RETURNING is not supported by target dialect {dst}"),
                Some(
                    "`sqlt translate` will drop the clause; fetch rows with a follow-up SELECT"
                        .into(),
                ),
            ));
        }
    }
}

// ───────────────────────────── SQLT0201 on-duplicate-unsupported ────────────

pub struct PreflightOnDuplicateUnsupported;

const META_PF_ONDUP: RuleMeta = RuleMeta {
    id: RuleId("SQLT0201"),
    name: "preflight-on-duplicate-unsupported",
    category: Category::PreFlight,
    default_severity: Severity::Error,
    default_enabled: true,
    summary: "Target dialect does not support `ON DUPLICATE KEY UPDATE`.",
    explanation: "MySQL/MariaDB-only. Postgres and SQLite use `ON CONFLICT (...) DO UPDATE SET ...`; \
                  MSSQL uses `MERGE`. `sqlt translate` rewrites the simple cases.",
};

impl Rule for PreflightOnDuplicateUnsupported {
    fn meta(&self) -> &'static RuleMeta {
        &META_PF_ONDUP
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let Some(dst) = ctx.dst else {
            return;
        };
        let dst_caps = caps_for(dst);
        if dst_caps.on_duplicate_key_update {
            return;
        }
        let SqltStatement::Std(boxed) = stmt else {
            return;
        };
        let Statement::Insert(i) = &**boxed else {
            return;
        };
        if matches!(&i.on, Some(sqlparser::ast::OnInsert::DuplicateKeyUpdate(_))) {
            let suggestion = if dst_caps.on_conflict {
                "rewrite as `ON CONFLICT (col) DO UPDATE SET ...`"
            } else {
                "rewrite as MERGE for MSSQL, or restructure as separate INSERT + UPDATE"
            };
            out.push(diag(
                &META_PF_ONDUP,
                ctx,
                &format!("ON DUPLICATE KEY UPDATE is not supported by {dst}"),
                Some(suggestion.into()),
            ));
        }
    }
}

// ───────────────────────────── SQLT0202 on-conflict-unsupported ─────────────

pub struct PreflightOnConflictUnsupported;

const META_PF_ONCONFLICT: RuleMeta = RuleMeta {
    id: RuleId("SQLT0202"),
    name: "preflight-on-conflict-unsupported",
    category: Category::PreFlight,
    default_severity: Severity::Error,
    default_enabled: true,
    summary: "Target dialect does not support `ON CONFLICT`.",
    explanation: "Postgres/SQLite-specific. MySQL/MariaDB use `ON DUPLICATE KEY UPDATE`; MSSQL \
                  uses `MERGE`. `sqlt translate` rewrites the simple cases.",
};

impl Rule for PreflightOnConflictUnsupported {
    fn meta(&self) -> &'static RuleMeta {
        &META_PF_ONCONFLICT
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let Some(dst) = ctx.dst else {
            return;
        };
        let dst_caps = caps_for(dst);
        if dst_caps.on_conflict {
            return;
        }
        let SqltStatement::Std(boxed) = stmt else {
            return;
        };
        let Statement::Insert(i) = &**boxed else {
            return;
        };
        if matches!(&i.on, Some(sqlparser::ast::OnInsert::OnConflict(_))) {
            let suggestion = if dst_caps.on_duplicate_key_update {
                "rewrite as `ON DUPLICATE KEY UPDATE`"
            } else {
                "rewrite as MERGE for MSSQL"
            };
            out.push(diag(
                &META_PF_ONCONFLICT,
                ctx,
                &format!("ON CONFLICT is not supported by {dst}"),
                Some(suggestion.into()),
            ));
        }
    }
}

// ───────────────────────────── SQLT0203 create-sequence-unsupported ─────────

pub struct PreflightCreateSequenceUnsupported;

const META_PF_SEQ: RuleMeta = RuleMeta {
    id: RuleId("SQLT0203"),
    name: "preflight-create-sequence-unsupported",
    category: Category::PreFlight,
    default_severity: Severity::Warning,
    default_enabled: true,
    summary: "Target dialect does not support `CREATE SEQUENCE`.",
    explanation: "MySQL and SQLite lack named sequences. Use `AUTO_INCREMENT` / `AUTOINCREMENT` \
                  on the column, or maintain a counter table by hand. `sqlt translate` will warn \
                  and emit the original SQL verbatim.",
};

impl Rule for PreflightCreateSequenceUnsupported {
    fn meta(&self) -> &'static RuleMeta {
        &META_PF_SEQ
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let Some(dst) = ctx.dst else {
            return;
        };
        if caps_for(dst).create_sequence {
            return;
        }
        let SqltStatement::Std(boxed) = stmt else {
            return;
        };
        if matches!(&**boxed, Statement::CreateSequence { .. }) {
            out.push(diag(
                &META_PF_SEQ,
                ctx,
                &format!("CREATE SEQUENCE is not supported by {dst}"),
                Some("use AUTO_INCREMENT/AUTOINCREMENT on the column or a counter table".into()),
            ));
        }
    }
}

// ───────────────────────────── SQLT0204 raw-passthrough-unsupported ─────────

pub struct PreflightRawPassthroughUnsupported;

const META_PF_RAW: RuleMeta = RuleMeta {
    id: RuleId("SQLT0204"),
    name: "preflight-raw-passthrough-unsupported",
    category: Category::PreFlight,
    default_severity: Severity::Error,
    default_enabled: true,
    summary: "MariaDB-specific construct cannot be represented in non-MariaDB targets.",
    explanation: "Constructs like WITH SYSTEM VERSIONING, FOR SYSTEM_TIME, CREATE PACKAGE, and \
                  vector types fall back to raw text in v1. `sqlt translate` will emit the original \
                  SQL with a RAW_PASSTHROUGH warning; the target server will reject it.",
};

impl Rule for PreflightRawPassthroughUnsupported {
    fn meta(&self) -> &'static RuleMeta {
        &META_PF_RAW
    }
    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        let Some(dst) = ctx.dst else {
            return;
        };
        if caps_for(dst).mariadb_raw_native {
            return;
        }
        let SqltStatement::Raw(r) = stmt else {
            return;
        };
        out.push(diag(
            &META_PF_RAW,
            ctx,
            &format!(
                "raw {} fragment cannot be represented in {dst}; emitted SQL will be rejected by the target server",
                r.reason
            ),
            Some("rewrite the construct as standard SQL, or keep --to as mariadb".into()),
        ));
    }
}

// ───────────────────────────── SQLT0205 quote-style-mismatch ────────────────

pub struct PreflightQuoteStyleMismatch;

const META_PF_QUOTE: RuleMeta = RuleMeta {
    id: RuleId("SQLT0205"),
    name: "preflight-quote-style-mismatch",
    category: Category::PreFlight,
    default_severity: Severity::Info,
    default_enabled: false,
    summary: "Identifier quote style won't survive emit to the target dialect.",
    explanation: "Disabled by default — the emitter renders identifiers using the upstream \
                  Display impl, which is generally faithful for the target dialect. Opt-in for \
                  cases where you want every quote-style change flagged for review.",
};

impl Rule for PreflightQuoteStyleMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &META_PF_QUOTE
    }
    // Implementation deferred — would walk every Ident and compare its
    // quote_style to the target dialect's preferred style. v1 only registers
    // the metadata so `--explain SQLT0205` works and the rule slot is
    // reserved.
}

// ───────────────────────────── helper ───────────────────────────────────────

fn diag(
    meta: &'static RuleMeta,
    ctx: &LintCtx,
    msg: &str,
    suggestion: Option<String>,
) -> Diagnostic {
    Diagnostic {
        rule: meta.id,
        rule_name: meta.name,
        category: meta.category,
        severity: meta.default_severity,
        message: msg.to_string(),
        suggestion,
        span: ctx.stmt_span,
        stmt_index: ctx.stmt_index,
        source_dialect: ctx.src,
        target_dialect: ctx.dst,
    }
}

#[allow(dead_code)]
fn _force_caps_used(_: DialectCaps, _: DialectId) {}
