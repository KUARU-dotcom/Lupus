//! Встроенные функции языка Lupus.
//!
//! Каждый подмодуль реализует группу встроенных операций. Все функции
//! принимают уже вычисленные аргументы (`&[Value]`) и возвращают
//! `Result<Value, Error>`.

pub mod arithmetic;
pub mod comparison;
pub mod lists;
pub mod logic;
pub mod map;
pub mod strings;

pub use arithmetic::{abs, add, div, float_to_int, int_to_float, max, min, mul, pow, rem, sub};
pub use comparison::{eq, ge, gt, le, lt, neq};
pub use lists::{contains, length, make_list, nth};
pub use logic::not;
pub use strings::{
    float_to_str, int_to_str, print_arg, str_to_float, str_to_int, string_append, string_length,
    string_reverse, string_split,
};

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
