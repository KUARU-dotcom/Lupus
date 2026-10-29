//! Интеграционные тесты нативной stdlib V0.5 (этап B):
//! `senko`, `kaltsit`, `w` и подкоманда `lupus test`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Каталоги создаются под `target/`: в `LUPUS_PATH` передаётся путь
/// относительно корня пакета (разделитель `LUPUS_PATH` — `:`, а абсолютные
/// пути Windows содержат `:` — буква диска). Относительный путь не содержит
/// `:` и работает одинаково на Unix и Windows.
fn temp_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("lupus_std_{}_{}_{}", std::process::id(), tag, n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn write_program(dir: &Path, name: &str, source: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, source).expect("write program");
    path
}

struct Output {
    stdout: String,
    stderr: String,
    code: i32,
}

fn run(args: &[&str], lupus_path: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lupus"));
    command.args(args);
    if let Some(dir) = lupus_path {
        // LUPUS_PATH использует ':' как разделитель, а абсолютные пути Windows
        // содержат ':' (буква диска), поэтому в переменную передаётся путь
        // относительно корня пакета — это текущий каталог процесса-наследника.
        let value = dir
            .strip_prefix(std::env::current_dir().expect("cwd"))
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| dir.to_path_buf());
        command.env("LUPUS_PATH", value);
    }
    let output = command.output().expect("run lupus");
    Output {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        code: output.status.code().unwrap_or(-1),
    }
}

fn run_ok(source: &str) -> String {
    let dir = temp_dir("ok");
    let file = write_program(&dir, "main.lupus", source);
    let result = run(&[file.to_str().unwrap()], None);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    result.stdout
}

fn error_code(stderr: &str) -> Option<String> {
    let marker = "\"code\":\"";
    let start = stderr.find(marker)? + marker.len();
    let rest = &stderr[start..];
    Some(rest[..rest.find('"')?].to_string())
}

/// Путь для интерполяции в Lupus-строковый литерал: с прямыми слешами.
/// Обратные слеши Windows-путей ловились бы лексером как escape-последовательности
/// (`\a` в `D:\a\...` даёт `invalid-escape`). На Unix замена тождественна;
/// файловые API Windows принимают прямые слеши.
fn to_source_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn senko_constants_and_math() {
    let out = run_ok(
        "(import senko)\n\
         (print (senko/sqrt 2.0))\n\
         (print (senko/sin 0.0))\n\
         (print (senko/cos 0.0))\n\
         (print (senko/log senko/e))\n\
         (print (senko/pow 2.0 10.0))\n\
         (print (senko/round 2.5))\n\
         (print (senko/min 3.0 1.5))\n\
         (print (senko/max 3.0 1.5))",
    );
    let values: Vec<f64> = out.lines().map(|line| line.parse().unwrap()).collect();
    assert_close(values[0], std::f64::consts::SQRT_2);
    assert_close(values[1], 0.0);
    assert_close(values[2], 1.0);
    assert_close(values[3], 1.0);
    assert_close(values[4], 1024.0);
    assert_close(values[5], 3.0);
    assert_close(values[6], 1.5);
    assert_close(values[7], 3.0);
}

#[test]
fn senko_floor_ceil_abs_log10() {
    let out = run_ok(
        "(import senko)\n\
         (print (senko/floor 3.7))\n\
         (print (senko/ceil 3.1))\n\
         (print (senko/abs (- 0.0 4.5)))\n\
         (print (senko/log10 1000.0))\n\
         (print (senko/tan 0.0))",
    );
    let values: Vec<f64> = out.lines().map(|line| line.parse().unwrap()).collect();
    assert_close(values[0], 3.0);
    assert_close(values[1], 4.0);
    assert_close(values[2], 4.5);
    assert_close(values[3], 3.0);
    assert_close(values[4], 0.0);
}

#[test]
fn kaltsit_file_round_trip() {
    let dir = temp_dir("kaltsit");
    let file = dir.join("данные.txt");
    let display = to_source_path(&file);
    let source = format!(
        "(import (kaltsit file))\n\
         (print (file/exists? \"{path}\"))\n\
         (match (file/write-file \"{path}\" \"привет\")\n\
            ((success _) (print \"written\"))\n\
            ((failure m) (print m)))\n\
         (match (file/read-file \"{path}\")\n\
            ((success s) (print s))\n\
            ((failure m) (print m)))\n\
         (match (file/size \"{path}\") ((success n) (print n)) ((failure m) (print m)))\n\
         (match (file/append-file \"{path}\" \"!\")\n\
            ((success _) (print \"appended\"))\n\
            ((failure m) (print m)))",
        path = display
    );
    let out = run_ok(&source);
    assert_eq!(out, "false\nwritten\nпривет\n12\nappended\n");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "привет!");
}

#[test]
fn kaltsit_directories_and_delete() {
    let dir = temp_dir("kaltsit-dir");
    let sub = dir.join("sub");
    let file = sub.join("a.txt");
    let sub_path = to_source_path(&sub);
    let file_path = to_source_path(&file);
    let source = format!(
        "(import (kaltsit file))\n\
         (match (file/mkdir \"{sub}\")\n\
            ((success _) (print (file/is-dir? \"{sub}\")))\n\
            ((failure m) (print m)))\n\
         (match (file/write-file \"{file}\" \"x\")\n\
            ((success _) (print \"ok\"))\n\
            ((failure m) (print m)))\n\
         (match (file/list-dir \"{sub}\")\n\
            ((success items) (print items))\n\
            ((failure m) (print m)))\n\
         (match (file/delete \"{file}\")\n\
            ((success _) (print (file/exists? \"{file}\")))\n\
            ((failure m) (print m)))",
        sub = sub_path,
        file = file_path
    );
    let out = run_ok(&source);
    assert_eq!(out, "true\nok\n(\"a.txt\")\nfalse\n");
}

#[test]
fn kaltsit_missing_file_returns_failure() {
    let dir = temp_dir("kaltsit-missing");
    let missing = dir.join("nope.txt");
    let source = format!(
        "(import (kaltsit file))\n\
         (print (match (file/read-file \"{path}\")\n\
            ((failure m) (string-append \"io:\" (string-append m \"\")))\n\
            ((success _) \"unexpected\")))",
        path = to_source_path(&missing)
    );
    let out = run_ok(&source);
    assert!(out.starts_with("io:"), "{out}");
    assert!(
        out.contains("No such file") || out.contains("os error"),
        "{out}"
    );
}

#[test]
fn test_command_emits_jsonl_and_exit_code() {
    let dir = temp_dir("test-jsonl");
    let file = write_program(
        &dir,
        "suite.lupus",
        "(import (w test))\n\
         (test \"ok\" (test/assert-eq 4 (+ 2 2)))\n\
         (test \"bad\" (test/assert-eq 5 (+ 2 2)))",
    );
    let result = run(&["test", file.to_str().unwrap()], None);
    assert_eq!(result.code, 1, "stderr: {}", result.stderr);
    let lines: Vec<&str> = result.stdout.lines().collect();
    assert_eq!(lines.len(), 5, "stdout: {}", result.stdout);
    assert!(lines[0].starts_with("{\"type\":\"test-start\""));
    assert!(lines[1].starts_with("{\"type\":\"test-pass\""));
    assert!(lines[2].starts_with("{\"type\":\"test-start\""));
    assert!(lines[3].starts_with("{\"type\":\"test-fail\""));
    assert!(lines[3].contains("\"code\":\"assert-failed\""));
    assert!(lines[4].starts_with("{\"type\":\"test-summary\""));
    assert!(lines[4].contains("\"total\":2"));
    assert!(lines[4].contains("\"passed\":1"));
    assert!(lines[4].contains("\"failed\":1"));
}

#[test]
fn test_command_isolates_tests() {
    let dir = temp_dir("test-isolation");
    let file = write_program(
        &dir,
        "suite.lupus",
        "(import (w test))\n\
         (test \"one\" (define x 1) (test/assert-eq 1 x))\n\
         (test \"two\" (test/assert-eq 1 x))",
    );
    let result = run(&["test", file.to_str().unwrap()], None);
    assert_eq!(result.code, 1);
    assert!(result
        .stdout
        .contains("\"type\":\"test-pass\",\"name\":\"one\""));
    assert!(result
        .stdout
        .contains("\"type\":\"test-fail\",\"name\":\"two\""));
    assert!(result.stdout.contains("\"code\":\"unknown-identifier\""));
}

#[test]
fn test_command_suppresses_print_without_verbose() {
    let dir = temp_dir("test-print");
    let file = write_program(
        &dir,
        "suite.lupus",
        "(import (w test))\n\
         (test \"noise\" (print \"NOISE\") (test/assert-true true))",
    );
    let quiet = run(&["test", file.to_str().unwrap()], None);
    assert!(!quiet.stdout.contains("NOISE"), "{}", quiet.stdout);
    assert!(quiet.stdout.starts_with("{\"type\":\"test-start\""));

    let verbose = run(&["test", "--verbose", file.to_str().unwrap()], None);
    assert!(verbose.stdout.contains("NOISE"), "{}", verbose.stdout);
}

#[test]
fn test_run_not_executed_by_normal_run() {
    let out = run_ok(
        "(import (w test))\n\
         (test \"would-print\" (print \"TEST-OUTPUT\") (test/assert-true true))\n\
         (print \"program\")",
    );
    assert_eq!(out, "program\n");
}

#[test]
fn test_w_run_all_returns_exit_code() {
    let dir = temp_dir("run-all");
    let file = write_program(
        &dir,
        "suite.lupus",
        "(import (w test))\n\
         (test \"passing\" (test/assert-true true))\n\
         (test \"aggregate\" (test/assert-eq 0 (test/run-all)))",
    );
    let result = run(&["test", file.to_str().unwrap()], None);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert!(result
        .stdout
        .contains("\"total\":2,\"passed\":2,\"failed\":0"));
}

#[test]
fn w_assert_eq_failure_has_diff_and_context_token() {
    let dir = temp_dir("assert-diff");
    let file = write_program(&dir, "main.lupus", "(import w)\n(w/assert-eq 1 2)");
    let result = run(&[file.to_str().unwrap()], None);
    assert_ne!(result.code, 0);
    assert_eq!(error_code(&result.stderr).as_deref(), Some("assert-failed"));
    assert!(result.stderr.contains("ожидалось 1"), "{}", result.stderr);
    assert!(result.stderr.contains("получено 2"), "{}", result.stderr);
    assert!(
        result.stderr.contains("\"token\":\"2\""),
        "{}",
        result.stderr
    );
}

#[test]
fn negative_math_domain_error() {
    let dir = temp_dir("domain");
    let file = write_program(
        &dir,
        "main.lupus",
        "(import senko)\n(print (senko/sqrt (- 0.0 1.0)))",
    );
    let result = run(&[file.to_str().unwrap()], None);
    assert_ne!(result.code, 0);
    assert_eq!(
        error_code(&result.stderr).as_deref(),
        Some("math-domain-error")
    );
}

#[test]
fn negative_test_name_duplicate() {
    let dir = temp_dir("dup-test");
    let file = write_program(
        &dir,
        "suite.lupus",
        "(import (w test))\n\
         (test \"same\" (test/assert-true true))\n\
         (test \"same\" (test/assert-true true))",
    );
    let result = run(&["test", file.to_str().unwrap()], None);
    assert_ne!(result.code, 0);
    assert_eq!(
        error_code(&result.stderr).as_deref(),
        Some("test-name-duplicate")
    );
}

#[test]
fn negative_test_in_function() {
    let dir = temp_dir("test-in-fn");
    let file = write_program(
        &dir,
        "main.lupus",
        "(import (w test))\n\
         (define (f) (test \"inner\" (test/assert-true true)) 1)\n\
         (print (f))",
    );
    let result = run(&["check", file.to_str().unwrap()], None);
    assert_ne!(result.code, 0);
    assert_eq!(
        error_code(&result.stderr).as_deref(),
        Some("test-in-function")
    );
}

#[test]
fn test_command_assert_message_uses_source_syntax() {
    let dir = temp_dir("test-assert-source");
    let file = write_program(
        &dir,
        "suite.lupus",
        "(import (w test))\n(test \"bad\" (assert (= (+ 1 2) 4)))",
    );
    let result = run(&["test", file.to_str().unwrap()], None);
    assert_eq!(result.code, 1, "stderr: {}", result.stderr);
    assert!(
        result.stdout.contains("(= (+ 1 2) 4)"),
        "stdout: {}",
        result.stdout
    );
    assert!(
        !result.stdout.contains("Symbol("),
        "Rust Debug не должен попадать в JSONL: {}",
        result.stdout
    );
}
