use std::ffi::OsStr;
use std::path::Path;

use crate::cli::{LintArgs, examples, read_input_bytes, read_input_text};
use crate::encoding::Encoding;
use crate::error::{Error, Result};
use crate::lint::schema::{Schema, SchemaSkip};
use crate::lint::{self, LintOptions, Severity, format};
use crate::parse;

pub fn run(args: LintArgs) -> Result<()> {
    // Short-circuits that don't need input.
    if args.examples {
        examples::print(examples::LINT);
        return Ok(());
    }
    if args.list_rules {
        print_rule_list();
        return Ok(());
    }
    if let Some(id) = args.explain.as_deref() {
        let meta = lint::registry::find_meta(id)?;
        println!("{}  ({})", meta.id, meta.name);
        println!("  category: {}", meta.category.as_str());
        println!(
            "  default:  {}",
            if meta.default_enabled { "on" } else { "off" }
        );
        println!("  severity: {}", meta.default_severity.as_str());
        println!();
        println!("{}", meta.summary);
        println!();
        println!("{}", meta.explanation);
        return Ok(());
    }

    let from = args.from.ok_or_else(|| {
        Error::UnknownDialect(
            "--from is required (or pass --examples / --explain / --list-rules)".into(),
        )
    })?;

    let source_label: String = match args.input.as_deref() {
        Some(p) if p.as_os_str() != "-" => p.display().to_string(),
        _ => "<stdin>".to_string(),
    };

    // Build the external schema by replaying every --schema file in CLI
    // order. Files with a `.json` extension are loaded as a previously
    // built artifact (from `sqlt build-schema`); everything else is
    // parsed and replayed.
    let mut schema = Schema::default();
    let mut all_skips: Vec<SchemaSkip> = Vec::new();
    for path in &args.schemas {
        load_schema_file(path, from, args.encoding, &mut schema, &mut all_skips)?;
    }
    for s in &all_skips {
        eprintln!("note: {}", s.render());
    }

    let sql = read_input_text(args.input.as_deref(), args.encoding)?;
    let stmts = parse::parse(&sql, from)?;

    let mut enable = args.rule.clone();
    if args.verbose {
        enable.push("SQLT0001".to_string());
    }
    let opts = LintOptions {
        enable,
        disable: args.no_rule.clone(),
    };
    let external = if args.schemas.is_empty() {
        None
    } else {
        Some(schema)
    };
    let mut diagnostics = lint::lint(&stmts, &sql, from, args.to, &opts, external)?;
    lint::sort(&mut diagnostics);

    // Compute exit threshold against the *unfiltered* diagnostics so a
    // severity filter on output doesn't accidentally suppress an exit.
    let exit_threshold = args.exit_on.severity();
    let any_at_threshold = diagnostics.iter().any(|d| d.severity <= exit_threshold);

    let display_threshold = args.severity.severity();
    let displayed: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity <= display_threshold)
        .cloned()
        .collect();

    let help_mode = if args.no_help {
        format::HelpMode::Never
    } else {
        args.help_mode.into()
    };
    let output = format::render_with(args.format, &source_label, &sql, &displayed, help_mode)?;
    print!("{output}");

    if any_at_threshold {
        return Err(Error::LintFindings);
    }
    Ok(())
}

/// Render the `--list-rules` table. Sorted by rule id.
fn print_rule_list() {
    let rules = lint::registry::all_rules();
    println!(
        "{:<10}  {:<35}  {:<13}  {:<7}  {:<8}  SUMMARY",
        "ID", "SLUG", "CATEGORY", "SEV", "DEFAULT"
    );
    let bar: String = "─".repeat(120);
    println!("{bar}");
    for r in &rules {
        let m = r.meta();
        println!(
            "{:<10}  {:<35}  {:<13}  {:<7}  {:<8}  {}",
            m.id.as_str(),
            m.name,
            m.category.as_str(),
            m.default_severity.as_str(),
            if m.default_enabled { "on" } else { "off" },
            m.summary,
        );
    }
    println!();
    println!(
        "{} rule{} total ({} on by default, {} off)",
        rules.len(),
        if rules.len() == 1 { "" } else { "s" },
        rules.iter().filter(|r| r.meta().default_enabled).count(),
        rules.iter().filter(|r| !r.meta().default_enabled).count(),
    );
    println!();
    println!("Run `sqlt lint --explain <ID|SLUG>` for full documentation on any rule.");
}

/// Load a single `--schema` file into the running schema. Dispatches on
/// extension: `.json` is a serialized `Schema` from `sqlt build-schema`;
/// everything else is parsed and replayed.
pub(crate) fn load_schema_file(
    path: &Path,
    from: crate::dialect::DialectId,
    encoding: Encoding,
    schema: &mut Schema,
    skips: &mut Vec<SchemaSkip>,
) -> Result<()> {
    if path.extension().and_then(OsStr::to_str) == Some("json") {
        let bytes = read_input_bytes(Some(path))?;
        let raw = Encoding::Utf8.decode(&bytes)?;
        let loaded: Schema = serde_json::from_str(&raw)?;
        if !loaded.sqlt_version.is_empty() && !version_compatible(&loaded.sqlt_version) {
            eprintln!(
                "note: schema artifact at {} was built with sqlt {} (current {}); attempting to load anyway",
                path.display(),
                loaded.sqlt_version,
                env!("CARGO_PKG_VERSION"),
            );
        }
        schema.merge(&loaded);
        return Ok(());
    }
    let text = read_input_text(Some(path), encoding)?;
    let stmts = parse::parse(&text, from)?;
    for stmt in &stmts {
        schema.apply_statement(stmt, path, skips);
    }
    Ok(())
}

fn version_compatible(loaded: &str) -> bool {
    // Match on major.minor — any patch difference is fine.
    let cur = env!("CARGO_PKG_VERSION");
    let take_two = |v: &str| -> Option<(String, String)> {
        let mut parts = v.splitn(3, '.');
        Some((parts.next()?.to_string(), parts.next()?.to_string()))
    };
    take_two(loaded) == take_two(cur)
}

/// `Severity` ordering: `Error < Warning < Info` per derive(Ord). To mean
/// "diagnostic at or above the threshold" we use `<=` against the chosen
/// threshold severity.
impl crate::cli::ExitOn {
    pub fn severity(self) -> Severity {
        match self {
            crate::cli::ExitOn::Error => Severity::Error,
            crate::cli::ExitOn::Warning => Severity::Warning,
            crate::cli::ExitOn::Info => Severity::Info,
        }
    }
}
