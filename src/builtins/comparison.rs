//! Встроенные функции сравнения: `=`, `!=`, `<`, `>`, `<=`, `>=`.

use crate::error::Error;
use crate::value::Value;

/// Проверяет, что передано ровно два аргумента.
fn require_two(op: &str, args: &[Value]) -> Result<(), Error> {
    if args.len() != 2 {
        return Err(Error::new(format!(
            "({} ...) требует ровно 2 аргумента, получено {}",
            op,
            args.len()
        )));
    }
    Ok(())
}

/// Числовое сравнение с проверкой целочисленности обоих аргументов.
fn compare_ints(op: &str, args: &[Value]) -> Result<(i64, i64), Error> {
    require_two(op, args)?;
    let a = match &args[0] {
        Value::Int(i) => *i,
        _ => return Err(Error::new(format!("{} требует целых чисел", op))),
    };
    let b = match &args[1] {
        Value::Int(i) => *i,
        _ => return Err(Error::new(format!("{} требует целых чисел", op))),
    };
    Ok((a, b))
}

/// Равенство `=` (работает для чисел, строк, булевых значений и списков).
pub fn eq(args: &[Value]) -> Result<Value, Error> {
    require_two("=", args)?;
    Ok(Value::Bool(args[0] == args[1]))
}

/// Неравенство `!=`.
pub fn neq(args: &[Value]) -> Result<Value, Error> {
    require_two("!=", args)?;
    Ok(Value::Bool(args[0] != args[1]))
}

/// Меньше `<`.
pub fn lt(args: &[Value]) -> Result<Value, Error> {
    let (a, b) = compare_ints("<", args)?;
    Ok(Value::Bool(a < b))
}

/// Больше `>`.
pub fn gt(args: &[Value]) -> Result<Value, Error> {
    let (a, b) = compare_ints(">", args)?;
    Ok(Value::Bool(a > b))
}

/// Меньше или равно `<=`.
pub fn le(args: &[Value]) -> Result<Value, Error> {
    let (a, b) = compare_ints("<=", args)?;
    Ok(Value::Bool(a <= b))
}

/// Больше или равно `>=`.
pub fn ge(args: &[Value]) -> Result<Value, Error> {
    let (a, b) = compare_ints(">=", args)?;
    Ok(Value::Bool(a >= b))
}
