use sqlparser::ast::Statement;

use crate::error::Result;

/// Default emitter: delegates to the upstream `Display` impl for each
/// statement and joins with `;\n`. Used for any dialect that doesn't supply
/// its own emitter.
pub fn emit(stmts: &[Statement]) -> Result<String> {
    let mut out = String::new();
    for (i, stmt) in stmts.iter().enumerate() {
        if i > 0 {
            out.push_str(";\n");
        }
        use std::fmt::Write;
        write!(&mut out, "{stmt}").expect("write to String never fails");
    }
    Ok(out)
}
