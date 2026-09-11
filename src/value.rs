//! Runtime-значения языка Lupus.
//!
//! Поддерживаются только необходимые для прототипа типы: целые числа,
//! булевы значения, строки, списки, литерал `none` и замыкания.

use crate::ast::Expr;
use crate::environment::Environment;
use std::cell::RefCell;
use std::rc::Rc;

/// Замыкание — пользовательская функция, захватившая окружающее окружение.
#[derive(Debug, Clone)]
pub struct Closure {
    /// Имена параметров.
    pub params: Vec<String>,
    /// Тело функции (одно выражение).
    pub body: Expr,
    /// Лексическое окружение, в котором было создано замыкание.
    pub env: Rc<RefCell<Environment>>,
}

/// Runtime-значение языка Lupus.
#[derive(Debug, Clone)]
pub enum Value {
    /// Целое число (i64).
    Int(i64),
    /// Булево значение.
    Bool(bool),
    /// Строка (UTF-8).
    Str(String),
    /// Список значений.
    List(Vec<Value>),
    /// Литерал `none` (отсутствие значения / пустой Option).
    None,
    /// Значение Option: (some v).
    Some(Box<Value>),
    /// Успешный Result: (success v).
    Success(Box<Value>),
    /// Ошибка Result: (failure msg).
    Failure(Box<Value>),
    /// Замыкание (функция).
    Closure(Closure),
}

/// Равенство значений для оператора `=`.
///
/// Замыкания не сравниваются между собой (всегда `false`).
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::None, Value::None) => true,
            (Value::Some(a), Value::Some(b)) => *a == *b,
            (Value::Success(a), Value::Success(b)) => *a == *b,
            (Value::Failure(a), Value::Failure(b)) => *a == *b,
            _ => false,
        }
    }
}

/// Проверка «истинности» значения, совместимая с семантикой Python
/// (используется интерпретатором в `if`, `while`, `and`, `or`, `not`).
///
/// `0`, пустая строка, пустой список, `none` и `Failure` считаются ложными.
pub fn is_truthy(value: &Value) -> bool {
    match value {
        Value::None => false,
        Value::Failure(_) => false,
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::Str(s) => !s.is_empty(),
        Value::List(items) => !items.is_empty(),
        Value::Some(_) => true,
        Value::Success(_) => true,
        Value::Closure(_) => true,
    }
}

/// Преобразование значения Lupus в строку для печати.
///
/// Списки печатаются в Lisp-подобном виде: `(1 2 3)`.
/// Вариант B (решение владельца): строки внутри составных значений
/// (список, some/success/failure) печатаются в двойных кавычках —
/// `("a" "b")`, `(some "x")`, `(failure "boom")`. Верхнеуровневая `print`
/// строки остаётся без кавычек.
pub fn value_to_string(value: &Value) -> String {
    value_to_string_inner(value, false)
}

/// Внутреннее преобразование с флагом «внутри составного значения».
///
/// При `in_container == true` строки дополнительно оборачиваются в кавычки.
fn value_to_string_inner(value: &Value, in_container: bool) -> String {
    match value {
        Value::None => "none".to_string(),
        Value::Bool(b) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Value::Int(i) => i.to_string(),
        Value::Str(s) => {
            if in_container {
                format!("\"{}\"", s)
            } else {
                s.clone()
            }
        }
        Value::List(items) => {
            let inner = items
                .iter()
                .map(|item| value_to_string_inner(item, true))
                .collect::<Vec<String>>()
                .join(" ");
            format!("({})", inner)
        }
        Value::Some(v) => format!("(some {})", value_to_string_inner(v, true)),
        Value::Success(v) => format!("(success {})", value_to_string_inner(v, true)),
        Value::Failure(v) => format!("(failure {})", value_to_string_inner(v, true)),
        Value::Closure(_) => "function".to_string(),
    }
}
