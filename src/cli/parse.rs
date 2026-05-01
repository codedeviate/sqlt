use crate::cli::{ParseArgs, read_input_text};
use crate::error::Result;
use crate::json::{self, Envelope};
use crate::parse;

pub fn run(args: ParseArgs) -> Result<()> {
    let sql = read_input_text(args.input.as_deref(), args.encoding)?;
    let stmts = parse::parse(&sql, args.from)?;
    let env = Envelope::new(args.from, stmts);
    let out = json::serialize(&env, args.pretty)?;
    // JSON output is always UTF-8 per spec.
    println!("{out}");
    Ok(())
}
