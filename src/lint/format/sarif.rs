//! SARIF 2.1.0 output (GitHub code-scanning subset).
//!
//! We hand-build the SARIF tree as a `serde_json::Value` rather than pull a
//! SARIF crate — the surface we need is small and stable, and avoiding the
//! dep keeps the binary lean.
//!
//! Reference: <https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html>

use serde_json::{Value, json};

use crate::lint::Diagnostic;
use crate::lint::registry;

pub fn render(source: &str, diagnostics: &[Diagnostic]) -> String {
    // Build the rules section by inspecting the registry once. SARIF wants
    // every rule referenced in `results` to be declared in `tool.driver.rules`.
    let all = registry::all_rules();
    let rules_json: Vec<Value> = all
        .iter()
        .map(|r| {
            let m = r.meta();
            json!({
                "id": m.id.as_str(),
                "name": m.name,
                "shortDescription": { "text": m.summary },
                "fullDescription":  { "text": m.explanation },
                "defaultConfiguration": { "level": m.default_severity.sarif_level() },
                "properties": { "tags": ["sql", m.category.as_str()] }
            })
        })
        .collect();

    let results: Vec<Value> = diagnostics
        .iter()
        .map(|d| {
            let mut location = json!({
                "physicalLocation": {
                    "artifactLocation": { "uri": source }
                }
            });
            if d.has_span() {
                location["physicalLocation"]["region"] = json!({
                    "startLine":   d.span.start.line,
                    "startColumn": d.span.start.column,
                    "endLine":     d.span.end.line,
                    "endColumn":   d.span.end.column,
                });
            }
            json!({
                "ruleId": d.rule.as_str(),
                "level": d.severity.sarif_level(),
                "message": { "text": d.message },
                "locations": [location]
            })
        })
        .collect();

    let sarif = json!({
        "$schema": "https://docs.oasis-open.org/sarif/sarif/v2.1.0/cos02/schemas/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "sqlt",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/thomasbjork/sqlt",
                    "rules": rules_json
                }
            },
            "results": results
        }]
    });
    serde_json::to_string_pretty(&sarif).expect("serialize sarif")
}
