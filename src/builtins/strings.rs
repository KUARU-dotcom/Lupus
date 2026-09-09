//! Встроенные функции строк и вывода: `print`, `int->str`, `string-append`.

use crate::error::Error;
use crate::value::Value;

/// Проверяет аргумент `print` и печатает его с переводом строки.
///
/// Lupus-версия требует, чтобы `print` получил строку.
///
/// # Ошибки
/// — не ровно один аргумент;
/// — аргумент не является строкой.
pub fn print_arg(value: &Value) -> Result<(), Error> {
    match value {
        Value::Str(s) => {
            println!("{}", s);
            Ok(())
        }
        _ => Err(Error::new("print требует строку")),
    }
}

/// Преобразует целое число в строку `(int->str число)`.
///
/// # Ошибки
/// — не ровно один аргумент;
/// — аргумент не является целым числом.
pub fn int_to_str(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::new(format!(
            "(int->str ...) требует ровно 1 аргумент, получено {}",
            args.len()
        )));
    }
    match &args[0] {
        Value::Int(i) => Ok(Value::Str(i.to_string())),
        _ => Err(Error::new("int->str требует целого числа")),
    }
}

/// Конкатенация строк `(string-append строка...)`.
///
/// # Ошибки
/// — любой аргумент не является строкой.
pub fn string_append(args: &[Value]) -> Result<Value, Error> {
    let mut result = String::new();
    for arg in args {
        match arg {
            Value::Str(s) => result.push_str(s),
            _ => return Err(Error::new("string-append требует строковых аргументов")),
        }
    }
    Ok(Value::Str(result))
}
