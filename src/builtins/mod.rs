//! Встроенные функции языка Lupus.
//!
//! Каждый подмодуль реализует группу встроенных операций. Все функции
//! принимают уже вычисленные аргументы (`&[Value]`) и возвращают
//! `Result<Value, Error>`. Арность проверяется централизованно по таблице
//! [`BUILTINS`].

pub mod arithmetic;
pub mod comparison;
pub mod higher_order;
pub mod lists;
pub mod logic;
pub mod map;
pub mod strings;

pub use arithmetic::{abs, add, div, float_to_int, int_to_float, max, min, mul, pow, rem, sub};
pub use comparison::{eq, ge, gt, le, lt, neq};
pub use lists::{contains, length, nth};
pub use logic::not;
pub use strings::{
    float_to_str, int_to_str, print_arg, str_to_float, str_to_int, string_append, string_length,
    string_reverse, string_split,
};

use crate::error::Error;
use crate::interpreter::Interpreter;
use crate::value::{Arity, BuiltinImpl, NativeImpl, Value};

/// Описание встроенной функции ядра.
#[derive(Clone, Copy)]
pub struct Builtin {
    /// Имя в языке Lupus.
    pub name: &'static str,
    /// Допустимая арность.
    pub arity: Arity,
    /// Реализация.
    pub func: BuiltinImpl,
}

macro_rules! builtin {
    ($name:literal, $arity:expr, $func:expr) => {
        Builtin {
            name: $name,
            arity: $arity,
            func: $func,
        }
    };
}

impl Builtin {
    /// Представляет встроенную функцию как значение первого класса.
    pub fn value(&self) -> Value {
        Value::Native {
            name: self.name.to_string(),
            arity: self.arity,
            func: NativeImpl::Builtin(self.func),
        }
    }
}

/// Статическая таблица встроенных функций, отсортированная по имени.
///
/// Поиск — бинарный (`lookup`). Специальные формы сюда не входят.
pub const BUILTINS: &[Builtin] = &[
    builtin!("!=", Arity::Exact(2), BuiltinImpl::Pure(comparison::neq)),
    builtin!("%", Arity::Exact(2), BuiltinImpl::Pure(arithmetic::rem)),
    builtin!("*", Arity::AtLeast(2), BuiltinImpl::Pure(arithmetic::mul)),
    builtin!("+", Arity::AtLeast(2), BuiltinImpl::Pure(arithmetic::add)),
    builtin!("-", Arity::AtLeast(1), BuiltinImpl::Pure(arithmetic::sub)),
    builtin!("/", Arity::Exact(2), BuiltinImpl::Pure(arithmetic::div)),
    builtin!("<", Arity::Exact(2), BuiltinImpl::Pure(comparison::lt)),
    builtin!("<=", Arity::Exact(2), BuiltinImpl::Pure(comparison::le)),
    builtin!("=", Arity::Exact(2), BuiltinImpl::Pure(comparison::eq)),
    builtin!(">", Arity::Exact(2), BuiltinImpl::Pure(comparison::gt)),
    builtin!(">=", Arity::Exact(2), BuiltinImpl::Pure(comparison::ge)),
    builtin!("abs", Arity::Exact(1), BuiltinImpl::Pure(arithmetic::abs)),
    builtin!("append", Arity::Exact(2), BuiltinImpl::Pure(lists::append)),
    builtin!("cons", Arity::Exact(2), BuiltinImpl::Pure(lists::cons)),
    builtin!(
        "contains?",
        Arity::Exact(2),
        BuiltinImpl::Pure(lists::contains)
    ),
    builtin!(
        "empty?",
        Arity::Exact(1),
        BuiltinImpl::Pure(lists::is_empty)
    ),
    builtin!("failure", Arity::Exact(1), BuiltinImpl::Pure(failure)),
    builtin!(
        "float->int",
        Arity::Exact(1),
        BuiltinImpl::Pure(arithmetic::float_to_int)
    ),
    builtin!(
        "float->str",
        Arity::Exact(1),
        BuiltinImpl::Pure(strings::float_to_str)
    ),
    builtin!("head", Arity::Exact(1), BuiltinImpl::Pure(lists::head)),
    builtin!(
        "int->float",
        Arity::Exact(1),
        BuiltinImpl::Pure(arithmetic::int_to_float)
    ),
    builtin!(
        "int->str",
        Arity::Exact(1),
        BuiltinImpl::Pure(strings::int_to_str)
    ),
    builtin!("len", Arity::Exact(1), BuiltinImpl::Pure(lists::length)),
    builtin!("length", Arity::Exact(1), BuiltinImpl::Pure(lists::length)),
    builtin!("list", Arity::Any, BuiltinImpl::Pure(lists::list)),
    builtin!(
        "list-filter",
        Arity::Exact(2),
        BuiltinImpl::Context(higher_order::filter)
    ),
    builtin!(
        "list-fold",
        Arity::Exact(3),
        BuiltinImpl::Context(higher_order::fold)
    ),
    builtin!(
        "list-map",
        Arity::Exact(2),
        BuiltinImpl::Context(higher_order::map)
    ),
    builtin!("map", Arity::Any, BuiltinImpl::Pure(map::make_map)),
    builtin!("map-get", Arity::Exact(2), BuiltinImpl::Pure(map::map_get)),
    builtin!("map-has?", Arity::Exact(2), BuiltinImpl::Pure(map::map_has)),
    builtin!(
        "map-keys",
        Arity::Exact(1),
        BuiltinImpl::Pure(map::map_keys)
    ),
    builtin!("map-set", Arity::Exact(3), BuiltinImpl::Pure(map::map_set)),
    builtin!(
        "map-values",
        Arity::Exact(1),
        BuiltinImpl::Pure(map::map_values)
    ),
    builtin!("max", Arity::Exact(2), BuiltinImpl::Pure(arithmetic::max)),
    builtin!("min", Arity::Exact(2), BuiltinImpl::Pure(arithmetic::min)),
    builtin!("mod", Arity::Exact(2), BuiltinImpl::Pure(arithmetic::rem)),
    builtin!("not", Arity::Exact(1), BuiltinImpl::Pure(logic::not)),
    builtin!("nth", Arity::Exact(2), BuiltinImpl::Pure(lists::nth)),
    builtin!("pow", Arity::Exact(2), BuiltinImpl::Pure(arithmetic::pow)),
    builtin!("print", Arity::Exact(1), BuiltinImpl::Context(print_value)),
    builtin!("some", Arity::Exact(1), BuiltinImpl::Pure(some)),
    builtin!(
        "str->float",
        Arity::Exact(1),
        BuiltinImpl::Pure(strings::str_to_float)
    ),
    builtin!(
        "str->int",
        Arity::Exact(1),
        BuiltinImpl::Pure(strings::str_to_int)
    ),
    builtin!(
        "string-append",
        Arity::Any,
        BuiltinImpl::Pure(strings::string_append)
    ),
    builtin!(
        "string-length",
        Arity::Exact(1),
        BuiltinImpl::Pure(strings::string_length)
    ),
    builtin!(
        "string-reverse",
        Arity::Exact(1),
        BuiltinImpl::Pure(strings::string_reverse)
    ),
    builtin!(
        "string-split",
        Arity::Exact(2),
        BuiltinImpl::Pure(strings::string_split)
    ),
    builtin!("success", Arity::Exact(1), BuiltinImpl::Pure(success)),
    builtin!("tail", Arity::Exact(1), BuiltinImpl::Pure(lists::tail)),
    builtin!("tuple", Arity::Any, BuiltinImpl::Pure(tuple)),
];

/// Ищет встроенную функцию по имени.
#[inline]
pub fn lookup(name: &str) -> Option<&'static Builtin> {
    let byte = *name.as_bytes().first()? as usize;
    if byte >= FIRST_BYTE.len() - 1 {
        return None;
    }
    let start = FIRST_BYTE[byte] as usize;
    let end = FIRST_BYTE[byte + 1] as usize;
    BUILTINS[start..end]
        .iter()
        .find(|candidate| candidate.name == name)
}

/// Начало диапазона [`BUILTINS`] для каждого первого байта имени.
///
/// Индекс 128 — sentinel, равный длине таблицы. Благодаря сортировке имена
/// с одинаковым первым байтом лежат подряд.
const FIRST_BYTE: [u8; 129] = build_first_byte_index();

const fn build_first_byte_index() -> [u8; 129] {
    let mut table = [0u8; 129];
    let mut builtin = 0usize;
    let mut byte = 0usize;
    while byte < 128 {
        while builtin < BUILTINS.len() && (BUILTINS[builtin].name.as_bytes()[0] as usize) < byte {
            builtin += 1;
        }
        table[byte] = builtin as u8;
        byte += 1;
    }
    table[128] = BUILTINS.len() as u8;
    table
}

fn some(args: &[Value]) -> Result<Value, Error> {
    Ok(Value::Some(Box::new(args[0].clone())))
}

fn success(args: &[Value]) -> Result<Value, Error> {
    Ok(Value::Success(Box::new(args[0].clone())))
}

fn failure(args: &[Value]) -> Result<Value, Error> {
    Ok(Value::Failure(Box::new(args[0].clone())))
}

fn tuple(args: &[Value]) -> Result<Value, Error> {
    Ok(Value::Tuple(args.to_vec()))
}

fn print_value(interpreter: &Interpreter, args: &[Value]) -> Result<Value, Error> {
    interpreter.print_value(&args[0])?;
    Ok(Value::None)
}

/// Имена, занятые ядром языка (операторы, специальные формы и встроенные
/// функции). Используются линтом `core-shadowing` и проверкой `:all`.
pub const CORE_NAMES: &[&str] = &[
    "define",
    "define-public",
    "define-const",
    "define-mutable",
    "set!",
    "lambda",
    "if",
    "cond",
    "else",
    "match",
    "if-let",
    "while",
    "for",
    "in",
    "loop",
    "exit",
    "begin",
    "and",
    "or",
    "not",
    "assert",
    "test",
    "import",
    "defstruct",
    "#lupus",
    "print",
    "+",
    "-",
    "*",
    "/",
    "%",
    "mod",
    "=",
    "!=",
    "<",
    ">",
    "<=",
    ">=",
    "pow",
    "min",
    "max",
    "abs",
    "int->str",
    "float->str",
    "int->float",
    "float->int",
    "str->int",
    "str->float",
    "string-append",
    "string-length",
    "string-split",
    "string-reverse",
    "list",
    "tuple",
    "map",
    "some",
    "success",
    "failure",
    "cons",
    "head",
    "tail",
    "empty?",
    "append",
    "list-map",
    "list-filter",
    "list-fold",
    "nth",
    "length",
    "len",
    "contains?",
    "range",
    "let",
    "map-get",
    "map-set",
    "map-keys",
    "map-values",
    "map-has?",
];

/// Проверяет, занято ли имя ядром языка.
pub fn is_core_name(name: &str) -> bool {
    CORE_NAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_table_is_sorted_and_registered() {
        for pair in BUILTINS.windows(2) {
            assert!(
                pair[0].name < pair[1].name,
                "таблица BUILTINS не отсортирована: {} → {}",
                pair[0].name,
                pair[1].name
            );
        }
        for builtin in BUILTINS {
            assert!(is_core_name(builtin.name), "{}", builtin.name);
            assert_eq!(
                lookup(builtin.name).map(|found| found.name),
                Some(builtin.name)
            );
        }
    }
}
