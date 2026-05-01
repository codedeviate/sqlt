pub mod rewrite;
pub mod warn;

pub use warn::{CollectingSink, StderrSink, WarnCode, WarnSink, Warning};

use crate::dialect::DialectId;
use crate::error::{Error, Result};
use crate::{emit, parse};

#[derive(Debug, Default, Clone, Copy)]
pub struct TranslateOptions {
    pub strict: bool,
}

pub fn translate(
    sql: &str,
    src: DialectId,
    dst: DialectId,
    sink: &mut dyn WarnSink,
    opts: &TranslateOptions,
) -> Result<String> {
    let mut stmts = parse::parse(sql, src)?;
    rewrite::rewrite(&mut stmts, src, dst, sink);
    let out = emit::emit(&stmts, dst)?;
    if opts.strict && sink.count() > 0 {
        return Err(Error::StrictWarnings);
    }
    Ok(out)
}
