use crate::ast::SqltStatement;
use crate::error::Result;

/// Default emitter: delegates to the upstream `Display` impl for each `Std`
/// statement and emits raw text verbatim for `Raw` statements. Statements
/// are joined with `;\n`.
pub fn emit(stmts: &[SqltStatement]) -> Result<String> {
    use std::fmt::Write;
    let mut out = String::new();
    for (i, stmt) in stmts.iter().enumerate() {
        if i > 0 {
            out.push_str(";\n");
        }
        match stmt {
            SqltStatement::Std(s) => write!(&mut out, "{s}").expect("infallible"),
            SqltStatement::Raw(r) => out.push_str(r.sqlt_raw.trim_end_matches(';').trim_end()),
        }
    }
    Ok(out)
}
