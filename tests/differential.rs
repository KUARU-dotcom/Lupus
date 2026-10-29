//! Differential-тесты: эталон V0.5.2 против текущей версии (V0.7 этап 4).
//!
//! Базовая версия берётся из `dist/lupus-v0.5.2-linux-x86_64.tar.gz` и
//! распаковывается в `target/differential/v0.5.2/lupus`. Путь можно
//! переопределить через `LUPUS_DIFF_BASELINE`. Таймаут запуска одной
//! программы — `LUPUS_DIFF_TIMEOUT_MS` (по умолчанию 5000 мс).
//!
//! Если базовый бинарник недоступен, тесты помечаются как SKIP. При
//! `LUPUS_DIFF_REQUIRED=1` отсутствие бинарника — ошибка гейта.

use lupus::differential::{
    compare_binaries, effective_timeout, BreakingChange, DiffOutcome, DiffReport,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn current_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lupus"))
}

/// Возвращает путь к V0.5.2 или `None`, если бинарник недоступен.
fn baseline_binary() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("LUPUS_DIFF_BASELINE") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    let direct = manifest_dir().join("oracles/v0.5.2/lupus");
    if direct.is_file() {
        return Some(direct);
    }

    let archive = manifest_dir().join("dist/lupus-v0.5.2-linux-x86_64.tar.gz");
    if !archive.is_file() {
        return None;
    }

    let dest_dir = manifest_dir().join("target/differential/v0.5.2");
    let dest = dest_dir.join("lupus");
    if dest.is_file() {
        return Some(dest);
    }

    std::fs::create_dir_all(&dest_dir).ok()?;
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&dest_dir)
        .arg("lupus")
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&dest).ok()?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&dest, permissions).ok()?;
    }

    dest.is_file().then_some(dest)
}

fn require_baseline() -> Option<PathBuf> {
    let baseline = baseline_binary();
    if baseline.is_none() && std::env::var("LUPUS_DIFF_REQUIRED").as_deref() == Ok("1") {
        panic!(
            "LUPUS_DIFF_REQUIRED=1, но базовый бинарник V0.5.2 не найден; \
             ожидался dist/lupus-v0.5.2-linux-x86_64.tar.gz или LUPUS_DIFF_BASELINE"
        );
    }
    if baseline.is_none() {
        eprintln!("SKIP: базовый бинарник V0.5.2 не найден");
    }
    baseline
}

fn temp_program(source: &str) -> PathBuf {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lupus_diff_{}_{}", std::process::id(), index));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("program.lupus");
    std::fs::write(&path, source).expect("write differential program");
    path
}

fn compare_source(baseline: &Path, source: &str) -> DiffReport {
    let path = temp_program(source);
    compare_binaries(&path, baseline, &current_binary(), effective_timeout())
        .unwrap_or_else(|error| panic!("не удалось сравнить программу:\n{source}\n{error}"))
}

fn collect_lupus(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("lupus") {
            out.push(path);
        }
    }
}

/// Файлы, использующие появившиеся в V0.6 модули/возможности, не входят в
/// общий для V0.5.2 и V0.6 корпус.
fn skip_v06_only(source: &str) -> Option<&'static str> {
    let markers = [
        ("(import (amiya", "V0.6 async"),
        ("(import amiya", "V0.6 async"),
        ("amiya/", "V0.6 async"),
        ("(import (texas", "V0.6 network"),
        ("(import texas", "V0.6 network"),
        ("texas/", "V0.6 network"),
        ("(import (lappland", "V0.6 lappland"),
        ("(import lappland", "V0.6 lappland"),
        ("lappland/", "V0.6 lappland"),
        ("(gc-collect", "V0.6 GC"),
        ("(set-fuel!", "V0.6 fuel"),
    ];
    markers
        .iter()
        .find(|(marker, _)| source.contains(marker))
        .map(|(_, reason)| *reason)
}

#[test]
fn test_diff_identical() {
    let Some(baseline) = require_baseline() else {
        return;
    };

    let programs = [
        "(print (+ 1 2))\n",
        "(print (string-append \"a\" \"b\"))\n",
        "(define xs (list 1 2 3))\n(print (length xs))\n(print (nth xs 1))\n",
        "(define (f x) (* x 2))\n(print (f 21))\n",
        "(print (match (some 5) ((some v) v) (none 0)))\n",
        "(print (let ((a 2) (b 3)) (+ a b)))\n",
    ];

    for source in programs {
        let report = compare_source(&baseline, source);
        assert_eq!(
            report.outcome,
            DiffOutcome::Identical,
            "программа должна вести себя одинаково:\n{source}\n{}",
            report.summary()
        );
    }
}

#[test]
fn test_diff_known_change() {
    let Some(baseline) = require_baseline() else {
        return;
    };

    let cases = [
        ("(print (and 1 2))\n", BreakingChange::AndOrValue),
        ("(print (or false 0))\n", BreakingChange::AndOrValue),
        ("(print (not 0))\n", BreakingChange::Truthiness),
        ("(print (head 5))\n", BreakingChange::WrongTypeStrictness),
        ("(print (tail 5))\n", BreakingChange::WrongTypeStrictness),
        ("(print ())\n", BreakingChange::EmptyListLiteral),
        ("(head ())\n", BreakingChange::HeadTailOnEmpty),
        (
            "(defstruct point (x int) (y int))\n(point \"a\" 1)\n",
            BreakingChange::StructFieldTypeCheck,
        ),
    ];

    for (source, expected) in cases {
        let report = compare_source(&baseline, source);
        assert_eq!(
            report.outcome,
            DiffOutcome::Expected(expected),
            "ожидалось известное breaking change {expected:?}:\n{source}\n{}",
            report.summary()
        );
        assert!(!report.is_unexpected());
    }
}

#[test]
fn test_diff_no_regression() {
    let Some(baseline) = require_baseline() else {
        return;
    };
    let current = current_binary();

    let mut files = Vec::new();
    collect_lupus(&manifest_dir().join("tests/golden"), &mut files);
    collect_lupus(&manifest_dir().join("tests/negative"), &mut files);
    files.sort();
    assert!(!files.is_empty(), "пустой корпус golden/negative");

    let mut compared = 0usize;
    let mut identical = 0usize;
    let mut expected = 0usize;
    let mut skipped = 0usize;
    let mut expected_by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut unexpected = Vec::new();

    for file in &files {
        let source = std::fs::read_to_string(file)
            .unwrap_or_else(|error| panic!("не удалось прочитать {}: {error}", file.display()));

        if skip_v06_only(&source).is_some() {
            skipped += 1;
            continue;
        }

        let report = compare_binaries(file, &baseline, &current, effective_timeout())
            .unwrap_or_else(|error| panic!("не удалось сравнить {}: {error}", file.display()));
        compared += 1;

        match report.outcome {
            DiffOutcome::Identical => identical += 1,
            DiffOutcome::Expected(change) => {
                expected += 1;
                *expected_by_kind.entry(change.as_str()).or_default() += 1;
            }
            DiffOutcome::Unexpected => {
                unexpected.push(format!("{}\n  {}", file.display(), report.summary()));
            }
        }
    }

    println!(
        "DIFF_REPORT total={} compared={} identical={} expected={} unexpected={} skipped={}",
        files.len(),
        compared,
        identical,
        expected,
        unexpected.len(),
        skipped
    );
    for (kind, count) in &expected_by_kind {
        println!("DIFF_EXPECTED {kind}={count}");
    }

    assert!(
        unexpected.is_empty(),
        "неожиданные расхождения ({} из {}):\n{}",
        unexpected.len(),
        compared,
        unexpected.join("\n\n")
    );
}
