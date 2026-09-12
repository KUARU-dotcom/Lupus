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

/// Числовое значение: целое или float.
#[derive(Clone, Copy)]
enum Num {
    Int(i64),
    Float(f64),
}

/// Числовое сравнение с проверкой, что оба аргумента — числа одного типа
/// (смешивание int и float — runtime-ошибка по спеке V0.4).
fn compare_nums(op: &str, args: &[Value]) -> Result<(Num, Num), Error> {
    require_two(op, args)?;
    let a = match &args[0] {
        Value::Int(i) => Num::Int(*i),
        Value::Float(f) => Num::Float(*f),
        _ => return Err(Error::new(format!("{} требует чисел", op))),
    };
    let b = match &args[1] {
        Value::Int(i) => Num::Int(*i),
        Value::Float(f) => Num::Float(*f),
        _ => return Err(Error::new(format!("{} требует чисел", op))),
    };
    if matches!(a, Num::Int(_)) != matches!(b, Num::Int(_)) {
        return Err(Error::new(format!("{}: нельзя смешивать int и float", op)));
    }
    Ok((a, b))
}

/// Равенство `=` (работает для чисел, строк, булевых значений и списков).
///
/// `(= 1 1.0)` → `false` (int не равен float, без ошибки).
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
    let (a, b) = compare_nums("<", args)?;
    match (a, b) {
        (Num::Int(x), Num::Int(y)) => Ok(Value::Bool(x < y)),
        (Num::Float(x), Num::Float(y)) => Ok(Value::Bool(x < y)),
        _ => unreachable!("типы уже проверены"),
    }
}

/// Больше `>`.
pub fn gt(args: &[Value]) -> Result<Value, Error> {
    let (a, b) = compare_nums(">", args)?;
    match (a, b) {
        (Num::Int(x), Num::Int(y)) => Ok(Value::Bool(x > y)),
        (Num::Float(x), Num::Float(y)) => Ok(Value::Bool(x > y)),
        _ => unreachable!("типы уже проверены"),
    }
}

/// Меньше или равно `<=`.
pub fn le(args: &[Value]) -> Result<Value, Error> {
    let (a, b) = compare_nums("<=", args)?;
    match (a, b) {
        (Num::Int(x), Num::Int(y)) => Ok(Value::Bool(x <= y)),
        (Num::Float(x), Num::Float(y)) => Ok(Value::Bool(x <= y)),
        _ => unreachable!("типы уже проверены"),
    }
}

/// Больше или равно `>=`.
pub fn ge(args: &[Value]) -> Result<Value, Error> {
    let (a, b) = compare_nums(">=", args)?;
    match (a, b) {
        (Num::Int(x), Num::Int(y)) => Ok(Value::Bool(x >= y)),
        (Num::Float(x), Num::Float(y)) => Ok(Value::Bool(x >= y)),
        _ => unreachable!("типы уже проверены"),
    }
}
