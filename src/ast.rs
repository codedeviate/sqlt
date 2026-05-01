//! `sqlt`'s statement type.
//!
//! For dialects whose syntax fully fits the upstream `sqlparser` AST, every
//! statement is `SqltStatement::Std(Statement)` and serializes/deserializes
//! exactly like the bare upstream `Statement` (via `#[serde(untagged)]`).
//!
//! MariaDB ships syntax that has no typed upstream representation in
//! `sqlparser` v0.59 — `WITH SYSTEM VERSIONING`, `FOR SYSTEM_TIME`,
//! Oracle-compat `PACKAGE`, and a handful of others. For those we capture the
//! raw text in `SqltStatement::Raw` so that:
//!   * round-trip parse → emit preserves the original SQL verbatim,
//!   * the JSON envelope is lossless, and
//!   * translation can emit a warning rather than silently corrupting input.
//!
//! See `OUT-OF-SCOPE.md` for the list of constructs that fall back to `Raw`
//! in v1 and the upstream contributions that would let us promote them to
//! typed AST nodes.

use serde::{Deserialize, Serialize};
use sqlparser::ast::Statement;

/// A parsed top-level statement, either upstream-typed or a raw passthrough.
///
/// `Std` carries `Box<Statement>` because the upstream `Statement` is large
/// (~2.6 kB) and we don't want to bloat every `Vec<SqltStatement>` element
/// to that size — the box keeps the enum compact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SqltStatement {
    /// A statement whose AST is fully represented upstream.
    Std(Box<Statement>),
    /// A raw passthrough used when no typed upstream node exists.
    Raw(RawStatement),
}

/// A raw SQL fragment we couldn't parse into a typed AST node.
///
/// Tagged with `sqlt_raw` so the JSON shape is unambiguously distinguishable
/// from a `Statement` variant — `Statement` always serializes as a single
/// upper-camel-case key (e.g. `{"Insert": {...}}`), whereas `RawStatement`
/// carries the marker key `sqlt_raw`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawStatement {
    /// The original SQL text (excluding the trailing `;`).
    pub sqlt_raw: String,
    /// Why this fragment was kept raw — e.g. `"system_versioning"`,
    /// `"create_package"`, `"sequence_option_order"`. Used by `translate` to
    /// build a useful warning message.
    pub reason: String,
}

impl SqltStatement {
    pub fn is_raw(&self) -> bool {
        matches!(self, SqltStatement::Raw(_))
    }
}

impl From<Statement> for SqltStatement {
    fn from(s: Statement) -> Self {
        SqltStatement::Std(Box::new(s))
    }
}
