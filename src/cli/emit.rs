use crate::cli::{EmitArgs, read_input_bytes, write_sql};
use crate::dialect::DialectId;
use crate::emit;
use crate::encoding::Encoding;
use crate::error::Result;
use crate::json;

pub fn run(args: EmitArgs) -> Result<()> {
    // JSON input is always UTF-8 per spec — decode strictly as UTF-8
    // regardless of --encoding, which only governs the SQL output.
    let bytes = read_input_bytes(args.input.as_deref())?;
    let raw = Encoding::Utf8.decode(&bytes)?;
    let env = json::deserialize(&raw)?;
    let dialect: DialectId = args.to.unwrap_or(env.dialect);
    let sql = emit::emit(&env.statements, dialect)?;
    write_sql(&sql, args.encoding)?;
    Ok(())
}
