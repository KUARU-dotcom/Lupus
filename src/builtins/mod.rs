//! Встроенные функции языка Lupus.
//!
//! Каждый подмодуль реализует группу встроенных операций. Все функции
//! принимают уже вычисленные аргументы (`&[Value]`) и возвращают
//! `Result<Value, Error>`.

pub mod arithmetic;
pub mod comparison;
pub mod lists;
pub mod logic;
pub mod strings;

pub use arithmetic::{add, div, mul, rem, sub};
pub use comparison::{eq, ge, gt, le, lt, neq};
pub use lists::{contains, length, make_list, nth};
pub use logic::not;
pub use strings::{int_to_str, print_arg, string_append};
