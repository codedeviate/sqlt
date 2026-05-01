use serde::{Deserialize, Serialize};
use sqlparser::ast::Statement;

use crate::dialect::DialectId;

pub const SQLT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub sqlt_version: String,
    pub dialect: DialectId,
    pub statements: Vec<Statement>,
}

impl Envelope {
    pub fn new(dialect: DialectId, statements: Vec<Statement>) -> Self {
        Self {
            sqlt_version: SQLT_VERSION.to_string(),
            dialect,
            statements,
        }
    }
}
