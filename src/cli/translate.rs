use crate::cli::{TranslateArgs, read_input_text, write_sql};
use crate::error::Result;
use crate::translate::{self, StderrSink, TranslateOptions};

pub fn run(args: TranslateArgs) -> Result<()> {
    let sql = read_input_text(args.input.as_deref(), args.encoding)?;
    let mut sink = StderrSink::new();
    let opts = TranslateOptions {
        strict: args.strict,
    };
    let out = translate::translate(&sql, args.from, args.to, &mut sink, &opts)?;
    write_sql(&out, args.encoding)?;
    Ok(())
}
