//! Арифметические встроенные функции: `+`, `-`, `*`, `/`, `%`.

use crate::error::Error;
use crate::value::Value;

/// Извлекает целое число из значения, иначе — ошибка.
fn as_int(value: &Value, op: &str) -> Result<i64, Error> {
    match value {
        Value::Int(i) => Ok(*i),
        _ => Err(Error::new(format!("{} требует целых чисел", op))),
    }
}

/// Сложение двух и более целых чисел.
///
/// # Ошибки
/// — меньше двух аргументов;
/// — аргумент не является целым числом.
pub fn add(args: &[Value]) -> Result<Value, Error> {
    if args.len() < 2 {
        return Err(Error::new(format!(
            "(+ ...) требует минимум 2 аргумента, получено {}",
            args.len()
        )));
    }
    let mut result = 0i64;
    for arg in args {
        result = result
            .checked_add(as_int(arg, "+")?)
            .ok_or_else(|| Error::new("Переполнение при сложении"))?;
    }
    Ok(Value::Int(result))
}

/// Вычитание: унарный минус (1 аргумент) или разность (2+ аргументов).
///
/// # Ошибки
/// — ноль аргументов;
/// — аргумент не является целым числом.
pub fn sub(args: &[Value]) -> Result<Value, Error> {
    if args.is_empty() {
        return Err(Error::new("(-) требует минимум 1 аргумент, получено 0"));
    }

    if args.len() == 1 {
        let value = as_int(&args[0], "-")?;
        return Ok(Value::Int(-value));
    }

    let mut result = as_int(&args[0], "-")?;
    for arg in &args[1..] {
        result = result
            .checked_sub(as_int(arg, "-")?)
            .ok_or_else(|| Error::new("Переполнение при вычитании"))?;
    }
    Ok(Value::Int(result))
}

/// Умножение двух и более целых чисел.
///
/// # Ошибки
/// — меньше двух аргументов;
/// — аргумент не является целым числом.
pub fn mul(args: &[Value]) -> Result<Value, Error> {
    if args.len() < 2 {
        return Err(Error::new(format!(
            "(* ...) требует минимум 2 аргумента, получено {}",
            args.len()
        )));
    }
    let mut result = 1i64;
    for arg in args {
        result = result
            .checked_mul(as_int(arg, "*")?)
            .ok_or_else(|| Error::new("Переполнение при умножении"))?;
    }
    Ok(Value::Int(result))
}

/// Целочисленное деление (ровно два аргумента). Деление на ноль — ошибка.
pub fn div(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::new(format!(
            "(/ ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }
    let dividend = as_int(&args[0], "/")?;
    let divisor = as_int(&args[1], "/")?;
    if divisor == 0 {
        return Err(Error::new("Деление на ноль"));
    }
    Ok(Value::Int(dividend / divisor))
}

/// Остаток от деления (ровно два аргумента). Деление на ноль — ошибка.
pub fn rem(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::new(format!(
            "(% ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }
    let dividend = as_int(&args[0], "%")?;
    let divisor = as_int(&args[1], "%")?;
    if divisor == 0 {
        return Err(Error::new("Деление на ноль"));
    }
    Ok(Value::Int(dividend % divisor))
}
