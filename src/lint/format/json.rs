//! JSON output. Pretty-printed for diff stability.
//!
//! Schema:
//! ```json
//! {
//!   "sqlt_version": "0.2.0",
//!   "source": "schema.sql",
//!   "source_dialect": "mariadb",
//!   "target_dialect": "postgres",
//!   "diagnostics": [
//!     { "rule": "SQLT0500", "name": "select-star", "category": "perf",
//!       "severity": "info", "message": "...", "suggestion": "...",
//!       "stmt_index": 0,
//!       "span": { "start": {"line":1,"column":8}, "end": {"line":1,"column":9} } }
//!   ],
//!   "summary": { "errors": 0, "warnings": 0, "info": 1 }
//! }
//! ```
//! Empty spans serialize as `null` so consumers can distinguish "no span"
//! from "(0,0)".

use serde::Serialize;

use crate::lint::Diagnostic;

#[derive(Serialize)]
struct DiagnosticOut<'a> {
    rule: &'a str,
    name: &'a str,
    category: &'a str,
    severity: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<&'a str>,
    stmt_index: usize,
    source_dialect: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_dialect: Option<&'a str>,
    span: Option<SpanOut>,
}

#[derive(Serialize)]
struct SpanOut {
    start: Loc,
    end: Loc,
}

#[derive(Serialize)]
struct Loc {
    line: u64,
    column: u64,
}

#[derive(Serialize)]
struct Summary {
    errors: usize,
    warnings: usize,
    info: usize,
}

#[derive(Serialize)]
struct Output<'a> {
    sqlt_version: &'static str,
    source: &'a str,
    diagnostics: Vec<DiagnosticOut<'a>>,
    summary: Summary,
}

pub fn render(source: &str, diagnostics: &[Diagnostic]) -> String {
    let (mut e, mut w, mut i) = (0, 0, 0);
    let out_diags: Vec<DiagnosticOut> = diagnostics
        .iter()
        .map(|d| {
            match d.severity {
                crate::lint::Severity::Error => e += 1,
                crate::lint::Severity::Warning => w += 1,
                crate::lint::Severity::Info => i += 1,
            }
            DiagnosticOut {
                rule: d.rule.as_str(),
                name: d.rule_name,
                category: d.category.as_str(),
                severity: d.severity.as_str(),
                message: &d.message,
                suggestion: d.suggestion.as_deref(),
                stmt_index: d.stmt_index,
                source_dialect: d.source_dialect.as_str(),
                target_dialect: d.target_dialect.map(|x| x.as_str()),
                span: if d.has_span() {
                    Some(SpanOut {
                        start: Loc {
                            line: d.span.start.line,
                            column: d.span.start.column,
                        },
                        end: Loc {
                            line: d.span.end.line,
                            column: d.span.end.column,
                        },
                    })
                } else {
                    None
                },
            }
        })
        .collect();
    let out = Output {
        sqlt_version: env!("CARGO_PKG_VERSION"),
        source,
        diagnostics: out_diags,
        summary: Summary {
            errors: e,
            warnings: w,
            info: i,
        },
    };
    serde_json::to_string_pretty(&out).expect("serialize lint json")
}
