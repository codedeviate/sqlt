use crate::cli::{ParseArgs, read_input};
use crate::error::Result;
use crate::json::{self, Envelope};
use crate::parse;

pub fn run(args: ParseArgs) -> Result<()> {
    let sql = read_input(args.input.as_deref())?;
    let stmts = parse::parse(&sql, args.from)?;
    let env = Envelope::new(args.from, stmts);
    let out = json::serialize(&env, args.pretty)?;
    println!("{out}");
    Ok(())
}
