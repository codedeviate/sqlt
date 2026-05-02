use crate::cli::{TranslateArgs, examples, read_input_text, write_sql};
use crate::error::{Error, Result};
use crate::translate::{self, StderrSink, TranslateOptions};

pub fn run(args: TranslateArgs) -> Result<()> {
    if args.examples {
        examples::print(examples::TRANSLATE);
        return Ok(());
    }
    let from = args
        .from
        .ok_or_else(|| Error::UnknownDialect("--from is required".into()))?;
    let to = args
        .to
        .ok_or_else(|| Error::UnknownDialect("--to is required".into()))?;
    let sql = read_input_text(args.input.as_deref(), args.encoding)?;
    let mut sink = StderrSink::new();
    let opts = TranslateOptions {
        strict: args.strict,
    };
    let out = translate::translate(&sql, from, to, &mut sink, &opts)?;
    write_sql(&out, args.encoding)?;
    Ok(())
}
