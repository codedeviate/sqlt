//! Default single-line text format. Grep-friendly.
//!
//! Format:
//!   <source>:<line>:<col>: <severity> [<rule_id>] <message>
//!   <source>:<line>:<col>:   help: <suggestion>     (only on first
//!                                                    occurrence of each
//!                                                    (rule, suggestion)
//!                                                    within a render)
//! footer:
//!   N diagnostics: E errors, W warnings, I info
//!
//! Help lines are deduplicated per `(rule_id, suggestion)` — for SQLT0001
//! with hundreds of identical raw-passthrough findings the help shows once
//! at the first occurrence and is implied for the rest. Pass
//! `HelpMode::Always` to restore the old per-finding rendering, or
//! `HelpMode::Never` to suppress help entirely.

use std::collections::HashSet;
use std::fmt::Write;

use crate::lint::Diagnostic;
use crate::lint::rule::RuleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HelpMode {
    /// Emit the help line on the first occurrence of each
    /// `(rule_id, suggestion)` pair. Default.
    #[default]
    Auto,
    /// Emit a help line under every diagnostic that has one.
    Always,
    /// Never emit a help line.
    Never,
}

/// Render with the default `HelpMode::Auto`.
pub fn render(source: &str, diagnostics: &[Diagnostic]) -> String {
    render_with(source, diagnostics, HelpMode::Auto)
}

pub fn render_with(source: &str, diagnostics: &[Diagnostic], help: HelpMode) -> String {
    let mut out = String::new();
    let (mut e, mut w, mut i) = (0usize, 0usize, 0usize);
    let mut help_seen: HashSet<(RuleId, String)> = HashSet::new();
    for d in diagnostics {
        let (line, col) = if d.has_span() {
            (d.span.start.line, d.span.start.column)
        } else {
            (1, 1)
        };
        let _ = writeln!(
            &mut out,
            "{source}:{line}:{col}: {} [{}] {}",
            d.severity.as_str(),
            d.rule,
            d.message
        );
        if let Some(s) = &d.suggestion {
            let show_help = match help {
                HelpMode::Always => true,
                HelpMode::Never => false,
                HelpMode::Auto => help_seen.insert((d.rule, s.clone())),
            };
            if show_help {
                let _ = writeln!(&mut out, "{source}:{line}:{col}:   help: {s}");
            }
        }
        match d.severity {
            crate::lint::Severity::Error => e += 1,
            crate::lint::Severity::Warning => w += 1,
            crate::lint::Severity::Info => i += 1,
        }
    }
    let total = diagnostics.len();
    if total == 0 {
        out.push_str("0 diagnostics\n");
    } else {
        let _ = writeln!(
            &mut out,
            "{total} diagnostic{}: {e} error{}, {w} warning{}, {i} info",
            if total == 1 { "" } else { "s" },
            if e == 1 { "" } else { "s" },
            if w == 1 { "" } else { "s" },
        );
    }
    out
}
