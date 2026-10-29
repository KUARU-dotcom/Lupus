//! Каркас property-based тестов для семантики Lupus.
//!
//! Каждое свойство проверяется на детерминированном наборе входов, который
//! строит [`Generator`] из этапа 1. При нарушении свойства выполняется
//! shrink: значения последовательно уменьшаются до неподвижной точки, после
//! чего выводится минимальный найденный контрпример.
//!
//! Свойства намеренно ограничены `int`, `float`, `str` и `list`: для `map`,
//! `tuple` и `struct` равенство может быть не определено или нестабильно.

use crate::generator::Generator;
use std::fmt;

/// Количество прогонов свойства по умолчанию.
pub const DEFAULT_CASES: usize = 1000;
/// Детерминированный seed по умолчанию.
pub const DEFAULT_SEED: u64 = 0x5052_4F50_4552_5459;

/// Описание нарушенного свойства и минимального контрпримера.
#[derive(Debug, Clone)]
pub struct Failure {
    /// Имя свойства.
    pub property: String,
    /// Сколько прогонов выполнено до нарушения.
    pub cases_run: usize,
    /// Минимальный контрпример в человекочитаемом виде.
    pub counterexample: String,
    /// Lupus-выражение контрпримера.
    pub expression: String,
    /// Ошибка интерпретатора (`assert-failed` или runtime-ошибка).
    pub error: String,
}

impl Failure {
    /// Форматирует отчёт о нарушении для panic/CLI.
    pub fn report(&self) -> String {
        format!(
            "свойство '{}' нарушено после {} прогонов\nконтрпример: {}\nвыражение: {}\nошибка: {}",
            self.property, self.cases_run, self.counterexample, self.expression, self.error
        )
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.report())
    }
}

impl std::error::Error for Failure {}

/// Вход property-теста: умеет построить проверяемое выражение, описать себя
/// и предложить более простые варианты для shrink.
pub trait Case: Clone {
    /// Lupus-выражение, которое должно быть истинным.
    fn expression(&self) -> String;
    /// Человекочитаемое описание входа.
    fn description(&self) -> String;
    /// Более простые кандидаты (в порядке от меньших к большим).
    fn shrink(&self) -> Vec<Self>;
}

/// Проверяет свойство на `cases` сгенерированных входах.
///
/// `generate` получает общий [`Generator`], поэтому входы детерминированы для
/// заданного seed. При первом нарушении вход минимизируется shrink-циклом.
pub fn check<C, G>(property: &str, cases: usize, seed: u64, mut generate: G) -> Result<(), Failure>
where
    C: Case,
    G: FnMut(&mut Generator) -> C,
{
    let total = cases.max(1);
    let mut generator = Generator::new(seed);

    for index in 0..total {
        let case = generate(&mut generator);
        let expression = case.expression();
        if holds(&expression).is_err() {
            let (minimal, error) = minimize(case);
            return Err(Failure {
                property: property.to_string(),
                cases_run: index + 1,
                counterexample: minimal.description(),
                expression: minimal.expression(),
                error,
            });
        }
    }

    Ok(())
}

/// Выполняет выражение через реальный интерпретатор: `(assert <expr>)`.
fn holds(expression: &str) -> Result<(), String> {
    crate::run_source(&format!("(assert {expression})")).map_err(|error| error.to_json())
}

/// Уменьшает контрпример до неподвижной точки shrink-функции.
fn minimize<C: Case>(mut case: C) -> (C, String) {
    let mut error = match holds(&case.expression()) {
        Err(error) => error,
        Ok(()) => return (case, String::new()),
    };

    loop {
        let mut next = None;
        for candidate in case.shrink() {
            if let Err(candidate_error) = holds(&candidate.expression()) {
                next = Some((candidate, candidate_error));
                break;
            }
        }

        match next {
            Some((candidate, candidate_error)) => {
                case = candidate;
                error = candidate_error;
            }
            None => return (case, error),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumKind {
    Int,
    Float,
}

impl NumKind {
    fn name(self) -> &'static str {
        match self {
            NumKind::Int => "int",
            NumKind::Float => "float",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Number {
    Int(i64),
    Float(f64),
}

impl Number {
    fn literal(self) -> String {
        match self {
            Number::Int(value) => value.to_string(),
            Number::Float(value) => float_literal(value),
        }
    }

    fn shrink(self) -> Vec<Number> {
        match self {
            Number::Int(value) => {
                let mut candidates = Vec::new();
                push_unique(&mut candidates, Number::Int(0));
                push_unique(&mut candidates, Number::Int(value / 2));
                if value > 0 {
                    push_unique(&mut candidates, Number::Int(value - 1));
                } else if value < 0 {
                    push_unique(&mut candidates, Number::Int(value + 1));
                }
                candidates.retain(|candidate| *candidate != self);
                candidates
            }
            Number::Float(value) => {
                let mut candidates = Vec::new();
                push_unique(&mut candidates, Number::Float(0.0));
                if value.abs() > 1.0 {
                    push_unique(&mut candidates, Number::Float(value / 2.0));
                }
                if value.abs() >= 1.0 {
                    let step = if value > 0.0 {
                        value - 0.5
                    } else {
                        value + 0.5
                    };
                    push_unique(&mut candidates, Number::Float(step));
                }
                candidates.retain(|candidate| *candidate != self);
                candidates
            }
        }
    }
}

fn push_unique<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn float_literal(value: f64) -> String {
    let mut text = format!("{value:?}");
    if !text.contains('.') && !text.contains('e') && !text.contains('E') {
        text.push_str(".0");
    }
    text
}

fn list_literal(values: &[i64]) -> String {
    if values.is_empty() {
        "(list)".to_string()
    } else {
        format!(
            "(list {})",
            values
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

fn string_literal(value: &str) -> String {
    let mut result = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\t' => result.push_str("\\t"),
            '\r' => result.push_str("\\r"),
            '\0' => result.push_str("\\0"),
            character if (character as u32) < 0x20 => {
                result.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => result.push(character),
        }
    }
    result.push('"');
    result
}

fn shrink_int(value: i64) -> Vec<i64> {
    let mut candidates = Vec::new();
    push_unique(&mut candidates, 0);
    push_unique(&mut candidates, value / 2);
    if value > 0 {
        push_unique(&mut candidates, value - 1);
    } else if value < 0 {
        push_unique(&mut candidates, value + 1);
    }
    candidates.retain(|candidate| *candidate != value);
    candidates
}

fn shrink_int_list(values: &[i64]) -> Vec<Vec<i64>> {
    let mut candidates = Vec::new();

    if !values.is_empty() {
        candidates.push(Vec::new());
        for index in 0..values.len() {
            let mut candidate = values.to_vec();
            candidate.remove(index);
            candidates.push(candidate);
        }
    }

    for (index, value) in values.iter().enumerate() {
        for smaller in shrink_int(*value) {
            let mut candidate = values.to_vec();
            candidate[index] = smaller;
            candidates.push(candidate);
        }
    }

    sort_lists(candidates, values)
}

fn shrink_nonempty_int_list(values: &[i64]) -> Vec<Vec<i64>> {
    let mut candidates = Vec::new();

    if values.len() > 1 {
        for index in 0..values.len() {
            let mut candidate = values.to_vec();
            candidate.remove(index);
            candidates.push(candidate);
        }
    }

    for (index, value) in values.iter().enumerate() {
        for smaller in shrink_int(*value) {
            let mut candidate = values.to_vec();
            candidate[index] = smaller;
            candidates.push(candidate);
        }
    }

    sort_lists(candidates, values)
}

fn sort_lists(mut candidates: Vec<Vec<i64>>, original: &[i64]) -> Vec<Vec<i64>> {
    candidates.retain(|candidate| candidate.as_slice() != original);
    candidates.sort_by_key(|candidate| {
        (
            candidate.len(),
            candidate
                .iter()
                .map(|value| value.unsigned_abs())
                .sum::<u64>(),
        )
    });
    candidates
}

fn shrink_string(value: &str) -> Vec<String> {
    let characters: Vec<char> = value.chars().collect();
    if characters.is_empty() {
        return Vec::new();
    }

    let mut candidates = vec![String::new()];
    for index in 0..characters.len() {
        let mut candidate = characters.clone();
        candidate.remove(index);
        candidates.push(candidate.into_iter().collect());
    }
    candidates.retain(|candidate| candidate != value);
    candidates.sort_by_key(|candidate| candidate.chars().count());
    candidates
}

#[derive(Clone)]
struct AddCase {
    kind: NumKind,
    values: Vec<Number>,
    associative: bool,
}

impl Case for AddCase {
    fn expression(&self) -> String {
        let values: Vec<String> = self.values.iter().map(|value| value.literal()).collect();
        if self.associative {
            format!(
                "(= (+ (+ {} {}) {}) (+ {} (+ {} {})))",
                values[0], values[1], values[2], values[0], values[1], values[2]
            )
        } else {
            format!(
                "(= (+ {} {}) (+ {} {}))",
                values[0], values[1], values[1], values[0]
            )
        }
    }

    fn description(&self) -> String {
        format!(
            "{}: {}",
            self.kind.name(),
            self.values
                .iter()
                .map(|value| value.literal())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    fn shrink(&self) -> Vec<Self> {
        let mut candidates = Vec::new();
        for (index, value) in self.values.iter().enumerate() {
            for smaller in value.shrink() {
                let mut values = self.values.clone();
                values[index] = smaller;
                candidates.push(AddCase {
                    kind: self.kind,
                    values,
                    associative: self.associative,
                });
            }
        }
        candidates
    }
}

#[derive(Clone)]
struct ListMapCase {
    xs: Vec<i64>,
}

impl Case for ListMapCase {
    fn expression(&self) -> String {
        let list = list_literal(&self.xs);
        format!("(= (length (list-map {list} (lambda (x) (+ x 1)))) (length {list}))")
    }

    fn description(&self) -> String {
        format!("xs = {}", list_literal(&self.xs))
    }

    fn shrink(&self) -> Vec<Self> {
        shrink_int_list(&self.xs)
            .into_iter()
            .map(|xs| ListMapCase { xs })
            .collect()
    }
}

#[derive(Clone)]
struct FilterLengthCase {
    xs: Vec<i64>,
}

impl Case for FilterLengthCase {
    fn expression(&self) -> String {
        let list = list_literal(&self.xs);
        format!("(<= (length (list-filter {list} (lambda (x) (> x 0)))) (length {list}))")
    }

    fn description(&self) -> String {
        format!("xs = {}", list_literal(&self.xs))
    }

    fn shrink(&self) -> Vec<Self> {
        shrink_int_list(&self.xs)
            .into_iter()
            .map(|xs| FilterLengthCase { xs })
            .collect()
    }
}

#[derive(Clone)]
struct AppendCase {
    xs: Vec<i64>,
    ys: Vec<i64>,
}

impl Case for AppendCase {
    fn expression(&self) -> String {
        let xs = list_literal(&self.xs);
        let ys = list_literal(&self.ys);
        format!("(= (length (append {xs} {ys})) (+ (length {xs}) (length {ys})))")
    }

    fn description(&self) -> String {
        format!(
            "xs = {}, ys = {}",
            list_literal(&self.xs),
            list_literal(&self.ys)
        )
    }

    fn shrink(&self) -> Vec<Self> {
        let mut candidates = Vec::new();
        for xs in shrink_int_list(&self.xs) {
            candidates.push(AppendCase {
                xs,
                ys: self.ys.clone(),
            });
        }
        for ys in shrink_int_list(&self.ys) {
            candidates.push(AppendCase {
                xs: self.xs.clone(),
                ys,
            });
        }
        candidates.sort_by_key(|candidate| {
            (
                candidate.xs.len() + candidate.ys.len(),
                candidate
                    .xs
                    .iter()
                    .chain(candidate.ys.iter())
                    .map(|value| value.unsigned_abs())
                    .sum::<u64>(),
            )
        });
        candidates
    }
}

#[derive(Clone)]
struct StringReverseCase {
    value: String,
}

impl Case for StringReverseCase {
    fn expression(&self) -> String {
        let literal = string_literal(&self.value);
        format!("(= (string-reverse (string-reverse {literal})) {literal})")
    }

    fn description(&self) -> String {
        format!("s = {}", string_literal(&self.value))
    }

    fn shrink(&self) -> Vec<Self> {
        shrink_string(&self.value)
            .into_iter()
            .map(|value| StringReverseCase { value })
            .collect()
    }
}

#[derive(Clone)]
struct ZeroCase {
    kind: NumKind,
    value: Number,
    multiply: bool,
}

impl Case for ZeroCase {
    fn expression(&self) -> String {
        let value = self.value.literal();
        match (self.kind, self.multiply) {
            (NumKind::Int, true) => format!("(= (* {value} 0) 0)"),
            (NumKind::Int, false) => format!("(= (+ {value} 0) {value})"),
            (NumKind::Float, true) => format!("(= (* {value} 0.0) 0.0)"),
            (NumKind::Float, false) => format!("(= (+ {value} 0.0) {value})"),
        }
    }

    fn description(&self) -> String {
        format!(
            "{} {} = {}",
            if self.multiply {
                "mul-zero:"
            } else {
                "add-zero:"
            },
            self.kind.name(),
            self.value.literal()
        )
    }

    fn shrink(&self) -> Vec<Self> {
        self.value
            .shrink()
            .into_iter()
            .map(|value| ZeroCase {
                kind: self.kind,
                value,
                multiply: self.multiply,
            })
            .collect()
    }
}

#[derive(Clone)]
struct HeadSomeCase {
    xs: Vec<i64>,
}

impl Case for HeadSomeCase {
    fn expression(&self) -> String {
        let list = list_literal(&self.xs);
        format!("(match (head {list}) ((some _) true) (none false))")
    }

    fn description(&self) -> String {
        format!("xs = {}", list_literal(&self.xs))
    }

    fn shrink(&self) -> Vec<Self> {
        shrink_nonempty_int_list(&self.xs)
            .into_iter()
            .map(|xs| HeadSomeCase { xs })
            .collect()
    }
}

fn next_kind(generator: &mut Generator) -> NumKind {
    if generator.next_bool() {
        NumKind::Int
    } else {
        NumKind::Float
    }
}

fn next_number(generator: &mut Generator, kind: NumKind) -> Number {
    match kind {
        NumKind::Int => Number::Int(generator.next_int(-1000, 1000)),
        NumKind::Float => Number::Float(generator.next_exact_float()),
    }
}

fn next_int_list(generator: &mut Generator) -> Vec<i64> {
    generator.next_list(6, |generator| generator.next_int(-50, 50))
}

/// Коммутативность сложения для `int` и `float`.
pub fn check_commutative_add(cases: usize, seed: u64) -> Result<(), Failure> {
    check("commutative-add", cases, seed, |generator| {
        let kind = next_kind(generator);
        let values = vec![next_number(generator, kind), next_number(generator, kind)];
        AddCase {
            kind,
            values,
            associative: false,
        }
    })
}

/// Ассоциативность сложения для `int` и `float`.
pub fn check_assoc_add(cases: usize, seed: u64) -> Result<(), Failure> {
    check("associative-add", cases, seed, |generator| {
        let kind = next_kind(generator);
        let values = (0..3)
            .map(|_| next_number(generator, kind))
            .collect::<Vec<_>>();
        AddCase {
            kind,
            values,
            associative: true,
        }
    })
}

/// `length (list-map xs f) == length xs`.
pub fn check_length_map(cases: usize, seed: u64) -> Result<(), Failure> {
    check("length-map", cases, seed, |generator| ListMapCase {
        xs: next_int_list(generator),
    })
}

/// Фильтр не увеличивает длину списка.
pub fn check_length_filter(cases: usize, seed: u64) -> Result<(), Failure> {
    check("length-filter", cases, seed, |generator| FilterLengthCase {
        xs: next_int_list(generator),
    })
}

/// Длина конкатенации равна сумме длин.
pub fn check_length_append(cases: usize, seed: u64) -> Result<(), Failure> {
    check("length-append", cases, seed, |generator| AppendCase {
        xs: next_int_list(generator),
        ys: next_int_list(generator),
    })
}

/// Двойное обращение строки возвращает исходную строку.
pub fn check_string_reverse(cases: usize, seed: u64) -> Result<(), Failure> {
    check("string-reverse", cases, seed, |generator| {
        StringReverseCase {
            value: generator.next_string(),
        }
    })
}

/// Умножение на ноль для `int` и `float`.
pub fn check_mul_zero(cases: usize, seed: u64) -> Result<(), Failure> {
    check("mul-zero", cases, seed, |generator| {
        let kind = next_kind(generator);
        ZeroCase {
            kind,
            value: next_number(generator, kind),
            multiply: true,
        }
    })
}

/// Сложение с нулём для `int` и `float`.
pub fn check_add_zero(cases: usize, seed: u64) -> Result<(), Failure> {
    check("add-zero", cases, seed, |generator| {
        let kind = next_kind(generator);
        ZeroCase {
            kind,
            value: next_number(generator, kind),
            multiply: false,
        }
    })
}

/// `head` непустого списка имеет форму `(some ...)`, а не `none`.
pub fn check_head_some(cases: usize, seed: u64) -> Result<(), Failure> {
    check("head-some", cases, seed, |generator| {
        let mut xs = next_int_list(generator);
        if xs.is_empty() {
            xs.push(generator.next_int(-50, 50));
        }
        HeadSomeCase { xs }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Failing(i64);

    impl Case for Failing {
        fn expression(&self) -> String {
            format!("(= {} 0)", self.0)
        }

        fn description(&self) -> String {
            format!("value = {}", self.0)
        }

        fn shrink(&self) -> Vec<Self> {
            if self.0 > 1 {
                vec![Failing(self.0 - 1)]
            } else {
                Vec::new()
            }
        }
    }

    #[test]
    fn shrinking_reports_minimal_counterexample() {
        let failure = check("always-false", 1, 1, |_| Failing(100)).unwrap_err();
        assert_eq!(failure.counterexample, "value = 1");
        assert!(failure.expression.contains("(= 1 0)"));
    }
}
