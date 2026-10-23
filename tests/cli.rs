//! Интеграционные тесты CLI V0.5 (этап E).

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_file(source: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lupus_cli_{}_{}", std::process::id(), n));
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

fn run(args: &[&str]) -> Output {
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

#[test]
fn cli_eval_prints_value() {
    let result = run(&["eval", "(+ 2 3)"]);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert_eq!(result.stdout, "5\n");

    let result = run(&["eval", "(string-append \"a\" \"b\")"]);
    assert_eq!(result.stdout, "ab\n");
}

#[test]
fn cli_run_default_and_explicit() {
    let path = temp_file("(print (+ 1 2))");
    let path = path.to_str().unwrap();

    let implicit = run(&[path]);
    assert_eq!(implicit.code, 0, "stderr: {}", implicit.stderr);
    assert_eq!(implicit.stdout, "3\n");

    let explicit = run(&["run", path]);
    assert_eq!(explicit.code, 0, "stderr: {}", explicit.stderr);
    assert_eq!(explicit.stdout, "3\n");
}

#[test]
fn cli_check_exit_code_on_lint_error() {
    let path = temp_file("(define y 1)");
    let result = run(&["check", path.to_str().unwrap()]);
    assert_eq!(result.code, 1);
    assert!(result.stderr.contains("\"code\":\"unused-variable\""));
}

#[test]
fn cli_no_lint_skips_core_shadowing() {
    let path = temp_file("(define print 1)");
    let path = path.to_str().unwrap();

    let linted = run(&[path]);
    assert_eq!(linted.code, 1);
    assert!(linted.stderr.contains("\"code\":\"core-shadowing\""));

    let skipped = run(&["--no-lint", path]);
    assert_eq!(skipped.code, 0, "stderr: {}", skipped.stderr);
    assert!(skipped.stderr.is_empty());
}

#[test]
fn cli_ast_stdout_is_json_only() {
    let path = temp_file("(print 1)");
    let result = run(&["ast", path.to_str().unwrap()]);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert!(result.stderr.is_empty());
    let stdout = result.stdout.trim();
    assert!(stdout.starts_with('{') && stdout.ends_with('}'));
    assert!(stdout.contains("\"kind\":\"Program\""));
}

#[test]
fn cli_version_and_lenient_parens() {
    let version = run(&["--version"]);
    assert_eq!(version.code, 0);
    assert!(version.stdout.starts_with("Lupus "));

    let path = temp_file("(print 1))");
    let path = path.to_str().unwrap();
    let strict = run(&[path]);
    assert_eq!(strict.code, 1);

    let lenient = run(&["--lenient-parens", path]);
    assert_eq!(lenient.code, 0, "stderr: {}", lenient.stderr);
    assert_eq!(lenient.stdout, "1\n");
}
