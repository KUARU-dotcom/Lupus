//! Тесты AST-shrinker (V0.7 этап 3).
//!
//! Баг воспроизводится runtime-ошибкой `index-out-of-bounds`: так проверка
//! остаётся детерминированной и не зависит от наличия реальной паники в
//! интерпретаторе. Механизм shrinker не зависит от природы отказа — fuzz
//! передаёт ему проверку panic/аварийного кода.

use lupus::generator::is_balanced;
use lupus::shrinker::shrink_source;

/// Воспроизводится ли баг: `(nth <пустой список> ...)`.
fn reproduces_index_error(source: &str) -> bool {
    match lupus::run_source(source) {
        Err(error) => error.code == "index-out-of-bounds",
        Ok(()) => false,
    }
}

/// Программа ровно из 500 строк: 456 не относящихся к багу форм верхнего
/// уровня и одна глубокая ветка `if`/`begin` с 40 «шумовыми» вызовами,
/// внутри которой спрятано падающее выражение.
fn large_failing_program() -> String {
    let mut source = String::new();

    for index in 0..456 {
        source.push_str(&format!("(define junk{index} {index})\n"));
    }

    source.push_str("(if true\n");
    source.push_str("  (begin\n");
    for index in 0..40 {
        source.push_str(&format!("    (define noise{index} {index})\n"));
    }
    source.push_str("    (nth (list) 0))\n");
    source.push_str("  (print 999))\n");

    source
}

#[test]
fn test_shrink_reduces_size() {
    let source = large_failing_program();
    assert_eq!(source.lines().count(), 500);

    let report = shrink_source(&source, reproduces_index_error).expect("shrink");

    println!(
        "SHRINK_EXAMPLE original_lines={} minimized_lines={} attempts={} accepted={}\n{}",
        report.original_lines,
        report.minimized_lines,
        report.attempts,
        report.accepted,
        report.minimized_source
    );

    assert!(
        report.original_lines >= 500,
        "исходная программа должна быть не меньше 500 строк, получено {}",
        report.original_lines
    );
    assert!(
        report.minimized_lines < 20,
        "минимальное репро должно быть < 20 строк, получено {}:\n{}",
        report.minimized_lines,
        report.minimized_source
    );
    assert!(
        report.accepted > 0,
        "shrink не принял ни одного преобразования"
    );
    assert!(
        report.reduction_percent() >= 90,
        "ожидалось сокращение ≥ 90%, получено {}%",
        report.reduction_percent()
    );
}

#[test]
fn test_shrink_preserves_bug() {
    let source = large_failing_program();
    assert!(reproduces_index_error(&source));

    let report = shrink_source(&source, reproduces_index_error).expect("shrink");

    assert!(
        reproduces_index_error(&report.minimized_source),
        "минимальное репро больше не воспроизводит баг:\n{}",
        report.minimized_source
    );

    let error = lupus::run_source(&report.minimized_source).expect_err("ожидалась ошибка");
    assert_eq!(error.code, "index-out-of-bounds");
}

#[test]
fn test_shrink_balance() {
    let source = large_failing_program();
    let mut checked = 0usize;

    let report = shrink_source(&source, |candidate| {
        checked += 1;
        assert!(
            is_balanced(candidate),
            "кандидат с несбалансированными скобками:\n{candidate}"
        );
        reproduces_index_error(candidate)
    })
    .expect("shrink");

    assert!(checked > 0, "проверяющее замыкание не вызывалось");
    assert!(
        is_balanced(&report.minimized_source),
        "минимальное репро несбалансировано:\n{}",
        report.minimized_source
    );
    assert!(
        lupus::parse_source(&report.minimized_source).is_ok(),
        "минимальное репро не парсится:\n{}",
        report.minimized_source
    );
}
