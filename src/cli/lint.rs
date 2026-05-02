use crate::cli::{LintArgs, examples, read_input_text};
use crate::error::{Error, Result};
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
    let mut diagnostics = lint::lint(&stmts, &sql, from, args.to, &opts)?;
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
