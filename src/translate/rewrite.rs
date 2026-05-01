//! AST rewriter: takes a list of `SqltStatement` parsed in the source
//! dialect and mutates it into a form that's valid for the target dialect,
//! recording a `Warning` for every lossy decision.
//!
//! The rule of thumb: emitters stay dumb (they just write the AST out for
//! their dialect). Anything semantic — dropping `RETURNING`, swapping
//! `ON DUPLICATE KEY UPDATE` for `ON CONFLICT`, warning that a raw
//! fragment can't be carried across — happens here, before emission.

use sqlparser::ast::Statement;

use crate::ast::SqltStatement;
use crate::dialect::DialectId;
use crate::dialect::caps::{DialectCaps, caps_for};
use crate::translate::warn::{WarnCode, WarnSink, Warning};

pub fn rewrite(
    stmts: &mut [SqltStatement],
    src: DialectId,
    dst: DialectId,
    sink: &mut dyn WarnSink,
) {
    let src_caps = caps_for(src);
    let dst_caps = caps_for(dst);
    for stmt in stmts.iter_mut() {
        match stmt {
            SqltStatement::Std(s) => rewrite_statement(s, &src_caps, &dst_caps, sink),
            SqltStatement::Raw(r) => {
                if !dst_caps.mariadb_raw_native {
                    sink.warn(Warning::new(
                        WarnCode::RawPassthrough,
                        format!(
                            "raw {} fragment cannot be represented in {dst}; emitting verbatim — \
                             the target server will likely reject it",
                            r.reason
                        ),
                    ));
                }
            }
        }
    }
}

fn rewrite_statement(
    stmt: &mut Statement,
    _src: &DialectCaps,
    dst: &DialectCaps,
    sink: &mut dyn WarnSink,
) {
    match stmt {
        Statement::Insert(insert) if insert.returning.is_some() && !dst.returning_in_insert => {
            insert.returning = None;
            sink.warn(Warning::new(
                WarnCode::ReturningDropped,
                "INSERT ... RETURNING dropped — target dialect does not support it",
            ));
        }
        Statement::Update { returning, .. } if returning.is_some() && !dst.returning_in_update => {
            *returning = None;
            sink.warn(Warning::new(
                WarnCode::ReturningDropped,
                "UPDATE ... RETURNING dropped — target dialect does not support it",
            ));
        }
        Statement::Delete(delete) if delete.returning.is_some() && !dst.returning_in_delete => {
            delete.returning = None;
            sink.warn(Warning::new(
                WarnCode::ReturningDropped,
                "DELETE ... RETURNING dropped — target dialect does not support it",
            ));
        }
        Statement::CreateSequence { .. } if !dst.create_sequence => {
            sink.warn(Warning::new(
                WarnCode::SequenceDropped,
                "CREATE SEQUENCE has no equivalent in the target dialect; emitting verbatim",
            ));
        }
        _ => {}
    }
}
