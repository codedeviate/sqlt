use serde::{Deserialize, Serialize};

use crate::ast::SqltStatement;
use crate::dialect::DialectId;

pub const SQLT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub sqlt_version: String,
    pub dialect: DialectId,
    pub statements: Vec<SqltStatement>,
}

impl Envelope {
    pub fn new(dialect: DialectId, statements: Vec<SqltStatement>) -> Self {
        Self {
            sqlt_version: SQLT_VERSION.to_string(),
            dialect,
            statements,
        }
    }
}
