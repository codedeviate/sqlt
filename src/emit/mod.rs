pub mod default;

use crate::ast::SqltStatement;
use crate::dialect::DialectId;
use crate::error::Result;

/// Emit a list of statements as SQL for the given dialect.
///
/// v1 delegates everything to the upstream sqlparser `Display` impls and
/// re-emits raw fallback fragments verbatim. Per-dialect overrides land in
/// later milestones if round-trip tests reveal `Display` infidelities.
pub fn emit(stmts: &[SqltStatement], _dialect: DialectId) -> Result<String> {
    default::emit(stmts)
}
