use crate::cli::{TranslateArgs, read_input};
use crate::error::Result;
use crate::translate::{self, StderrSink, TranslateOptions};

pub fn run(args: TranslateArgs) -> Result<()> {
    let sql = read_input(args.input.as_deref())?;
    let mut sink = StderrSink::new();
    let opts = TranslateOptions {
        strict: args.strict,
    };
    let out = translate::translate(&sql, args.from, args.to, &mut sink, &opts)?;
    println!("{out}");
    Ok(())
}
