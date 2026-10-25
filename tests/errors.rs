//! Интеграционные тесты JSON-диагностик, `lupus check` и `lupus ast`
//! (этап D).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_file(tag: &str, source: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lupus_err_{}_{}_{}", std::process::id(), tag, n));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("main.lupus");
    std::fs::write(&path, source).expect("write");
    path
}

struct Output {
    stdout: String,
    stderr: String,
    code: i32,
}

fn run_args(args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_lupus"))
        .args(args)
        .output()
        .expect("run lupus");
    Output {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        code: output.status.code().unwrap_or(-1),
    }
}

fn run_file(path: &Path, extra: &[&str]) -> Output {
    let mut args: Vec<&str> = extra.to_vec();
    let path = path.to_str().unwrap();
    args.push(path);
    run_args(&args)
}

fn field(stderr: &str, name: &str) -> Option<String> {
    let marker = format!("\"{name}\":\"");
    let start = stderr.find(&marker)? + marker.len();
    let rest = &stderr[start..];
    Some(rest[..rest.find('"')?].to_string())
}

#[test]
fn check_clean_file_with_public_annotations() {
    let path = temp_file(
        "clean",
        "(import util)\n\
         (define-public (sum (a int) (b int)) -> int (+ a b))\n\
         (print (sum (util/double 2) 3))",
    );
    let result = run_file(&path, &["check"]);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn check_reports_lint_code_as_json() {
    let path = temp_file("unused", "(define y 10)");
    let result = run_file(&path, &["check"]);
    assert_eq!(result.code, 1);
    assert_eq!(
        field(&result.stderr, "code").as_deref(),
        Some("unused-variable")
    );
    assert!(result.stderr.trim().starts_with('{'));
    assert!(result.stderr.trim().ends_with('}'));
}

#[test]
fn check_no_lint_skips_lint_phase() {
    let path = temp_file("nolint", "(define y 10)");
    let result = run_file(&path, &["check", "--no-lint"]);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert!(result.stderr.is_empty());
}

#[test]
fn ast_output_is_valid_deterministic_json() {
    let path = temp_file(
        "ast",
        "(import senko :as math)\n\
         (defstruct (Node a) (value a) (left (Option (Node a))))\n\
         (define-public (f (x int)) -> int (Node x none none))\n\
         (print (math/floor 2.5))\n\
         (test \"t\" (assert (= 1 1)))",
    );
    let first = run_file(&path, &["ast"]);
    let second = run_file(&path, &["ast"]);
    assert_eq!(first.code, 0, "stderr: {}", first.stderr);
    assert!(first.stderr.is_empty());
    assert_eq!(
        first.stdout, second.stdout,
        "AST должен быть детерминирован"
    );
    let trimmed = first.stdout.trim();
    assert!(trimmed.starts_with('{') && trimmed.ends_with('}'));
    for kind in [
        "Program",
        "Import",
        "DefStruct",
        "Define",
        "Lambda",
        "Param",
        "Test",
        "Assert",
        "Call",
        "QualifiedName",
        "Identifier",
    ] {
        assert!(
            trimmed.contains(&format!("\"kind\":\"{kind}\"")),
            "нет {kind}"
        );
    }
    assert!(trimmed.contains("\"span\":null"));
}

#[test]
fn pretty_renders_human_readable_error() {
    let path = temp_file("pretty", "(/ 1 0)");
    let result = run_file(&path, &["--pretty"]);
    assert_eq!(result.code, 1);
    assert!(result.stderr.contains("error[runtime] divide-by-zero"));
    assert!(!result.stderr.trim().starts_with('{'));
}

#[test]
fn json_flag_suppresses_pretty() {
    let path = temp_file("jsonpretty", "(/ 1 0)");
    let result = run_file(&path, &["--pretty", "--json"]);
    assert_eq!(result.code, 1);
    assert!(result.stderr.trim().starts_with('{'));
    assert_eq!(
        field(&result.stderr, "code").as_deref(),
        Some("divide-by-zero")
    );
}

#[test]
fn runtime_error_has_null_location_and_clean_stdout() {
    let path = temp_file("runtime", "(print \"before\")\n(/ 1 0)");
    let result = run_file(&path, &[]);
    assert_eq!(result.code, 1);
    assert_eq!(result.stdout, "before\n");
    assert_eq!(field(&result.stderr, "phase").as_deref(), Some("runtime"));
    assert!(result.stderr.contains("\"location\":null"));
}

#[test]
fn lex_error_has_file_line_and_col() {
    let path = temp_file("lex", "(print @)");
    let result = run_file(&path, &[]);
    assert_eq!(result.code, 1);
    assert_eq!(field(&result.stderr, "phase").as_deref(), Some("lex"));
    assert_eq!(
        field(&result.stderr, "code").as_deref(),
        Some("unknown-token")
    );
    assert!(result.stderr.contains("\"line\":1"), "{}", result.stderr);
    assert!(result.stderr.contains("\"col\":8"), "{}", result.stderr);
    assert!(result.stderr.contains("main.lupus"), "{}", result.stderr);
}

#[test]
fn number_letter_tail_is_unknown_token_with_position() {
    let path = temp_file("numtail", "(print 1a)");
    let result = run_file(&path, &[]);
    assert_eq!(result.code, 1);
    assert!(result.stdout.is_empty(), "stdout: {}", result.stdout);
    assert_eq!(field(&result.stderr, "phase").as_deref(), Some("lex"));
    assert_eq!(
        field(&result.stderr, "code").as_deref(),
        Some("unknown-token")
    );
    assert!(result.stderr.contains("\"line\":1"), "{}", result.stderr);
    // 'a' — 9-й символ: позиция сразу после числового токена `1`.
    assert!(result.stderr.contains("\"col\":9"), "{}", result.stderr);
}

#[test]
fn lex_error_col_counts_characters_not_bytes() {
    // Символ '@' — 18-я позиция в символах, но 24-й байт из-за кириллицы.
    let path = temp_file("col", "(print \"привет\") @");
    let result = run_file(&path, &[]);
    assert_eq!(result.code, 1);
    assert_eq!(
        field(&result.stderr, "code").as_deref(),
        Some("unknown-token")
    );
    assert!(
        result.stderr.contains("\"col\":18"),
        "col должен считаться в символах: {}",
        result.stderr
    );
    assert!(
        !result.stderr.contains("\"col\":24"),
        "col не должен считаться в байтах: {}",
        result.stderr
    );
}
