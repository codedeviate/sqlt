//! `sqlt build-schema` — compile reusable schema artifact from SQL files.

use std::io::Write;

use crate::cli::lint::load_schema_file;
use crate::cli::{BuildSchemaArgs, examples};
use crate::error::{Error, Result};
use crate::lint::schema::{Schema, SchemaSkip};

pub fn run(args: BuildSchemaArgs) -> Result<()> {
    if args.examples {
        examples::print(examples::BUILD_SCHEMA);
        return Ok(());
    }
    let from = args
        .from
        .ok_or_else(|| Error::UnknownDialect("--from is required (or pass --examples)".into()))?;
    if args.schemas.is_empty() {
        return Err(Error::UnknownDialect(
            "at least one --schema <file> is required".into(),
        ));
    }

    let mut schema = Schema::default();
    let mut skips: Vec<SchemaSkip> = Vec::new();
    for path in &args.schemas {
        load_schema_file(path, from, args.encoding, &mut schema, &mut skips)?;
    }
    for s in &skips {
        eprintln!("note: {}", s.render());
    }

    schema.sqlt_version = env!("CARGO_PKG_VERSION").to_string();
    let json = if args.pretty {
        serde_json::to_string_pretty(&schema)?
    } else {
        serde_json::to_string(&schema)?
    };

    let total_tables: usize = schema.databases.values().map(|d| d.tables.len()).sum();
    let total_dbs = schema
        .databases
        .iter()
        .filter(|(k, d)| !k.is_empty() || !d.tables.is_empty())
        .count();
    eprintln!(
        "built schema: {} database{}, {} table{}, {} skipped",
        total_dbs,
        if total_dbs == 1 { "" } else { "s" },
        total_tables,
        if total_tables == 1 { "" } else { "s" },
        skips.len(),
    );

    if let Some(path) = args.output.as_deref() {
        std::fs::write(path, &json)?;
    } else {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        out.write_all(json.as_bytes())?;
        out.write_all(b"\n")?;
    }
    Ok(())
}
