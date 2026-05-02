//! End-to-end CLI integration tests. Spawns the built `sqlt` binary so we
//! exercise the actual exit codes and stdin/stdout/stderr wiring.

use std::io::Write;
use std::process::{Command, Stdio};

fn sqlt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sqlt"))
}

fn run(cmd: &mut Command, stdin: &str) -> (String, String, i32) {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sqlt");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    (
        String::from_utf8(out.stdout).expect("stdout utf8"),
        String::from_utf8(out.stderr).expect("stderr utf8"),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn parse_pipe_emit_roundtrips() {
    let (json, _, code) = run(sqlt().args(["parse", "--from", "mysql"]), "SELECT 1");
    assert_eq!(code, 0);
    let (sql, _, code) = run(sqlt().args(["emit", "--to", "mysql"]), &json);
    assert_eq!(code, 0);
    assert!(sql.trim_start().to_uppercase().starts_with("SELECT"));
}

#[test]
fn translate_drops_returning_with_warning() {
    let (sql, stderr, code) = run(
        sqlt().args(["translate", "--from", "mariadb", "--to", "mysql"]),
        "INSERT INTO t (a) VALUES (1) RETURNING id",
    );
    assert_eq!(
        code, 0,
        "exit code should be 0 (warnings non-fatal by default)"
    );
    assert!(
        !sql.contains("RETURNING"),
        "RETURNING should be dropped: {sql:?}"
    );
    assert!(
        stderr.contains("RETURNING_DROPPED"),
        "stderr should carry warning: {stderr:?}"
    );
}

#[test]
fn translate_strict_exits_three_on_warnings() {
    let (_, stderr, code) = run(
        sqlt().args([
            "translate",
            "--from",
            "mariadb",
            "--to",
            "mysql",
            "--strict",
        ]),
        "INSERT INTO t (a) VALUES (1) RETURNING id",
    );
    assert_eq!(code, 3, "strict mode must exit 3 on any warning");
    assert!(stderr.contains("RETURNING_DROPPED"));
}

#[test]
fn parse_error_exits_one_with_message() {
    let (_, stderr, code) = run(sqlt().args(["parse", "--from", "mysql"]), "SELECT FROM");
    assert_eq!(code, 1);
    assert!(
        stderr.contains("parse error"),
        "stderr should mention parse error: {stderr:?}"
    );
}

#[test]
fn unknown_dialect_exits_two() {
    // clap rejects the value before our code runs; clap returns exit code 2.
    let (_, stderr, code) = run(sqlt().args(["parse", "--from", "bogusdb"]), "");
    assert_eq!(code, 2);
    assert!(
        stderr.contains("bogusdb") || stderr.contains("invalid"),
        "stderr should explain invalid dialect: {stderr:?}"
    );
}

#[test]
fn parse_latin1_input_with_encoding_flag() {
    // Build Latin-1 bytes that aren't valid UTF-8: SELECT 'café'
    let mut bytes = b"SELECT 'caf".to_vec();
    bytes.push(0xE9); // é in Latin-1
    bytes.extend_from_slice(b"' FROM t");

    // Write raw bytes to a temp file so the binary reads them unchanged.
    let tmp = std::env::temp_dir().join("sqlt_latin1_test.sql");
    std::fs::write(&tmp, &bytes).expect("write tmp");

    // Default UTF-8 mode must reject the file because 0xE9 alone is invalid UTF-8.
    let (_, stderr, code_default) = run(
        sqlt().args(["parse", "--from", "mysql", tmp.to_str().unwrap()]),
        "",
    );
    assert_eq!(
        code_default, 1,
        "default UTF-8 should reject Latin-1 file (got stderr: {stderr:?})"
    );
    assert!(
        stderr.contains("encoding") || stderr.contains("utf-8"),
        "stderr should mention encoding error: {stderr:?}"
    );

    // With --encoding latin1 it should parse and produce UTF-8 JSON.
    let (json, _, code_latin1) = run(
        sqlt().args([
            "parse",
            "--from",
            "mysql",
            "-e",
            "latin1",
            tmp.to_str().unwrap(),
        ]),
        "",
    );
    assert_eq!(
        code_latin1, 0,
        "latin1 decoding should succeed for high-bit bytes"
    );
    assert!(
        json.contains("café"),
        "JSON output (always UTF-8) should contain the decoded code points: {json:?}"
    );
}

#[test]
fn translate_latin1_preserves_bytes_through_pipeline() {
    // SELECT 'naïve' in latin1: 0xEF for ï.
    let mut bytes = b"SELECT 'na".to_vec();
    bytes.push(0xEF);
    bytes.extend_from_slice(b"ve'");
    let tmp = std::env::temp_dir().join("sqlt_latin1_translate.sql");
    std::fs::write(&tmp, &bytes).expect("write tmp");

    let child = sqlt()
        .args([
            "translate",
            "--from",
            "mysql",
            "--to",
            "mariadb",
            "-e",
            "latin1",
            tmp.to_str().unwrap(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert_eq!(out.status.code().unwrap(), 0);
    // Output must contain 0xEF verbatim (no UTF-8 expansion).
    assert!(
        out.stdout.contains(&0xEF),
        "expected raw latin1 byte 0xEF in output, got {:?}",
        out.stdout
    );
    // And must not contain UTF-8 multibyte for ï (0xC3 0xAF).
    assert!(
        !out.stdout.windows(2).any(|w| w == [0xC3, 0xAF]),
        "output must be latin1, not utf-8: {:?}",
        out.stdout
    );
}

#[test]
fn lint_select_star_warns_and_exits_zero_by_default() {
    let (stdout, _, code) = run(
        sqlt().args(["lint", "--from", "mysql"]),
        "SELECT * FROM users",
    );
    assert_eq!(
        code, 0,
        "info severity below default --exit-on=error threshold"
    );
    assert!(
        stdout.contains("SQLT0500"),
        "stdout missing rule id: {stdout:?}"
    );
    assert!(
        stdout.contains("help:"),
        "stdout missing help line: {stdout:?}"
    );
}

#[test]
fn lint_exit_on_info_promotes_to_failure() {
    let (_, _, code) = run(
        sqlt().args(["lint", "--from", "mysql", "--exit-on", "info"]),
        "SELECT * FROM users",
    );
    assert_eq!(code, 1);
}

#[test]
fn lint_no_rule_disables() {
    let (stdout, _, code) = run(
        sqlt().args(["lint", "--from", "mysql", "--no-rule", "SQLT0500"]),
        "SELECT * FROM users",
    );
    assert_eq!(code, 0);
    assert!(
        !stdout.contains("SQLT0500"),
        "rule should be disabled: {stdout:?}"
    );
    assert!(stdout.contains("0 diagnostics"));
}

#[test]
fn lint_unknown_rule_exits_two() {
    let (_, stderr, code) = run(
        sqlt().args(["lint", "--from", "mysql", "--no-rule", "SQLT9999"]),
        "SELECT 1",
    );
    assert_eq!(code, 2);
    assert!(stderr.contains("unknown rule"), "stderr: {stderr:?}");
}

#[test]
fn lint_explain_prints_docs_and_exits_zero() {
    let (stdout, _, code) = run(sqlt().args(["lint", "--explain", "SQLT0500"]), "");
    assert_eq!(code, 0);
    assert!(stdout.contains("SQLT0500"));
    assert!(stdout.contains("select-star"));
    assert!(stdout.contains("category: perf"));
}

#[test]
fn lint_schema_flag_alter_drop_column_actually_drops_it() {
    // Schema declares column `b`, then ALTER drops it. A query that
    // references `t.b` must fire SQLT0900 because the schema's current
    // state has only column `a`.
    let tmp_schema = std::env::temp_dir().join("sqlt_schema_alter_drop.sql");
    std::fs::write(
        &tmp_schema,
        "CREATE TABLE t (a INT, b INT); ALTER TABLE t DROP COLUMN b",
    )
    .expect("write schema");

    let (stdout, _, code) = run(
        sqlt().args([
            "lint",
            "--from",
            "mysql",
            "--schema",
            tmp_schema.to_str().unwrap(),
        ]),
        "SELECT t.b FROM t",
    );
    assert_eq!(code, 1, "SQLT0900 should fire — column was dropped");
    assert!(
        stdout.contains("SQLT0900"),
        "expected SQLT0900 in stdout: {stdout:?}"
    );
}

#[test]
fn lint_schema_unknown_statement_warns_to_stderr() {
    let tmp = std::env::temp_dir().join("sqlt_schema_unknown.sql");
    std::fs::write(&tmp, "INSERT INTO seed VALUES (1)").expect("write");

    let (_, stderr, code) = run(
        sqlt().args(["lint", "--from", "mysql", "--schema", tmp.to_str().unwrap()]),
        "SELECT 1",
    );
    assert_eq!(code, 0);
    assert!(
        stderr.contains("note: skipping Insert"),
        "stderr should include the skip note: {stderr:?}"
    );
}

#[test]
fn lint_schema_drop_missing_table_warns_not_errors() {
    let tmp = std::env::temp_dir().join("sqlt_schema_drop_missing.sql");
    std::fs::write(&tmp, "DROP TABLE nonexistent").expect("write");

    let (_, stderr, code) = run(
        sqlt().args(["lint", "--from", "mysql", "--schema", tmp.to_str().unwrap()]),
        "SELECT 1",
    );
    assert_eq!(code, 0);
    assert!(
        stderr.contains("DROP TABLE on missing table"),
        "got: {stderr:?}"
    );
}

#[test]
fn build_schema_then_lint_using_json_artifact() {
    let tmp_sch = std::env::temp_dir().join("sqlt_bs_input.sql");
    std::fs::write(
        &tmp_sch,
        "CREATE TABLE users (id INT NOT NULL); \
         ALTER TABLE users ADD COLUMN email VARCHAR(255)",
    )
    .expect("write sch");
    let tmp_json = std::env::temp_dir().join("sqlt_bs_artifact.json");

    // Step 1: compile
    let (_, stderr_build, code_build) = run(
        sqlt().args([
            "build-schema",
            "--from",
            "mysql",
            "--schema",
            tmp_sch.to_str().unwrap(),
            "-o",
            tmp_json.to_str().unwrap(),
        ]),
        "",
    );
    assert_eq!(code_build, 0, "build-schema failed: {stderr_build:?}");

    // Step 2: lint using the artifact (no .sql replay needed)
    let (_, _, code_clean) = run(
        sqlt().args([
            "lint",
            "--from",
            "mysql",
            "--schema",
            tmp_json.to_str().unwrap(),
        ]),
        "SELECT u.email FROM users u",
    );
    assert_eq!(code_clean, 0, "email should resolve from the artifact");

    // Step 3: typo gets caught
    let (stdout, _, code_typo) = run(
        sqlt().args([
            "lint",
            "--from",
            "mysql",
            "--schema",
            tmp_json.to_str().unwrap(),
        ]),
        "SELECT u.bogus FROM users u",
    );
    assert_eq!(code_typo, 1);
    assert!(stdout.contains("SQLT0900"), "got: {stdout:?}");
}

#[test]
fn build_schema_then_lint_with_late_sql_migration() {
    let tmp_sch = std::env::temp_dir().join("sqlt_bs_base.sql");
    std::fs::write(&tmp_sch, "CREATE TABLE t (id INT)").expect("write base");
    let tmp_json = std::env::temp_dir().join("sqlt_bs_base.json");

    // Compile the base schema
    let (_, _, code) = run(
        sqlt().args([
            "build-schema",
            "--from",
            "mysql",
            "--schema",
            tmp_sch.to_str().unwrap(),
            "-o",
            tmp_json.to_str().unwrap(),
        ]),
        "",
    );
    assert_eq!(code, 0);

    // Late-arriving migration (raw SQL, not yet rolled into the artifact).
    let tmp_late = std::env::temp_dir().join("sqlt_bs_late.sql");
    std::fs::write(&tmp_late, "ALTER TABLE t ADD COLUMN created_at TIMESTAMP").expect("write late");

    // Mix .json + .sql in CLI order — the late migration applies on top.
    let (_, _, code) = run(
        sqlt().args([
            "lint",
            "--from",
            "mysql",
            "--schema",
            tmp_json.to_str().unwrap(),
            "--schema",
            tmp_late.to_str().unwrap(),
        ]),
        "SELECT t.created_at FROM t",
    );
    assert_eq!(code, 0, "the new column should resolve");
}

#[test]
fn lint_multi_database_no_collision() {
    let tmp = std::env::temp_dir().join("sqlt_multidb.sql");
    std::fs::write(
        &tmp,
        "USE shop_db; CREATE TABLE orders (id INT NOT NULL, sid INT NOT NULL); \
         USE global_db; CREATE TABLE orders (uid INT NOT NULL, payload TEXT)",
    )
    .expect("write");

    // Reference shop_db.orders.sid — present, no warning.
    let (_, _, code) = run(
        sqlt().args(["lint", "--from", "mysql", "--schema", tmp.to_str().unwrap()]),
        "SELECT shop_db.orders.sid FROM shop_db.orders",
    );
    assert_eq!(code, 0);

    // Reference shop_db.orders.uid — uid is in global_db, not shop_db. SQLT0900.
    let (stdout, _, code) = run(
        sqlt().args(["lint", "--from", "mysql", "--schema", tmp.to_str().unwrap()]),
        "SELECT shop_db.orders.uid FROM shop_db.orders",
    );
    assert_eq!(code, 1);
    assert!(stdout.contains("SQLT0900"), "got: {stdout:?}");
    assert!(
        stdout.contains("shop_db.orders"),
        "diagnostic should mention the resolved table: {stdout:?}"
    );
}

#[test]
fn lint_multiple_schema_flags_processed_in_order() {
    // First file creates the table; second file adds a column. The query
    // references the column added by the second file — must succeed.
    let a = std::env::temp_dir().join("sqlt_schema_a.sql");
    std::fs::write(&a, "CREATE TABLE t (id INT)").expect("write a");
    let b = std::env::temp_dir().join("sqlt_schema_b.sql");
    std::fs::write(&b, "ALTER TABLE t ADD COLUMN email VARCHAR(100)").expect("write b");

    let (stdout, _, code) = run(
        sqlt().args([
            "lint",
            "--from",
            "mysql",
            "--schema",
            a.to_str().unwrap(),
            "--schema",
            b.to_str().unwrap(),
        ]),
        "SELECT t.email FROM t",
    );
    assert_eq!(code, 0, "no SQLT0900 expected; stdout: {stdout:?}");
}

#[test]
fn unknown_encoding_exits_two() {
    let (_, stderr, code) = run(
        sqlt().args(["parse", "--from", "mysql", "-e", "ebcdic"]),
        "SELECT 1",
    );
    assert_eq!(code, 2);
    assert!(
        stderr.contains("ebcdic") || stderr.contains("encoding"),
        "stderr should mention the bad encoding: {stderr:?}"
    );
}

#[test]
fn multi_statement_input_parses_all_statements() {
    let multi = "SELECT 1; INSERT INTO t (a) VALUES (1); UPDATE t SET a = 2 WHERE a = 1";
    let (json, _, code) = run(sqlt().args(["parse", "--from", "mysql"]), multi);
    assert_eq!(code, 0);
    assert!(json.contains("\"Query\""));
    assert!(json.contains("\"Insert\""));
    assert!(json.contains("\"Update\""));
}
