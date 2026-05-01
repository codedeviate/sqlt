//! Golden translation tests.
//!
//! For every directory `tests/fixtures/translations/<src>__<dst>/` we walk
//! every `<case>.in.sql` and assert that translating it from `<src>` to
//! `<dst>` produces SQL matching `<case>.expected.sql` and the warning set
//! matching `<case>.expected.warn` (one WarnCode per line, sorted).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use sqlt::dialect::DialectId;
use sqlt::translate::{self, CollectingSink, TranslateOptions};

fn fixtures_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/translations")
}

fn parse_dir_name(name: &str) -> Option<(DialectId, DialectId)> {
    let (s, d) = name.split_once("__")?;
    Some((s.parse().ok()?, d.parse().ok()?))
}

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n").trim().to_string()
}

#[test]
fn golden_translation_fixtures() {
    let root = fixtures_root();
    if !root.exists() {
        return;
    }
    let mut total = 0usize;
    for pair in fs::read_dir(&root).expect("read translations dir") {
        let pair = pair.expect("entry").path();
        if !pair.is_dir() {
            continue;
        }
        let dir_name = pair.file_name().unwrap().to_string_lossy().into_owned();
        let (src, dst) = parse_dir_name(&dir_name)
            .unwrap_or_else(|| panic!("invalid translations subdir name: {dir_name}"));

        for entry in fs::read_dir(&pair).expect("read pair dir") {
            let path = entry.expect("entry").path();
            let fname = path.file_name().unwrap().to_string_lossy().into_owned();
            let case = match fname.strip_suffix(".in.sql") {
                Some(c) => c,
                None => continue,
            };
            let in_sql = fs::read_to_string(&path).expect("read .in.sql");
            let expected_sql_path = pair.join(format!("{case}.expected.sql"));
            let expected_warn_path = pair.join(format!("{case}.expected.warn"));
            let expected_sql = fs::read_to_string(&expected_sql_path)
                .unwrap_or_else(|_| panic!("missing {}", expected_sql_path.display()));
            let expected_warn_raw = fs::read_to_string(&expected_warn_path).unwrap_or_default();

            let mut sink = CollectingSink::default();
            let opts = TranslateOptions::default();
            let actual_sql = translate::translate(&in_sql, src, dst, &mut sink, &opts)
                .unwrap_or_else(|e| {
                    panic!("translate failed for {dir_name}/{case}: {e}");
                });

            assert_eq!(
                normalize(&actual_sql),
                normalize(&expected_sql),
                "SQL mismatch for {dir_name}/{case}\n  in:       {in_sql:?}\n  expected: {expected_sql:?}\n  actual:   {actual_sql:?}",
            );

            let actual_warns: BTreeSet<String> = sink
                .items
                .iter()
                .map(|w| w.code.as_str().to_string())
                .collect();
            let expected_warns: BTreeSet<String> = expected_warn_raw
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            assert_eq!(
                actual_warns, expected_warns,
                "warning set mismatch for {dir_name}/{case}\n  expected: {expected_warns:?}\n  actual:   {actual_warns:?}",
            );

            total += 1;
        }
    }
    assert!(total > 0, "no translation fixtures found");
}
