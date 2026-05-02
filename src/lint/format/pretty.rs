//! Pretty / grouped-by-file format.
//!
//! Diagnostics are grouped by source file, with a snippet of the offending
//! line and the rule explanation inlined the first time each rule appears
//! in a file.

use std::collections::HashSet;
use std::fmt::Write;

use crate::lint::Diagnostic;
use crate::lint::registry;

pub fn render(source: &str, source_text: &str, diagnostics: &[Diagnostic]) -> String {
    let mut out = String::new();
    if diagnostics.is_empty() {
        let _ = writeln!(&mut out, "{source}: no diagnostics");
        return out;
    }
    let _ = writeln!(&mut out, "{source}");
    let bar: String = "═".repeat(source.chars().count().max(10));
    let _ = writeln!(&mut out, "{bar}");

    let lines: Vec<&str> = source_text.lines().collect();
    let mut shown_explanation: HashSet<&'static str> = HashSet::new();

    let all = registry::all_rules();
    for d in diagnostics {
        let _ = writeln!(
            &mut out,
            "  {sev} [{id}] {name}",
            sev = d.severity.as_str(),
            id = d.rule,
            name = d.rule_name
        );
        if d.has_span() {
            let line_idx = (d.span.start.line as usize).saturating_sub(1);
            let col = d.span.start.column as usize;
            let _ = writeln!(
                &mut out,
                "    at line {}, column {}",
                d.span.start.line, d.span.start.column
            );
            if let Some(line) = lines.get(line_idx) {
                let mut snippet: String = line.chars().take(100).collect();
                if line.chars().count() > 100 {
                    snippet.push('…');
                }
                let _ = writeln!(&mut out, "    │  {snippet}");
                let caret_col = col.saturating_sub(1);
                let _ = writeln!(&mut out, "    │  {}^", " ".repeat(caret_col));
            }
        } else {
            let _ = writeln!(&mut out, "    in statement {}", d.stmt_index);
        }
        let _ = writeln!(&mut out, "    {}", d.message);
        if let Some(s) = &d.suggestion {
            let _ = writeln!(&mut out, "    help: {s}");
        }
        if shown_explanation.insert(d.rule_name) {
            if let Some(meta) = all.iter().find(|r| r.meta().id == d.rule).map(|r| r.meta()) {
                let _ = writeln!(&mut out, "    explanation: {}", meta.explanation);
            }
        }
        out.push('\n');
    }

    let dash: String = "─".repeat(50);
    let _ = writeln!(&mut out, "{dash}");
    let (mut e, mut w, mut i) = (0, 0, 0);
    for d in diagnostics {
        match d.severity {
            crate::lint::Severity::Error => e += 1,
            crate::lint::Severity::Warning => w += 1,
            crate::lint::Severity::Info => i += 1,
        }
    }
    let total = diagnostics.len();
    let _ = writeln!(
        &mut out,
        "{total} diagnostic{} in {source}: {e} errors, {w} warnings, {i} info",
        if total == 1 { "" } else { "s" },
    );
    out
}
