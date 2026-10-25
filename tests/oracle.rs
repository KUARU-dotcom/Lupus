//! Дифференциальный тест против эталонного Lupus v0.4.0 (этап F2).
//!
//! Корпус состоит из всех golden/examples и 60 детерминированных программ
//! (seed=42, 70% exit 0 / 30% exit 1). Файлы с новыми возможностями V0.5
//! пропускаются: v0.4.0 их не понимает.
//!
//! Если `oracles/v0.4.0/lupus` отсутствует, тест помечается как SKIP и не
//! проваливается. При `LUPUS_ORACLE_REQUIRED=1` отсутствие бинарника — ошибка
//! гейта. Скачать бинарник: `scripts/fetch-oracle.sh`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

const GENERATED_COUNT: usize = 60;
const GENERATED_SEED: u64 = 42;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn oracle_path() -> PathBuf {
    manifest_dir().join("oracles/v0.4.0/lupus")
}

fn collect_lupus(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("lupus") {
            out.push(path);
        }
    }
}

/// Детерминированный xorshift64* — без внешних зависимостей.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            self.next() as usize % bound
        }
    }
}

/// Валидная программа только из конструкций, общих для v0.4.0 и V0.5.
fn generate_valid(rng: &mut Rng) -> String {
    match rng.below(9) {
        0 => {
            let a = 1 + rng.below(50);
            let b = 1 + rng.below(20);
            format!(
                "(print (+ {a} {b}))\n\
                 (print (- {a} {b}))\n\
                 (print (* {a} {b}))\n\
                 (print (/ {a} {b}))\n\
                 (print (% {a} {b}))\n\
                 (print (pow {b} 3))\n\
                 (print (min {a} {b}))\n\
                 (print (max {a} {b}))\n\
                 (print (abs (- 0 {a})))\n\
                 (print (= {a} {a}))\n\
                 (print (< {b} {a}))\n\
                 (print (and (> {a} 0) (< {b} {a})))\n\
                 (print (or (= {a} 0) (= {b} {b})))\n"
            )
        }
        1 => {
            let n = 1 + rng.below(7);
            let fib = 5 + rng.below(8);
            format!(
                "(define (fact n)\n\
                   (if (= n 0) 1 (* n (fact (- n 1)))))\n\
                 (print (fact {n}))\n\
                 (define (fib n)\n\
                   (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))\n\
                 (print (fib {fib}))\n\
                 (print ((lambda (x) (+ x 1)) 41))\n"
            )
        }
        2 => {
            let values = (0..6)
                .map(|_| rng.below(50).to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let index = rng.below(6);
            let needle = rng.below(50);
            let extra = rng.below(100);
            format!(
                "(define xs (list {values}))\n\
                 (print xs)\n\
                 (print (cons {extra} xs))\n\
                 (print (head xs))\n\
                 (print (tail xs))\n\
                 (print (nth xs {index}))\n\
                 (print (length xs))\n\
                 (print (append xs (list {extra})))\n\
                 (print (contains? xs {needle}))\n\
                 (print (empty? xs))\n\
                 (print (empty? (list)))\n"
            )
        }
        3 => {
            let n = 3 + rng.below(4);
            let values = (1..=n)
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "(define xs (list {values}))\n\
                 (print (list-map xs (lambda (x) (* x 2))))\n\
                 (print (list-filter xs (lambda (x) (> x 2))))\n\
                 (print (list-fold xs 0 (lambda (acc x) (+ acc x))))\n"
            )
        }
        4 => {
            let word = ["lupus", "волк", "hello", "тест"][rng.below(4)];
            let number = rng.below(1000);
            format!(
                "(print (int->str {number}))\n\
                 (print (string-append \"{word}\" \"-\" \"{number}\"))\n\
                 (print (string-length \"{word}\"))\n\
                 (print (string-split \"{word},lupus,{number}\" \",\"))\n\
                 (print (string-reverse \"{word}\"))\n\
                 (print (str->int \"{number}\"))\n"
            )
        }
        5 => {
            let x = 1 + rng.below(100);
            let y = 1 + rng.below(100);
            format!(
                "(defstruct point (x int) (y int))\n\
                 (define p (point {x} {y}))\n\
                 (print p)\n\
                 (print (point-x p))\n\
                 (print (point-y p))\n\
                 (print (match p ((point a b) (+ a b)) (_ 0)))\n\
                 (print (= p (point {x} {y})))\n"
            )
        }
        6 => {
            let a = 1 + rng.below(100);
            let b = 1 + rng.below(100);
            format!(
                "(print (let ((a {a}) (b {b})) (+ a b)))\n\
                 (print ((lambda (x y) (* x y)) {a} {b}))\n\
                 (print (if (< {a} {b}) \"less\" \"ge\"))\n\
                 (print (and (> {a} 0) (< {b} 100)))\n\
                 (print (or (= {a} 0) (> {b} 0)))\n"
            )
        }
        7 => {
            let end = 3 + rng.below(4);
            let limit = 1 + rng.below(4);
            format!(
                "(define-mutable total 0)\n\
                 (for x in (range 1 {end}) (set! total (+ total x)))\n\
                 (print total)\n\
                 (print (range 1 {end}))\n\
                 (define-mutable i 0)\n\
                 (while (< i {limit}) (print i) (set! i (+ i 1)))\n"
            )
        }
        _ => {
            let a = 1 + rng.below(100);
            format!(
                "(assert (= {a} {a}))\n\
                 (assert (> {a} -1))\n\
                 (print \"ok\")\n"
            )
        }
    }
}

/// Программа с гарантированной ошибкой выполнения (exit 1).
fn generate_error(rng: &mut Rng) -> String {
    let a = 1 + rng.below(20);
    match rng.below(15) {
        0 => format!("(print \"before\")\n(/ {a} 0)\n"),
        1 => {
            let index = 5 + rng.below(5);
            format!("(print \"before\")\n(nth (list 1 2 3) {index})\n")
        }
        2 => format!("(print \"before\")\n(+ {a} \"x\")\n"),
        3 => "(assert (= 1 2))\n".to_string(),
        4 => format!("(print \"before\")\n(string-length {a})\n"),
        5 => "(print \"before\")\n(list-map (list 1 2) 5)\n".to_string(),
        6 => "(print \"before\")\n(cons 1 2)\n".to_string(),
        7 => "(print \"before\")\n(append (list 1) 2)\n".to_string(),
        8 => "(print \"before\")\n(length 1)\n".to_string(),
        9 => "(print \"before\")\n(contains? 1 1)\n".to_string(),
        10 => "(print \"before\")\n(empty? 1)\n".to_string(),
        11 => "(print \"before\")\n(min 1)\n".to_string(),
        12 => {
            let exponent = 1 + rng.below(5);
            format!("(print \"before\")\n(pow 2 -{exponent})\n")
        }
        13 => "(print \"before\")\n(str->int 1)\n".to_string(),
        _ => "(print \"before\")\n(string-reverse 1)\n".to_string(),
    }
}

fn generate_programs(seed: u64, count: usize) -> Vec<String> {
    let mut rng = Rng::new(seed);
    (0..count)
        .map(|index| {
            if index % 10 < 7 {
                generate_valid(&mut rng)
            } else {
                generate_error(&mut rng)
            }
        })
        .collect()
}

fn generated_corpus_dir() -> PathBuf {
    manifest_dir().join("target/oracle_corpus_seed42")
}

fn write_generated_corpus() -> Vec<PathBuf> {
    let dir = generated_corpus_dir();
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("удалить старый сгенерированный корпус");
    }
    std::fs::create_dir_all(&dir).expect("создать каталог сгенерированного корпуса");

    generate_programs(GENERATED_SEED, GENERATED_COUNT)
        .into_iter()
        .enumerate()
        .map(|(index, source)| {
            let path = dir.join(format!("generated_{index:03}.lupus"));
            std::fs::write(&path, source).expect("записать сгенерированную программу");
            path
        })
        .collect()
}

fn corpus() -> Vec<PathBuf> {
    let root = manifest_dir();
    let mut files = Vec::new();
    collect_lupus(&root.join("tests/golden"), &mut files);
    collect_lupus(&root.join("examples"), &mut files);
    files.sort();
    files.extend(write_generated_corpus());
    files
}

/// Возвращает причину пропуска для файлов, использующих возможности V0.5,
/// которых нет в v0.4.0.
fn skip_reason(source: &str) -> Option<&'static str> {
    if source.starts_with('\u{feff}') {
        return Some("BOM (V0.5.1)");
    }
    if has_unicode_identifier(source) {
        return Some("Unicode-идентификаторы (V0.5.1)");
    }
    if source.contains("(import ") {
        return Some("import (V0.5)");
    }
    if source.contains("(define-public ") {
        return Some("define-public (V0.5)");
    }
    if source.contains("(defstruct (") {
        return Some("generic defstruct (V0.5)");
    }
    if source.contains("(map ")
        || source.contains("(map-get ")
        || source.contains("(map-keys ")
        || source.contains("(map-values ")
        || source.contains("(map-has? ")
        || source.contains("(map-set ")
        || source.contains("(map-remove ")
    {
        return Some("Map (V0.5)");
    }
    if source.contains("(test ") || source.contains("test/") {
        return Some("test-форма (V0.5)");
    }
    if source.contains("\\u") || source.contains("\\x") {
        return Some("escape-последовательности V0.5");
    }
    None
}

/// Есть ли в исходнике Unicode-буква вне строкового литерала/комментария.
fn has_unicode_identifier(source: &str) -> bool {
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(ch) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if in_block_comment {
            if ch == '|' && chars.peek() == Some(&'#') {
                chars.next();
                in_block_comment = false;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            ';' => in_line_comment = true,
            '#' if chars.peek() == Some(&'|') => {
                chars.next();
                in_block_comment = true;
            }
            c if !c.is_ascii() && c.is_alphabetic() => return true,
            _ => {}
        }
    }
    false
}

/// v0.4.0 печатает диагностику в stdout, V0.5 — JSON в stderr. Для exit 1
/// поэтому сравниваем exitcode и stdout до диагностики (prefix), а для exit 0 —
/// stdout байт-в-байт.
fn stdout_matches(expected: &Output, actual: &Output) -> bool {
    match expected.status.code() {
        Some(0) => expected.stdout == actual.stdout,
        _ => expected.stdout.starts_with(&actual.stdout),
    }
}

#[test]
fn differential_against_v04_oracle() {
    let oracle = oracle_path();
    if !oracle.is_file() {
        if std::env::var("LUPUS_ORACLE_REQUIRED").as_deref() == Ok("1") {
            panic!(
                "LUPUS_ORACLE_REQUIRED=1, но эталонный бинарник {} отсутствует; \
                 запустите scripts/fetch-oracle.sh",
                oracle.display()
            );
        }
        eprintln!(
            "SKIP: эталонный бинарник {} отсутствует; запустите scripts/fetch-oracle.sh",
            oracle.display()
        );
        return;
    }

    let current = PathBuf::from(env!("CARGO_BIN_EXE_lupus"));
    let files = corpus();
    assert!(!files.is_empty(), "пустой корпус дифференциального теста");

    let started = Instant::now();
    let mut passed = 0;
    let mut skipped = Vec::new();
    let mut failures = Vec::new();

    for file in &files {
        let source = std::fs::read_to_string(file)
            .unwrap_or_else(|error| panic!("не удалось прочитать {}: {error}", file.display()));
        if let Some(reason) = skip_reason(&source) {
            skipped.push((file.clone(), reason));
            continue;
        }

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
        if expected_code != actual_code || !stdout_matches(&expected, &actual) {
            failures.push(format!(
                "{}\n  oracle: code={:?} stdout={:?}\n  lupus:  code={:?} stdout={:?}",
                file.display(),
                expected_code,
                String::from_utf8_lossy(&expected.stdout),
                actual_code,
                String::from_utf8_lossy(&actual.stdout),
            ));
        } else {
            passed += 1;
        }
    }

    let elapsed = started.elapsed();
    let generated_valid = (0..GENERATED_COUNT).filter(|index| index % 10 < 7).count();
    println!(
        "ORACLE_REPORT total={} pass={} fail={} skip={} elapsed_ms={}",
        files.len(),
        passed,
        failures.len(),
        skipped.len(),
        elapsed.as_millis()
    );
    println!(
        "ORACLE_GENERATED total={GENERATED_COUNT} valid={generated_valid} error={} seed={GENERATED_SEED}",
        GENERATED_COUNT - generated_valid
    );
    for (path, reason) in &skipped {
        println!("ORACLE_SKIP {}: {reason}", path.display());
    }

    assert!(
        failures.is_empty(),
        "дифференциальные расхождения ({} из {}):\n{}",
        failures.len(),
        files.len(),
        failures.join("\n\n")
    );
}

#[test]
fn generated_corpus_is_deterministic() {
    let first = generate_programs(GENERATED_SEED, GENERATED_COUNT);
    let second = generate_programs(GENERATED_SEED, GENERATED_COUNT);
    assert_eq!(first, second, "корпус должен быть идентичен при seed=42");
    assert_eq!(first.len(), GENERATED_COUNT);

    let valid = (0..GENERATED_COUNT).filter(|index| index % 10 < 7).count();
    assert_eq!(valid, 42, "ожидается 42 валидные программы");
    assert_eq!(
        GENERATED_COUNT - valid,
        18,
        "ожидается 18 программ с ошибкой"
    );

    let forbidden = [
        "(import ",
        "(map ",
        "(tuple",
        "(test ",
        "(loop",
        "(exit",
        "(defstruct (",
        "#lupus",
    ];
    for source in &first {
        for marker in forbidden {
            assert!(
                !source.contains(marker),
                "сгенерированная программа содержит запрещённую конструкцию {marker}:\n{source}"
            );
        }
    }
}
