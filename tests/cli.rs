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
fn multi_statement_input_parses_all_statements() {
    let multi = "SELECT 1; INSERT INTO t (a) VALUES (1); UPDATE t SET a = 2 WHERE a = 1";
    let (json, _, code) = run(sqlt().args(["parse", "--from", "mysql"]), multi);
    assert_eq!(code, 0);
    assert!(json.contains("\"Query\""));
    assert!(json.contains("\"Insert\""));
    assert!(json.contains("\"Update\""));
}
