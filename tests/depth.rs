//! Лимиты глубины eval/parse V0.5-rc2.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
static HEAVY: Mutex<()> = Mutex::new(());

struct Output {
    stdout: String,
    stderr: String,
    code: i32,
}

fn run_source(source: &str) -> Output {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path: PathBuf =
        std::env::temp_dir().join(format!("lupus_depth_{}_{}.lupus", std::process::id(), n));
    std::fs::write(&path, source).expect("не удалось записать тестовый файл");
    let output = Command::new(env!("CARGO_BIN_EXE_lupus"))
        .arg(&path)
        .output()
        .expect("не удалось запустить интерпретатор");
    let _ = std::fs::remove_file(&path);
    Output {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        code: output.status.code().unwrap_or(-1),
    }
}

fn assert_error_code(output: &Output, code: &str) {
    assert_eq!(output.code, 1, "stderr: {}", output.stderr);
    assert!(output.stdout.is_empty(), "stdout: {}", output.stdout);
    let stderr = output.stderr.trim();
    assert!(
        stderr.starts_with('{') && stderr.ends_with('}'),
        "{}",
        stderr
    );
    assert!(
        stderr.contains(&format!("\"code\":\"{code}\"")),
        "{}",
        stderr
    );
}

#[test]
fn v05_depth_deep_recursion() {
    let _guard = HEAVY.lock().unwrap();
    let output = run_source("(define (f n) (f (+ n 1)))\n(f 0)\n");
    assert_error_code(&output, "stack-depth-exceeded");
    assert!(output.stderr.contains("\"phase\":\"runtime\""));
    assert!(output.stderr.contains("\"location\":null"));
}

#[test]
fn v05_depth_deep_nesting() {
    let _guard = HEAVY.lock().unwrap();
    let source = format!("{}\n", "(".repeat(1001));
    let output = run_source(&source);
    assert_error_code(&output, "nesting-too-deep");
    assert!(output.stderr.contains("\"phase\":\"parse\""));
    assert!(output.stderr.contains("\"span\""));
}

#[test]
fn v05_depth_recursion_5000_ok() {
    let _guard = HEAVY.lock().unwrap();
    let output = run_source("(define (f n) (if (= n 0) 0 (f (- n 1))))\n(print (f 5000))\n");
    assert_eq!(output.code, 0, "stderr: {}", output.stderr);
    assert_eq!(output.stdout, "0\n");
}

#[test]
fn v05_depth_mutual_recursion() {
    let _guard = HEAVY.lock().unwrap();
    let output = run_source("(define (f n) (g (+ n 1)))\n(define (g n) (f (+ n 1)))\n(f 0)\n");
    assert_error_code(&output, "stack-depth-exceeded");
}
