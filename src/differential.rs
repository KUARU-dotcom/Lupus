//! Differential fuzzing: сравнение двух версий интерпретатора.
//!
//! Одна и та же программа запускается на двух бинарниках: эталон — релиз
//! V0.5.2, текущая версия — собираемая (сейчас V0.7.0). Сравниваются:
//!
//! - код выхода;
//! - stdout (байт-в-байт);
//! - stderr **только по полю `code`** — с V0.6 в диагностику добавился
//!   `location`, поэтому сравнение полного текста давало бы ложные
//!   расхождения.
//!
//! Известные breaking changes V0.5.2 → V0.6 (`and`/`or` возвращают значение,
//! строгая проверка типов `head`/`tail`/…, `()` как пустой список, проверка
//! полей `defstruct`) классифицируются как [`DiffOutcome::Expected`] и не
//! считаются регрессией.
//!
//! Таймаут запуска одной программы по умолчанию — [`DEFAULT_TIMEOUT`] (5 с);
//! на медленном CPU дефолт может давать ложные `timeout`, которые классифицируются
//! как неожиданное расхождение. Таймаут переопределяется переменной окружения
//! `LUPUS_DIFF_TIMEOUT_MS` (миллисекунды; невалидное значение — дефолт).

use std::io;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

/// Таймаут запуска одной программы по умолчанию.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Разбирает значение `LUPUS_DIFF_TIMEOUT_MS` (миллисекунды).
///
/// Допустимо только целое `N > 0`; ноль, мусор и отрицательные числа означают
/// таймаут по умолчанию.
fn parse_timeout_ms(raw: Option<&str>) -> Duration {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_TIMEOUT)
}

/// Действующий таймаут запуска одной программы.
///
/// По умолчанию [`DEFAULT_TIMEOUT`]; переопределяется переменной окружения
/// `LUPUS_DIFF_TIMEOUT_MS`. Невалидное значение возвращает дефолт.
pub fn effective_timeout() -> Duration {
    parse_timeout_ms(std::env::var("LUPUS_DIFF_TIMEOUT_MS").ok().as_deref())
}

/// Результат запуска программы на одном бинарнике.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    /// Код выхода (`None`, если процесс завершился сигналом).
    pub code: Option<i32>,
    /// stdout целиком.
    pub stdout: String,
    /// Поле `code` из JSON-диагностики в stderr, если оно есть.
    pub stderr_code: Option<String>,
    /// stderr целиком (для отчёта, не для сравнения).
    pub stderr: String,
    /// Истёк ли таймаут.
    pub timed_out: bool,
}

impl RunOutcome {
    /// Создаёт результат из вывода процесса.
    pub fn from_output(output: std::process::Output) -> Self {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stderr_code = extract_error_code(&stderr);
        RunOutcome {
            code: output.status.code(),
            stdout,
            stderr_code,
            stderr,
            timed_out: false,
        }
    }

    /// Результат с истёкшим таймаутом.
    pub fn timeout() -> Self {
        RunOutcome {
            code: None,
            stdout: String::new(),
            stderr_code: None,
            stderr: String::new(),
            timed_out: true,
        }
    }

    /// Успешно ли завершилась программа.
    pub fn is_success(&self) -> bool {
        !self.timed_out && self.code == Some(0)
    }
}

/// Известное ожидаемое расхождение V0.5.2 → V0.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakingChange {
    /// `and`/`or` возвращают значение, а не только булево.
    AndOrValue,
    /// Ложны только `false` и `none`; `0` и `""` стали истинными.
    Truthiness,
    /// `()` — литерал пустого списка, а не `none`.
    EmptyListLiteral,
    /// `head`/`tail`/`nth`/`empty?`/`contains?`/`length` на неверном типе
    /// дают `type-mismatch`, а не `none`.
    WrongTypeStrictness,
    /// `head`/`tail` на пустом списке дают `index-out-of-bounds`.
    HeadTailOnEmpty,
    /// Конструкторы `defstruct` проверяют типы полей.
    StructFieldTypeCheck,
}

impl BreakingChange {
    /// Короткое имя для отчётов.
    pub fn as_str(self) -> &'static str {
        match self {
            BreakingChange::AndOrValue => "and-or-value",
            BreakingChange::Truthiness => "truthiness",
            BreakingChange::EmptyListLiteral => "empty-list-literal",
            BreakingChange::WrongTypeStrictness => "wrong-type-strictness",
            BreakingChange::HeadTailOnEmpty => "head-tail-on-empty",
            BreakingChange::StructFieldTypeCheck => "struct-field-type-check",
        }
    }
}

/// Итог сравнения двух запусков.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffOutcome {
    /// Поведение совпало.
    Identical,
    /// Расхождение объясняется известным breaking change.
    Expected(BreakingChange),
    /// Неожиданное расхождение — возможная регрессия.
    Unexpected,
}

/// Подробный отчёт о сравнении.
#[derive(Debug, Clone)]
pub struct DiffReport {
    /// Запуск на базовой версии (эталон V0.5.2).
    pub baseline: RunOutcome,
    /// Запуск на текущей версии (сейчас V0.7.0).
    pub current: RunOutcome,
    /// Классификация расхождения.
    pub outcome: DiffOutcome,
    /// Человекочитаемые отличия (exit/stdout/stderr.code).
    pub details: Vec<String>,
}

impl DiffReport {
    /// Является ли расхождение неожиданным.
    pub fn is_unexpected(&self) -> bool {
        self.outcome == DiffOutcome::Unexpected
    }

    /// Короткая строка для отчёта.
    pub fn summary(&self) -> String {
        let kind = match self.outcome {
            DiffOutcome::Identical => "identical".to_string(),
            DiffOutcome::Expected(change) => format!("expected:{}", change.as_str()),
            DiffOutcome::Unexpected => "unexpected".to_string(),
        };
        format!(
            "{kind} baseline(code={:?}, stdout={:?}, stderr.code={:?}) current(code={:?}, stdout={:?}, stderr.code={:?})",
            self.baseline.code,
            self.baseline.stdout,
            self.baseline.stderr_code,
            self.current.code,
            self.current.stdout,
            self.current.stderr_code
        )
    }
}

/// Запускает `program` на `binary` с таймаутом.
pub fn run_binary(binary: &Path, program: &Path, timeout: Duration) -> io::Result<RunOutcome> {
    let binary = binary.to_path_buf();
    let program = program.to_path_buf();
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        let output = Command::new(&binary).arg(&program).output();
        let _ = sender.send(output);
    });

    match receiver.recv_timeout(timeout) {
        Ok(Ok(output)) => Ok(RunOutcome::from_output(output)),
        Ok(Err(error)) => Err(error),
        Err(_) => Ok(RunOutcome::timeout()),
    }
}

/// Запускает одну программу на двух бинарниках и сравнивает результаты.
pub fn compare_binaries(
    program: &Path,
    baseline: &Path,
    current: &Path,
    timeout: Duration,
) -> io::Result<DiffReport> {
    let source = std::fs::read_to_string(program)?;
    let baseline_outcome = run_binary(baseline, program, timeout)?;
    let current_outcome = run_binary(current, program, timeout)?;
    Ok(compare_outcomes(&source, baseline_outcome, current_outcome))
}

/// Сравнивает два уже полученных результата.
///
/// `source` нужен для классификации известных breaking changes.
pub fn compare_outcomes(source: &str, baseline: RunOutcome, current: RunOutcome) -> DiffReport {
    let mut details = Vec::new();

    if baseline.timed_out || current.timed_out {
        details.push(format!(
            "timeout: baseline={} current={}",
            baseline.timed_out, current.timed_out
        ));
    }
    if baseline.code != current.code {
        details.push(format!(
            "exit code: {:?} vs {:?}",
            baseline.code, current.code
        ));
    }
    if baseline.stdout != current.stdout {
        details.push(format!(
            "stdout: {:?} vs {:?}",
            baseline.stdout, current.stdout
        ));
    }
    if baseline.stderr_code != current.stderr_code {
        details.push(format!(
            "stderr.code: {:?} vs {:?}",
            baseline.stderr_code, current.stderr_code
        ));
    }

    let outcome = if details.is_empty() {
        DiffOutcome::Identical
    } else if let Some(change) = classify_expected(source, &baseline, &current) {
        DiffOutcome::Expected(change)
    } else {
        DiffOutcome::Unexpected
    };

    DiffReport {
        baseline,
        current,
        outcome,
        details,
    }
}

/// Извлекает поле `code` из JSON-диагностики в stderr.
pub fn extract_error_code(stderr: &str) -> Option<String> {
    let marker = "\"code\":\"";
    let start = stderr.find(marker)? + marker.len();
    let rest = &stderr[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Классифицирует расхождение как известное breaking change V0.6.
pub fn classify_expected(
    source: &str,
    baseline: &RunOutcome,
    current: &RunOutcome,
) -> Option<BreakingChange> {
    if baseline.timed_out || current.timed_out {
        return None;
    }

    // V0.6 стал строже к типам: V0.5 возвращал none/false, V0.6 — ошибку.
    if baseline.code == Some(0) && current.code != Some(0) {
        if current.stderr_code.as_deref() == Some("index-out-of-bounds")
            && (contains_call(source, "head") || contains_call(source, "tail"))
        {
            return Some(BreakingChange::HeadTailOnEmpty);
        }
        if current.stderr_code.as_deref() == Some("type-mismatch") {
            if contains_call(source, "defstruct") {
                return Some(BreakingChange::StructFieldTypeCheck);
            }
            if has_strict_type_call(source) {
                return Some(BreakingChange::WrongTypeStrictness);
            }
        }
    }

    // and/or теперь возвращают значение, а ложны только false и none.
    // Ожидаем построчное совпадение, где меняется только логический вывод.
    let has_logic =
        contains_call(source, "and") || contains_call(source, "or") || contains_call(source, "not");
    if has_logic
        && baseline.code == current.code
        && logic_only_difference(
            &baseline.stdout,
            &current.stdout,
            contains_call(source, "not"),
        )
    {
        if contains_call(source, "not")
            && has_bool_to_bool_difference(&baseline.stdout, &current.stdout)
        {
            return Some(BreakingChange::Truthiness);
        }
        return Some(BreakingChange::AndOrValue);
    }

    // () в V0.5 — none, в V0.6 — пустой список.
    if has_empty_list_literal(source)
        && baseline.code == current.code
        && baseline.stdout.trim() == "none"
        && current.stdout.trim() == "()"
    {
        return Some(BreakingChange::EmptyListLiteral);
    }

    None
}

fn has_strict_type_call(source: &str) -> bool {
    [
        "head",
        "tail",
        "nth",
        "empty?",
        "contains?",
        "length",
        "list-map",
        "list-filter",
        "list-fold",
    ]
    .iter()
    .any(|name| contains_call(source, name))
}

/// Содержит ли программа вызов `(name ...)` вне строк и комментариев.
fn contains_call(source: &str, name: &str) -> bool {
    scan_symbols(source, |symbol, _next| symbol == name)
}

/// Есть ли в программе литерал `()` вне строк и комментариев.
fn has_empty_list_literal(source: &str) -> bool {
    scan_symbols(source, |symbol, next| {
        symbol.is_empty() && next == Some(b')')
    })
}

/// Проходит по символам сразу после `(`, игнорируя строки и комментарии.
///
/// `visit(symbol, next_byte)` вызывается для каждого такого символа; для `()`
/// symbol пустой, а `next_byte` — `)`.
fn scan_symbols(source: &str, mut visit: impl FnMut(&str, Option<u8>) -> bool) -> bool {
    let bytes = source.as_bytes();
    let mut index = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;

    while index < bytes.len() {
        let byte = bytes[index];

        if line_comment {
            if byte == b'\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }
        if block_comment {
            if byte == b'|' && bytes.get(index + 1) == Some(&b'#') {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        match byte {
            b'"' => {
                in_string = true;
                index += 1;
            }
            b';' => {
                line_comment = true;
                index += 1;
            }
            b'#' if bytes.get(index + 1) == Some(&b'|') => {
                block_comment = true;
                index += 2;
            }
            b'(' => {
                let mut start = index + 1;
                while start < bytes.len() && bytes[start].is_ascii_whitespace() {
                    start += 1;
                }
                let mut end = start;
                while end < bytes.len() && is_symbol_byte(bytes[end]) {
                    end += 1;
                }
                let symbol = &source[start..end];
                if visit(symbol, bytes.get(end).copied()) {
                    return true;
                }
                index = end.max(index + 1);
            }
            _ => index += 1,
        }
    }

    false
}

fn is_symbol_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'_' | b'-' | b'?' | b'!' | b'*' | b'+' | b'<' | b'>' | b'=' | b'/' | b'%' | b'.'
        )
        || byte >= 0x80
}

/// Совпадают ли stdout построчно, кроме изменений в логическом выводе.
///
/// `allow_bool_bool` разрешает пары `true`/`false` — это нужно для смены
/// истинности `0` и `""` в `not`/`and`/`or`.
fn logic_only_difference(baseline: &str, current: &str, allow_bool_bool: bool) -> bool {
    let baseline_lines: Vec<&str> = baseline.lines().collect();
    let current_lines: Vec<&str> = current.lines().collect();
    if baseline_lines.len() != current_lines.len() {
        return false;
    }

    let mut changed = false;
    for (before, after) in baseline_lines.iter().zip(current_lines.iter()) {
        if before == after {
            continue;
        }
        let before_bool = matches!(*before, "true" | "false");
        let after_bool = matches!(*after, "true" | "false");
        if before_bool == after_bool {
            if before_bool && allow_bool_bool {
                changed = true;
                continue;
            }
            return false;
        }
        changed = true;
    }
    changed
}

fn has_bool_to_bool_difference(baseline: &str, current: &str) -> bool {
    baseline
        .lines()
        .zip(current.lines())
        .any(|(before, after)| {
            before != after
                && matches!(before, "true" | "false")
                && matches!(after, "true" | "false")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(code: i32, stdout: &str, stderr_code: Option<&str>) -> RunOutcome {
        RunOutcome {
            code: Some(code),
            stdout: stdout.to_string(),
            stderr_code: stderr_code.map(str::to_string),
            stderr: String::new(),
            timed_out: false,
        }
    }

    #[test]
    fn identical_when_everything_matches() {
        let report = compare_outcomes(
            "(print (+ 1 2))",
            outcome(0, "3\n", None),
            outcome(0, "3\n", None),
        );
        assert_eq!(report.outcome, DiffOutcome::Identical);
        assert!(report.details.is_empty());
    }

    #[test]
    fn stderr_is_compared_by_code_only() {
        let mut baseline = outcome(1, "", Some("type-mismatch"));
        baseline.stderr = "старый текст".to_string();
        let mut current = outcome(1, "", Some("type-mismatch"));
        current.stderr = "новый текст с location".to_string();
        let report = compare_outcomes("(head 5)", baseline, current);
        assert_eq!(report.outcome, DiffOutcome::Identical);
    }

    #[test]
    fn known_and_or_change_is_expected() {
        let report = compare_outcomes(
            "(print (and 1 2))",
            outcome(0, "true\n", None),
            outcome(0, "2\n", None),
        );
        assert_eq!(
            report.outcome,
            DiffOutcome::Expected(BreakingChange::AndOrValue)
        );
    }

    #[test]
    fn unexpected_difference_is_reported() {
        let report = compare_outcomes(
            "(print (+ 1 2))",
            outcome(0, "3\n", None),
            outcome(0, "4\n", None),
        );
        assert_eq!(report.outcome, DiffOutcome::Unexpected);
        assert!(report.is_unexpected());
    }

    #[test]
    fn extracts_code_from_json() {
        assert_eq!(
            extract_error_code(r#"{"severity":"error","code":"type-mismatch"}"#),
            Some("type-mismatch".to_string())
        );
        assert_eq!(extract_error_code("not json"), None);
    }

    #[test]
    fn parse_timeout_defaults() {
        assert_eq!(parse_timeout_ms(None), DEFAULT_TIMEOUT);
        assert_eq!(parse_timeout_ms(Some("")), DEFAULT_TIMEOUT);
        assert_eq!(parse_timeout_ms(Some("мусор")), DEFAULT_TIMEOUT);
        assert_eq!(parse_timeout_ms(Some("0")), DEFAULT_TIMEOUT);
        assert_eq!(parse_timeout_ms(Some("-5")), DEFAULT_TIMEOUT);
        assert_eq!(parse_timeout_ms(Some("1.5")), DEFAULT_TIMEOUT);
        assert_eq!(
            parse_timeout_ms(Some("99999999999999999999999")),
            DEFAULT_TIMEOUT
        );
    }

    #[test]
    fn parse_timeout_accepts_positive_integer() {
        assert_eq!(parse_timeout_ms(Some("1000")), Duration::from_secs(1));
        assert_eq!(
            parse_timeout_ms(Some(" 15000 ")),
            Duration::from_millis(15000)
        );
        assert_eq!(parse_timeout_ms(Some("60000")), Duration::from_secs(60));
    }
}
