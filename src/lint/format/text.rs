//! Default single-line text format. Grep-friendly, no color in v1
//! (color comes back in M2 along with `pretty`/`json`/`sarif`).
//!
//! Format:
//!   <source>:<line>:<col>: <severity> [<rule_id>] <message>
//!   <source>:<line>:<col>:   help: <suggestion>     (optional)
//! followed by a footer:
//!   N diagnostics: E errors, W warnings, I info

use std::fmt::Write;

use crate::lint::Diagnostic;

/// Render diagnostics. `source` is the user-supplied path or the literal
/// `"<stdin>"`.
pub fn render(source: &str, diagnostics: &[Diagnostic]) -> String {
    let mut out = String::new();
    let (mut e, mut w, mut i) = (0usize, 0usize, 0usize);
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
            let _ = writeln!(&mut out, "{source}:{line}:{col}:   help: {s}");
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
