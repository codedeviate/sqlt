use crate::cli::{ParseArgs, examples, read_input_text};
use crate::error::{Error, Result};
use crate::json::{self, Envelope};
use crate::parse;

pub fn run(args: ParseArgs) -> Result<()> {
    if args.examples {
        examples::print(examples::PARSE);
        return Ok(());
    }
    let from = args
        .from
        .ok_or_else(|| Error::UnknownDialect("--from is required (or pass --examples)".into()))?;
    let sql = read_input_text(args.input.as_deref(), args.encoding)?;
    let stmts = parse::parse(&sql, from)?;
    let env = Envelope::new(from, stmts);
    let out = json::serialize(&env, args.pretty)?;
    // JSON output is always UTF-8 per spec.
    println!("{out}");
    Ok(())
}
