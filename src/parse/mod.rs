use sqlparser::ast::Statement;
use sqlparser::parser::Parser;

use crate::dialect::DialectId;
use crate::error::Result;

pub fn parse(sql: &str, dialect: DialectId) -> Result<Vec<Statement>> {
    let upstream = dialect.upstream();
    let stmts = Parser::parse_sql(&*upstream, sql)?;
    Ok(stmts)
}
