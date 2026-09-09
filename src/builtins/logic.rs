//! Логические встроенные функции: `not`.
//!
//! Формы `and` и `or` являются специальными формами интерпретатора
//! (короткое замыкание) и реализованы прямо в `interpreter.rs`.

use crate::error::Error;
use crate::value::{is_truthy, Value};

/// Логическое отрицание `not` (ровно один аргумент).
///
/// Возвращает булево значение, обратное «истинности» аргумента
/// (см. [`is_truthy`]).
pub fn not(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::new(format!(
            "(not ...) требует ровно 1 аргумент, получено {}",
            args.len()
        )));
    }
    Ok(Value::Bool(!is_truthy(&args[0])))
}
