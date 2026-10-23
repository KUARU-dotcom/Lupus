//! Дифференциальный тест против эталонного Lupus v0.4.0 (этап F2).
//!
//! Если `oracles/v0.4.0/lupus` отсутствует, тест явно помечается как SKIP
//! и не проваливается. Скачать бинарник: `scripts/fetch-oracle.sh`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn oracle_path() -> PathBuf {
    manifest_dir().join("oracles/v0.4.0/lupus")
}

fn collect_lupus(dir: &Path, predicate: &dyn Fn(&Path) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("lupus") && predicate(&path) {
            out.push(path);
        }
    }
}

fn is_v05_fixture(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name.starts_with("v05_"))
}

fn corpus() -> Vec<PathBuf> {
    let root = manifest_dir();
    let mut files = Vec::new();
    // V0.5-фикстуры и примеры используют import/Map/дженерики, которых нет
    // в v0.4.0; дифференциальный корпус — только совместимые файлы.
    collect_lupus(
        &root.join("tests/golden"),
        &|path| !is_v05_fixture(path),
        &mut files,
    );
    collect_lupus(
        &root.join("examples"),
        &|path| !is_v05_fixture(path),
        &mut files,
    );
    files.sort();
    files
}

#[test]
fn differential_against_v04_oracle() {
    let oracle = oracle_path();
    if !oracle.is_file() {
        eprintln!(
            "SKIP: эталонный бинарник {} отсутствует; запустите scripts/fetch-oracle.sh",
            oracle.display()
        );
        return;
    }

    let current = PathBuf::from(env!("CARGO_BIN_EXE_lupus"));
    let files = corpus();
    assert!(!files.is_empty(), "пустой корпус дифференциального теста");

    let mut failures = Vec::new();
    for file in &files {
        let expected = Command::new(&oracle)
            .arg(file)
            .output()
            .unwrap_or_else(|error| {
                panic!(
                    "не удалось запустить oracle для {}: {error}",
                    file.display()
                )
            });
        let actual = Command::new(&current)
            .arg("--no-lint")
            .arg(file)
            .output()
            .unwrap_or_else(|error| {
                panic!("не удалось запустить lupus для {}: {error}", file.display())
            });

        let expected_code = expected.status.code();
        let actual_code = actual.status.code();
        if expected_code != actual_code || expected.stdout != actual.stdout {
            failures.push(format!(
                "{}\n  oracle: code={:?} stdout={:?}\n  lupus:  code={:?} stdout={:?}",
                file.display(),
                expected_code,
                String::from_utf8_lossy(&expected.stdout),
                actual_code,
                String::from_utf8_lossy(&actual.stdout),
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "дифференциальные расхождения ({} из {}):\n{}",
        failures.len(),
        files.len(),
        failures.join("\n\n")
    );
}
