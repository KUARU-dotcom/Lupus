//! Встроенные функции строк и вывода: `print`, `int->str`, `string-append`,
//! `string-length`, `string-split`, `string-reverse`, `str->int`.

use crate::error::Error;
use crate::value::{value_to_string, Value};

/// Печатает значение с переводом строки `(print значение)`.
///
/// В отличие от предыдущей версии, принимает **любое** значение
/// (не только строку), преобразуя его через [`value_to_string`].
pub fn print_arg(value: &Value) -> Result<(), Error> {
    println!("{}", value_to_string(value));
    Ok(())
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

/// Возвращает длину строки в Unicode code points `(string-length строка)`.
///
/// # Ошибки
/// — не ровно один аргумент;
/// — аргумент не является строкой.
pub fn string_length(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::new(format!(
            "(string-length ...) требует ровно 1 аргумент, получено {}",
            args.len()
        )));
    }
    match &args[0] {
        Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
        _ => Err(Error::new("string-length требует строку")),
    }
}

/// Разбивает строку по разделителю `(string-split строка разделитель)`.
///
/// Возвращает список подстрок. Если разделитель не найден — список с
/// одной исходной строкой.
///
/// # Ошибки
/// — не ровно два аргумента;
/// — любой аргумент не является строкой.
pub fn string_split(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::new(format!(
            "(string-split ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }
    let s = match &args[0] {
        Value::Str(s) => s.clone(),
        _ => {
            return Err(Error::new(
                "Первый аргумент string-split должен быть строкой",
            ))
        }
    };
    let sep = match &args[1] {
        Value::Str(sep) => sep.clone(),
        _ => {
            return Err(Error::new(
                "Второй аргумент string-split должен быть строкой",
            ))
        }
    };
    let parts: Vec<Value> = if sep.is_empty() {
        s.chars().map(|c| Value::Str(c.to_string())).collect()
    } else {
        s.split(&sep).map(|p| Value::Str(p.to_string())).collect()
    };
    Ok(Value::List(parts))
}

/// Переворачивает строку по code points `(string-reverse строка)`.
///
/// # Ошибки
/// — не ровно один аргумент;
/// — аргумент не является строкой.
pub fn string_reverse(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::new(format!(
            "(string-reverse ...) требует ровно 1 аргумент, получено {}",
            args.len()
        )));
    }
    match &args[0] {
        Value::Str(s) => Ok(Value::Str(s.chars().rev().collect())),
        _ => Err(Error::new("string-reverse требует строку")),
    }
}

/// Парсит строку в целое число `(str->int строка)`.
///
/// Возвращает `(some n)` при успехе, `none` при ошибке парсинга.
///
/// # Ошибки
/// — не ровно один аргумент;
/// — аргумент не является строкой.
pub fn str_to_int(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::new(format!(
            "(str->int ...) требует ровно 1 аргумент, получено {}",
            args.len()
        )));
    }
    match &args[0] {
        Value::Str(s) => match s.trim().parse::<i64>() {
            Ok(n) => Ok(Value::Some(Box::new(Value::Int(n)))),
            Err(_) => Ok(Value::None),
        },
        _ => Err(Error::new("str->int требует строку")),
    }
}
