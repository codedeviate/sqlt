use crate::cli::{LintArgs, read_input_text};
use crate::error::{Error, Result};
use crate::lint::{self, LintOptions, Severity, format};
use crate::parse;

pub fn run(args: LintArgs) -> Result<()> {
    // --explain short-circuits before any parsing.
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

    let from = args
        .from
        .ok_or_else(|| Error::UnknownDialect("--from is required (or pass --explain)".into()))?;

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
