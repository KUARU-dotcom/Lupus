//! Интеграционные тесты модулей V0.5 (этап A).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Каталоги модулей создаются под `target/`, а в `LUPUS_PATH` передаётся путь
/// относительно корня пакета: разделитель `LUPUS_PATH` — `:`, а абсолютные
/// пути Windows содержат `:` (буква диска), что ломало бы разбор. Относительный
/// путь не содержит `:` и работает одинаково на Unix и Windows.
fn temp_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("lupus_mod_{}_{}_{}", std::process::id(), tag, n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn write_program(dir: &Path, name: &str, source: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, source).expect("write program");
    path
}

/// Запускает программу, возвращая (stdout, stderr, код).
fn run_with_path(path: &Path, lupus_path: Option<&Path>) -> (String, String, i32) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lupus"));
    command.arg(path);
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
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

fn error_code(stderr: &str) -> Option<String> {
    let marker = "\"code\":\"";
    let start = stderr.find(marker)? + marker.len();
    let rest = &stderr[start..];
    Some(rest[..rest.find('"')?].to_string())
}

fn run_ok(source: &str) -> String {
    let dir = temp_dir("ok");
    let file = write_program(&dir, "main.lupus", source);
    let (stdout, stderr, code) = run_with_path(&file, None);
    assert_eq!(code, 0, "stderr: {}", stderr);
    stdout
}

#[test]
fn modules_import_native_package() {
    let out = run_ok("(import senko)\n(print (senko/sqrt 25.0))");
    assert_eq!(out, "5.0\n");
}

#[test]
fn modules_import_as_alias() {
    let out = run_ok("(import senko :as math)\n(print (math/floor 3.7))");
    assert_eq!(out, "3.0\n");
}

#[test]
fn modules_qualified_constant_and_call() {
    let out = run_ok("(import senko)\n(print (> senko/pi 3.14))\n(print (senko/pow 2.0 10.0))");
    assert_eq!(out, "true\n1024.0\n");
}

#[test]
fn modules_file_module_from_stdlib() {
    let out = run_ok("(import util)\n(print (util/double 21))\n(print (util/greet \"Lupus\"))");
    assert_eq!(out, "42\nПривет, Lupus\n");
}

#[test]
fn modules_file_module_from_lupus_path() {
    let module_dir = temp_dir("path");
    write_program(
        &module_dir,
        "shapes.lupus",
        "(define-public (square (x int)) -> int (* x x))\n(define secret 1)",
    );
    let program_dir = temp_dir("main");
    let file = write_program(
        &program_dir,
        "main.lupus",
        "(import shapes)\n(print (shapes/square 9))",
    );
    let (stdout, stderr, code) = run_with_path(&file, Some(&module_dir));
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert_eq!(stdout, "81\n");
}

#[test]
fn modules_repeated_import_is_cached() {
    let module_dir = temp_dir("cache");
    write_program(
        &module_dir,
        "side.lupus",
        "(print \"loaded\")\n(define-public (value) -> int 7)",
    );
    let program_dir = temp_dir("cachemain");
    let file = write_program(
        &program_dir,
        "main.lupus",
        "(import side)\n(import side)\n(print (side/value))",
    );
    let (stdout, stderr, code) = run_with_path(&file, Some(&module_dir));
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert_eq!(stdout, "loaded\n7\n");
}

#[test]
fn modules_import_all_binds_names_without_prefix() {
    let module_dir = temp_dir("all");
    write_program(
        &module_dir,
        "nums.lupus",
        "(define-public (triple (x int)) -> int (* x 3))",
    );
    let program_dir = temp_dir("allmain");
    let file = write_program(
        &program_dir,
        "main.lupus",
        "(import nums :all)\n(print (triple 5))",
    );
    let (stdout, stderr, code) = run_with_path(&file, Some(&module_dir));
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert_eq!(stdout, "15\n");
}

#[test]
fn modules_unknown_module() {
    let dir = temp_dir("unknown");
    let file = write_program(&dir, "main.lupus", "(import definitely_not_a_module)");
    let (_, stderr, code) = run_with_path(&file, None);
    assert_ne!(code, 0);
    assert_eq!(error_code(&stderr).as_deref(), Some("unknown-module"));
}

#[test]
fn modules_unknown_module_symbol() {
    let out = run_ok("");
    assert_eq!(out, "");
    let dir = temp_dir("unknownsym");
    let file = write_program(&dir, "main.lupus", "(import util)\n(util/hidden 1)");
    let (_, stderr, code) = run_with_path(&file, None);
    assert_ne!(code, 0);
    assert_eq!(
        error_code(&stderr).as_deref(),
        Some("unknown-module-symbol")
    );
}

#[test]
fn modules_qualified_without_import_is_unknown_identifier() {
    let dir = temp_dir("noimport");
    let file = write_program(&dir, "main.lupus", "(senko/sqrt 25.0)");
    let (_, stderr, code) = run_with_path(&file, None);
    assert_ne!(code, 0);
    assert_eq!(error_code(&stderr).as_deref(), Some("unknown-identifier"));
    assert!(stderr.contains("добавьте (import senko)"), "{}", stderr);
}

#[test]
fn modules_import_cycle() {
    let module_dir = temp_dir("cycle");
    write_program(
        &module_dir,
        "a.lupus",
        "(import b)\n(define-public (a) -> int 1)",
    );
    write_program(
        &module_dir,
        "b.lupus",
        "(import a)\n(define-public (b) -> int 2)",
    );
    let program_dir = temp_dir("cyclemain");
    let file = write_program(&program_dir, "main.lupus", "(import a)");
    let (_, stderr, code) = run_with_path(&file, Some(&module_dir));
    assert_ne!(code, 0);
    assert_eq!(error_code(&stderr).as_deref(), Some("import-cycle"));
}
