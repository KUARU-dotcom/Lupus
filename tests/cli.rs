//! Интеграционные тесты CLI V0.5 (этап E).

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lupus_cli_{}_{}_{}", std::process::id(), tag, n));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn temp_file(source: &str) -> PathBuf {
    let dir = temp_dir("file");
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

/// Проверяет, что ошибка использования даёт exit 2, usage в stderr и пустой
/// stdout без JSON-диагностики.
fn assert_usage_error(args: &[&str]) {
    let result = run(args);
    assert_eq!(result.code, 2, "args={args:?}, stderr: {}", result.stderr);
    assert_eq!(result.stdout, "", "args={args:?}");
    assert!(!result.stderr.trim().is_empty(), "args={args:?}");
    assert!(
        result.stderr.contains("Использование:"),
        "args={args:?}, stderr: {}",
        result.stderr
    );
    assert!(
        !result.stderr.trim_start().starts_with('{'),
        "usage-ошибка не должна быть JSON, args={args:?}: {}",
        result.stderr
    );
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
    assert_eq!(version.stdout.trim(), "Lupus 0.5.1");

    let path = temp_file("(print 1))");
    let path = path.to_str().unwrap();
    let strict = run(&[path]);
    assert_eq!(strict.code, 1);

    let lenient = run(&["--lenient-parens", path]);
    assert_eq!(lenient.code, 0, "stderr: {}", lenient.stderr);
    assert_eq!(lenient.stdout, "1\n");
}

#[test]
fn cli_help_prints_usage() {
    let result = run(&["--help"]);
    assert_eq!(result.code, 0);
    assert!(result.stdout.contains("Использование:"));
    assert!(result.stderr.is_empty());
}

#[test]
fn cli_unknown_flag_exits_2_with_plain_usage() {
    let path = temp_file("(print 1)");
    let path = path.to_str().unwrap();

    for args in [vec!["run", "--нет-флага", path], vec!["--нет-флага"]] {
        let result = run(&args);
        assert_eq!(result.code, 2, "stderr: {}", result.stderr);
        assert!(
            result.stderr.contains("Неизвестный флаг: --нет-флага"),
            "stderr: {}",
            result.stderr
        );
        assert!(
            result.stderr.contains("Использование:"),
            "stderr: {}",
            result.stderr
        );
        assert!(
            !result.stderr.trim_start().starts_with('{'),
            "ошибка CLI не должна быть JSON: {}",
            result.stderr
        );
    }
}

#[test]
fn cli_usage_run_without_file_exits_2() {
    assert_usage_error(&["run"]);
}

#[test]
fn cli_usage_check_without_file_exits_2() {
    assert_usage_error(&["check"]);
}

#[test]
fn cli_usage_eval_without_expression_exits_2() {
    assert_usage_error(&["eval"]);
}

#[test]
fn cli_usage_flag_without_command_exits_2() {
    assert_usage_error(&["--pretty"]);
}

#[test]
fn cli_usage_run_extra_positional_exits_2() {
    assert_usage_error(&["run", "a", "b"]);
}

#[test]
fn cli_usage_no_args_exits_2() {
    assert_usage_error(&[]);
}

#[test]
fn cli_usage_ast_without_file_exits_2() {
    assert_usage_error(&["ast"]);
}

#[test]
fn cli_usage_test_without_path_exits_2() {
    assert_usage_error(&["test"]);
}

#[test]
fn cli_test_file_without_tests_reports_no_tests() {
    let path = temp_file("(print 1)");
    let result = run(&["test", path.to_str().unwrap()]);
    assert_eq!(result.code, 1, "stdout: {}", result.stdout);
    assert!(result.stdout.contains("\"type\":\"test-summary\""));
    assert!(result.stdout.contains("\"total\":0"));
    assert!(
        result.stderr.contains("\"code\":\"no-tests-found\""),
        "stderr: {}",
        result.stderr
    );
    assert!(result.stderr.contains("\"phase\":\"runtime\""));
    assert!(result.stderr.contains("не найдено ни одного теста"));
}

#[test]
fn cli_test_empty_directory_reports_no_tests() {
    let dir = temp_dir("empty");
    let result = run(&["test", dir.to_str().unwrap()]);
    assert_eq!(result.code, 1, "stdout: {}", result.stdout);
    assert!(result.stdout.is_empty());
    assert!(
        result.stderr.contains("\"code\":\"no-tests-found\""),
        "stderr: {}",
        result.stderr
    );
    assert!(result.stderr.contains(&dir.display().to_string()));
}

#[cfg(unix)]
#[test]
fn cli_test_does_not_follow_directory_symlinks() {
    use std::os::unix::fs::symlink;

    let dir = temp_dir("symlink");
    std::fs::write(
        dir.join("suite.lupus"),
        "(import (w test))\n(test \"once\" (test/assert-true true))\n",
    )
    .expect("write suite");
    symlink(".", dir.join("self")).expect("dir symlink");
    symlink("suite.lupus", dir.join("alias.lupus")).expect("file symlink");

    let result = run(&["test", dir.to_str().unwrap()]);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert_eq!(
        result.stdout.matches("\"type\":\"test-start\"").count(),
        1,
        "stdout: {}",
        result.stdout
    );
    assert!(result.stdout.contains("\"total\":1"));
}
