use crate::cli::{EmitArgs, read_input};
use crate::dialect::DialectId;
use crate::emit;
use crate::error::Result;
use crate::json;

pub fn run(args: EmitArgs) -> Result<()> {
    let raw = read_input(args.input.as_deref())?;
    let env = json::deserialize(&raw)?;
    let dialect: DialectId = args.to.unwrap_or(env.dialect);
    let sql = emit::emit(&env.statements, dialect)?;
    println!("{sql}");
    Ok(())
}
