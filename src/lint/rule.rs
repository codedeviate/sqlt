use sqlparser::ast::{Expr, Query, Select};

use crate::ast::SqltStatement;
use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;

/// Stable rule identifier — `"SQLT0500"`. Once shipped, never change or
/// reuse: external configs key off these strings. Use the `&'static str`
/// inside as both the canonical id and the SARIF rule id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleId(pub &'static str);

impl RuleId {
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }

    /// SARIF level mapping. Note `info` maps to `note` per SARIF spec.
    pub fn sarif_level(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "note",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Raw,
    DialectXc,
    PreFlight,
    Joins,
    Subquery,
    Perf,
    Correctness,
    Style,
    Ddl,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Raw => "raw",
            Category::DialectXc => "dialect-xc",
            Category::PreFlight => "pre-flight",
            Category::Joins => "joins",
            Category::Subquery => "subquery",
            Category::Perf => "perf",
            Category::Correctness => "correctness",
            Category::Style => "style",
            Category::Ddl => "ddl",
        }
    }
}

pub struct RuleMeta {
    pub id: RuleId,
    /// Short slug, e.g. `"select-star"`. Used for `--rule <slug>` and
    /// pretty/SARIF output.
    pub name: &'static str,
    pub category: Category,
    pub default_severity: Severity,
    pub default_enabled: bool,
    /// Single-sentence description for `--explain` and SARIF
    /// `shortDescription`.
    pub summary: &'static str,
    /// Long-form rationale + example for `--explain` and SARIF
    /// `fullDescription`. Schema-blind heuristic rules MUST call out their
    /// false-positive risk here.
    pub explanation: &'static str,
}

/// A lint rule. Rules supply only the callbacks for the AST shapes they
/// inspect; the shared driver in `walk.rs` invokes them as it traverses.
pub trait Rule: Send + Sync {
    fn meta(&self) -> &'static RuleMeta;

    fn check_statement(&self, _stmt: &SqltStatement, _ctx: &LintCtx, _out: &mut Vec<Diagnostic>) {}
    fn check_query(&self, _query: &Query, _ctx: &LintCtx, _out: &mut Vec<Diagnostic>) {}
    fn check_select(&self, _select: &Select, _ctx: &LintCtx, _out: &mut Vec<Diagnostic>) {}
    fn check_expr(&self, _expr: &Expr, _ctx: &LintCtx, _out: &mut Vec<Diagnostic>) {}
}
