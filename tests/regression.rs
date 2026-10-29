//! Регрессионные сюиты V0.5-rc2: границы, строки, модули и детерминизм.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Output {
    stdout: String,
    stderr: String,
    code: i32,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_args(args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_lupus"))
        .args(args)
        .current_dir(root())
        .output()
        .expect("не удалось запустить интерпретатор");
    Output {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        code: output.status.code().unwrap_or(-1),
    }
}

fn run_file(path: &Path) -> Output {
    run_args(&[path.to_str().unwrap()])
}

fn run_source(source: &str) -> Output {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "lupus_regression_{}_{}.lupus",
        std::process::id(),
        n
    ));
    std::fs::write(&path, source).expect("не удалось записать тестовый файл");
    let output = run_file(&path);
    let _ = std::fs::remove_file(&path);
    output
}

fn field(stderr: &str, name: &str) -> Option<String> {
    let marker = format!("\"{name}\":\"");
    let start = stderr.find(&marker)? + marker.len();
    let rest = &stderr[start..];
    Some(rest[..rest.find('"')?].to_string())
}

fn assert_negative(name: &str, code: &str) {
    let path = root().join("tests/negative").join(format!("{name}.lupus"));
    let output = run_file(&path);
    assert_eq!(output.code, 1, "[{name}] stderr: {}", output.stderr);
    assert!(
        output.stdout.is_empty(),
        "[{name}] stdout: {}",
        output.stdout
    );
    assert_eq!(
        field(&output.stderr, "code").as_deref(),
        Some(code),
        "[{name}] stderr: {}",
        output.stderr
    );
}

/// Сравнение golden-stdout с нормализацией переводов строк: на Windows-раннере
/// git выгружает .expected с CRLF (autocrlf), а stdout интерпретатора всегда
/// содержит '\n'. Нормализация идентична на Unix.
fn normalize_eol(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn assert_golden(name: &str) {
    let base = root().join("tests/golden");
    let output = run_file(&base.join(format!("{name}.lupus")));
    let expected = std::fs::read_to_string(base.join(format!("{name}.expected")))
        .unwrap_or_else(|_| panic!("нет {name}.expected"));
    assert_eq!(output.code, 0, "[{name}] stderr: {}", output.stderr);
    assert!(
        output.stderr.is_empty(),
        "[{name}] stderr: {}",
        output.stderr
    );
    assert_eq!(
        normalize_eol(&output.stdout),
        normalize_eol(&expected),
        "[{name}] stdout не совпал"
    );
}

#[test]
fn v05_add_overflow_is_integer_overflow() {
    assert_negative("v05_add_overflow", "integer-overflow");
}

#[test]
fn v05_sub_overflow_is_integer_overflow() {
    assert_negative("v05_sub_overflow", "integer-overflow");
}

#[test]
fn v05_mul_overflow_is_integer_overflow() {
    assert_negative("v05_mul_overflow", "integer-overflow");
}

#[test]
fn v05_unary_neg_overflow_is_integer_overflow() {
    assert_negative("v05_unary_neg_overflow", "integer-overflow");
}

#[test]
fn v05_pow_overflow_is_integer_overflow() {
    assert_negative("v05_pow_overflow", "integer-overflow");
}

#[test]
fn v05_i64_boundaries_ok() {
    assert_golden("v05_i64_boundaries_ok");
}

#[test]
fn v05_float_nan_inf_ok() {
    assert_golden("v05_float_nan_inf");
}

#[test]
fn v05_float_precision_loss_is_intentional() {
    assert_golden("v05_float_precision_loss");
}

#[test]
fn v05_exit_negative_is_invalid_exit_code() {
    assert_negative("v05_exit_negative", "invalid-exit-code");
}

#[test]
fn v05_exit_1000_is_invalid_exit_code() {
    assert_negative("v05_exit_1000", "invalid-exit-code");
}

#[test]
fn v05_exit_255_is_valid() {
    let output = run_source("(exit 255)\n");
    assert_eq!(output.code, 255, "stderr: {}", output.stderr);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn v05_unicode_emoji_ok() {
    assert_golden("v05_unicode_emoji");
}

#[test]
fn v05_cyrillic_strings_ok() {
    assert_golden("v05_cyrillic_strings");
}

#[test]
fn v05_escape_sequences_ok() {
    assert_golden("v05_escape_sequences");
}

#[test]
fn v05_module_cache_once_ok() {
    assert_golden("v05_module_cache_once");
}

#[test]
fn v05_import_all_collision_detected() {
    assert_negative("v05_import_all_collision", "import-collision");
}

#[test]
fn v05_determinism_is_stable() {
    let base = root().join("tests/golden");
    let path = base.join("v05_determinism.lupus");
    let expected = std::fs::read_to_string(base.join("v05_determinism.expected"))
        .expect("нет v05_determinism.expected");
    let mut first: Option<String> = None;
    for _ in 0..3 {
        let output = run_file(&path);
        assert_eq!(output.code, 0, "stderr: {}", output.stderr);
        assert_eq!(
            normalize_eol(&output.stdout),
            normalize_eol(&expected),
            "stdout не совпал с .expected"
        );
        if let Some(previous) = &first {
            assert_eq!(&output.stdout, previous, "недетерминированный stdout");
        } else {
            first = Some(output.stdout);
        }
    }
}
