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

/// Возведение в степень `(pow основание показатель)`.
///
/// Показатель должен быть неотрицательным.
///
/// # Ошибки
/// — не ровно два аргумента;
/// — аргумент не является целым числом;
/// — отрицательный показатель;
/// — переполнение результата.
pub fn pow(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::new(format!(
            "(pow ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }
    let base = as_int(&args[0], "pow")?;
    let exp = as_int(&args[1], "pow")?;
    if exp < 0 {
        return Err(Error::new(
            "pow: показатель степени не может быть отрицательным",
        ));
    }
    let result = base
        .checked_pow(exp as u32)
        .ok_or_else(|| Error::new("Переполнение при возведении в степень"))?;
    Ok(Value::Int(result))
}

/// Минимум двух целых чисел `(min a b)`.
///
/// # Ошибки
/// — не ровно два аргумента;
/// — аргумент не является целым числом.
pub fn min(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::new(format!(
            "(min ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }
    let a = as_int(&args[0], "min")?;
    let b = as_int(&args[1], "min")?;
    Ok(Value::Int(a.min(b)))
}

/// Максимум двух целых чисел `(max a b)`.
///
/// # Ошибки
/// — не ровно два аргумента;
/// — аргумент не является целым числом.
pub fn max(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::new(format!(
            "(max ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }
    let a = as_int(&args[0], "max")?;
    let b = as_int(&args[1], "max")?;
    Ok(Value::Int(a.max(b)))
}

/// Абсолютная величина целого числа `(abs число)`.
///
/// # Ошибки
/// — не ровно один аргумент;
/// — аргумент не является целым числом.
pub fn abs(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::new(format!(
            "(abs ...) требует ровно 1 аргумент, получено {}",
            args.len()
        )));
    }
    let a = as_int(&args[0], "abs")?;
    Ok(Value::Int(a.abs()))
}
