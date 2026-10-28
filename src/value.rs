//! Runtime-значения языка Lupus.
//!
//! Поддерживаются только необходимые для прототипа типы: целые числа,
//! булевы значения, строки, списки, литерал `none` и замыкания.

use crate::ast::Expr;
use crate::builtins::texas::{Listener, Socket};
use crate::environment::Environment;
use crate::error::Error;
use crate::interpreter::Interpreter;
use std::cell::RefCell;
use std::fmt;
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

/// Арность нативной функции.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity {
    /// Ровно N аргументов.
    Exact(usize),
    /// Не меньше N аргументов.
    AtLeast(usize),
    /// Любое число аргументов, включая ноль.
    Any,
}

impl Arity {
    /// Проверяет число аргументов; единственная точка проверки арности
    /// нативных функций.
    #[inline]
    pub fn check(self, name: &str, got: usize) -> Result<(), Error> {
        // Квалифицированные имена сохраняют прежний формат сообщений пакетов.
        if name.contains('/') {
            if let Arity::Exact(expected) = self {
                if got != expected {
                    return Err(Error::arity(format!(
                        "{name} ожидает {expected} аргумент(ов), получено {got}"
                    )));
                }
            }
        }
        let error = match self {
            Arity::Exact(expected) if got != expected => Some(format!(
                "({name} ...) требует ровно {expected} {}, получено {got}",
                arguments_word(expected)
            )),
            Arity::AtLeast(minimum) if got < minimum => Some(format!(
                "({name} ...) требует минимум {minimum} {}, получено {got}",
                arguments_word(minimum)
            )),
            _ => None,
        };
        match error {
            Some(message) => Err(Error::arity(message)),
            None => Ok(()),
        }
    }
}

/// Склонение слова «аргумент» для сообщений об арности.
fn arguments_word(count: usize) -> &'static str {
    let tail = count % 100;
    if (11..=14).contains(&tail) {
        return "аргументов";
    }
    match count % 10 {
        1 => "аргумент",
        2..=4 => "аргумента",
        _ => "аргументов",
    }
}

/// Функция ядра без доступа к интерпретатору.
pub type PureBuiltinFn = fn(&[Value]) -> Result<Value, Error>;

/// Функция ядра, которой нужен интерпретатор и текущее окружение
/// (например, `list-map`, `print`, `gc-collect`).
pub type ContextBuiltinFn =
    fn(&Interpreter, &Rc<RefCell<Environment>>, &[Value]) -> Result<Value, Error>;

/// Реализация встроенной функции ядра.
#[derive(Clone, Copy)]
pub enum BuiltinImpl {
    /// Чистая функция над значениями.
    Pure(PureBuiltinFn),
    /// Функция, вызывающая замыкания или печатающая значения.
    Context(ContextBuiltinFn),
}

impl fmt::Debug for BuiltinImpl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuiltinImpl::Pure(_) => f.write_str("Pure(..)"),
            BuiltinImpl::Context(_) => f.write_str("Context(..)"),
        }
    }
}

/// Реализация нативной функции: ядро или функция пакета `senko`/`kaltsit`/`w`.
#[derive(Clone)]
pub enum NativeImpl {
    /// Встроенная функция ядра.
    Builtin(BuiltinImpl),
    /// Функция нативного пакета.
    Module {
        /// Имя пакета.
        package: String,
        /// Имя функции внутри пакета.
        symbol: String,
    },
}

impl fmt::Debug for NativeImpl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NativeImpl::Builtin(_) => f.write_str("Builtin(..)"),
            NativeImpl::Module { package, symbol } => f
                .debug_struct("Module")
                .field("package", package)
                .field("symbol", symbol)
                .finish(),
        }
    }
}

/// Runtime-значение языка Lupus.
#[derive(Debug, Clone)]
pub enum Value {
    /// Целое число (i64).
    Int(i64),
    /// Число с плавающей точкой (f64).
    Float(f64),
    /// Булево значение.
    Bool(bool),
    /// Строка (UTF-8).
    Str(String),
    /// Список значений.
    List(Vec<Value>),
    /// Кортеж произвольной арности (ЭТАП 2 V0.4).
    Tuple(Vec<Value>),
    /// Ассоциативный массив: упорядоченный по вставке список пар.
    ///
    /// Ключом может быть любое значение, кроме `Float`-NaN.
    Map(Vec<(Value, Value)>),
    /// Структура, созданная через `defstruct` (ЭТАП 3 V0.4 / V0.5).
    /// Поля хранятся позиционно; имя типа — для аксессоров и равенства.
    Struct { name: String, fields: Vec<Value> },
    /// Opaque сетевой сокет (`texas`). Не сериализуется в `SendValue`.
    Socket(Rc<Socket>),
    /// Opaque TCP-слушатель (`texas`). Не сериализуется в `SendValue`.
    Listener(Rc<Listener>),
    /// Литерал `none` (отсутствие значения / пустой Option).
    None,
    /// Значение Option: (some v).
    Some(Box<Value>),
    /// Успешный Result: (success v).
    Success(Box<Value>),
    /// Ошибка Result: (failure msg).
    Failure(Box<Value>),
    /// Замыкание (пользовательская функция).
    Closure(Closure),
    /// Нативная функция: встроенная или из пакета стандартной библиотеки.
    Native {
        /// Полное имя функции (например, `abs` или `senko/sqrt`).
        name: String,
        /// Арность.
        arity: Arity,
        /// Реализация.
        func: NativeImpl,
    },
}

/// Объект, умеющий перечислять ссылки на другие GC-объекты.
///
/// Значения обходят вложенные значения (элементы списков, пары `Map`, поля
/// структур, `some`/`success`/`failure`), а замыкания сообщают своё
/// окружение. `Environment` переопределяет оба метода: значения из `bindings`
/// и родительское окружение.
pub trait Traversable {
    /// Перечисляет значения, на которые ссылается объект.
    fn visit_values(&self, _visitor: &mut dyn FnMut(&Value)) {}

    /// Перечисляет окружения, на которые ссылается объект.
    fn visit_envs(&self, _visitor: &mut dyn FnMut(&Rc<RefCell<Environment>>)) {}
}

impl Traversable for Value {
    fn visit_values(&self, visitor: &mut dyn FnMut(&Value)) {
        match self {
            Value::List(items) | Value::Tuple(items) => {
                for item in items {
                    visitor(item);
                }
            }
            Value::Map(pairs) => {
                for (key, value) in pairs {
                    visitor(key);
                    visitor(value);
                }
            }
            Value::Struct { fields, .. } => {
                for field in fields {
                    visitor(field);
                }
            }
            Value::Some(value) | Value::Success(value) | Value::Failure(value) => {
                visitor(value);
            }
            Value::Int(_)
            | Value::Float(_)
            | Value::Bool(_)
            | Value::Str(_)
            | Value::None
            | Value::Socket(_)
            | Value::Listener(_)
            | Value::Closure(_)
            | Value::Native { .. } => {}
        }
    }

    fn visit_envs(&self, visitor: &mut dyn FnMut(&Rc<RefCell<Environment>>)) {
        if let Value::Closure(closure) = self {
            closure.visit_envs(visitor);
        }
    }
}

impl Traversable for Closure {
    fn visit_envs(&self, visitor: &mut dyn FnMut(&Rc<RefCell<Environment>>)) {
        visitor(&self.env);
    }
}

impl Traversable for Vec<Value> {
    fn visit_values(&self, visitor: &mut dyn FnMut(&Value)) {
        for value in self {
            visitor(value);
        }
    }
}

impl Value {
    /// Приблизительный размер значения в байтах для отчёта GC.
    ///
    /// Окружения замыканий не учитываются здесь: они считаются отдельными
    /// объектами в реестре GC.
    pub fn estimated_size(&self) -> usize {
        use std::mem::size_of;

        let base = size_of::<Value>();
        match self {
            Value::Int(_) | Value::Float(_) | Value::Bool(_) | Value::None => base,
            Value::Str(text) => base + text.capacity(),
            Value::List(items) | Value::Tuple(items) => {
                base + items.capacity() * size_of::<Value>()
                    + items.iter().map(Value::estimated_size).sum::<usize>()
            }
            Value::Map(pairs) => {
                base + pairs.capacity() * size_of::<(Value, Value)>()
                    + pairs
                        .iter()
                        .map(|(key, value)| key.estimated_size() + value.estimated_size())
                        .sum::<usize>()
            }
            Value::Struct { name, fields } => {
                base + name.capacity()
                    + fields.capacity() * size_of::<Value>()
                    + fields.iter().map(Value::estimated_size).sum::<usize>()
            }
            Value::Socket(_) => base + size_of::<Socket>(),
            Value::Listener(_) => base + size_of::<Listener>(),
            Value::Some(value) | Value::Success(value) | Value::Failure(value) => {
                base + value.estimated_size()
            }
            Value::Closure(closure) => {
                base + size_of::<Closure>()
                    + closure.params.capacity() * size_of::<String>()
                    + closure
                        .params
                        .iter()
                        .map(|param| param.capacity())
                        .sum::<usize>()
            }
            Value::Native { name, .. } => base + name.capacity(),
        }
    }
}

/// Равенство значений для оператора `=`.
///
/// Замыкания не сравниваются между собой (всегда `false`).
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(key, value)| {
                        b.iter()
                            .find(|(other_key, _)| other_key == key)
                            .is_some_and(|(_, other_value)| other_value == value)
                    })
            }
            (
                Value::Struct {
                    name: an,
                    fields: af,
                },
                Value::Struct {
                    name: bn,
                    fields: bf,
                },
            ) => an == bn && af == bf,
            (Value::Socket(a), Value::Socket(b)) => Rc::ptr_eq(a, b),
            (Value::Listener(a), Value::Listener(b)) => Rc::ptr_eq(a, b),
            (Value::None, Value::None) => true,
            (Value::Some(a), Value::Some(b)) => *a == *b,
            (Value::Success(a), Value::Success(b)) => *a == *b,
            (Value::Failure(a), Value::Failure(b)) => *a == *b,
            _ => false,
        }
    }
}

/// Проверка «истинности» значения (V0.6).
///
/// Ложны только `false` и `none`. Все остальные значения истинны, включая
/// `0`, `0.0`, пустую строку, пустые списки/кортежи/Map и `failure`.
/// Используется интерпретатором в `if`, `while`, `cond`, `assert`, `and`,
/// `or`, `not` и `list-filter`.
pub fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::None | Value::Bool(false))
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
        Value::Float(f) => float_to_string(*f),
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
        Value::Tuple(items) => {
            let inner = items
                .iter()
                .map(|item| value_to_string_inner(item, true))
                .collect::<Vec<String>>()
                .join(" ");
            format!("(tuple {})", inner)
        }
        Value::Map(pairs) => {
            let inner = pairs
                .iter()
                .map(|(key, value)| {
                    let key = value_to_string_inner(key, true);
                    let value = value_to_string_inner(value, true);
                    format!("(tuple {} {})", key, value)
                })
                .collect::<Vec<String>>()
                .join(" ");
            format!("(map {})", inner)
        }
        Value::Struct { name, fields } => {
            let inner = fields
                .iter()
                .map(|item| value_to_string_inner(item, true))
                .collect::<Vec<String>>()
                .join(" ");
            format!("({} {})", name, inner)
        }
        Value::Socket(_) => "socket".to_string(),
        Value::Listener(_) => "listener".to_string(),
        Value::Some(v) => format!("(some {})", value_to_string_inner(v, true)),
        Value::Success(v) => format!("(success {})", value_to_string_inner(v, true)),
        Value::Failure(v) => format!("(failure {})", value_to_string_inner(v, true)),
        Value::Closure(_) => "function".to_string(),
        Value::Native { .. } => "function".to_string(),
    }
}

/// Детерминированная политика представления f64 в строку.
///
/// Используется Rust `Display`. Если полученная строка не содержит точки
/// (то есть значение — конечное «целое» float, например `4.0`), добавляется
/// суффикс `.0`, чтобы результат отличался от целочисленного литерала:
/// `4.0 → "4.0"`, `3.14 → "3.14"`. Для `inf`/`-inf`/`nan` хвост `.0` не
/// добавляется.
pub fn float_to_string(f: f64) -> String {
    let s = f.to_string();
    if f.is_finite() && !s.contains('.') {
        format!("{}.0", s)
    } else {
        s
    }
}
