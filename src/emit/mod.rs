pub mod default;

use sqlparser::ast::Statement;

use crate::dialect::DialectId;
use crate::error::Result;

/// Emit a list of statements as SQL for the given dialect.
///
/// v1 delegates everything to the upstream sqlparser `Display` impls.
/// Dialect-faithful overrides will be added per-dialect as round-trip tests
/// reveal infidelities (see milestone M5 in the plan).
pub fn emit(stmts: &[Statement], _dialect: DialectId) -> Result<String> {
    default::emit(stmts)
}
