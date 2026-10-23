//! Арифметические встроенные функции: `+`, `-`, `*`, `/`, `%`, `pow`,
//! `min`, `max`, `abs`, а также конверсии `int->float`, `float->int`.

use crate::error::Error;
use crate::value::Value;

/// Числовое значение: целое или float.
#[derive(Clone, Copy)]
enum Num {
    Int(i64),
    Float(f64),
}

/// Извлекает целое число из значения, иначе — ошибка.
fn as_int(value: &Value, op: &str) -> Result<i64, Error> {
    match value {
        Value::Int(i) => Ok(*i),
        _ => Err(Error::new(format!("{} требует целых чисел", op))),
    }
}

/// Извлекает числовое значение (int или float) из аргумента.
fn as_num(value: &Value, op: &str) -> Result<Num, Error> {
    match value {
        Value::Int(i) => Ok(Num::Int(*i)),
        Value::Float(f) => Ok(Num::Float(*f)),
        _ => Err(Error::new(format!("{} требует чисел", op))),
    }
}

/// Проверяет, что среди чисел нет одновременного int и float
/// (смешивание типов — runtime-ошибка по спеке V0.4).
fn check_no_mix(nums: &[Num], op: &str) -> Result<(), Error> {
    let has_int = nums.iter().any(|n| matches!(n, Num::Int(_)));
    let has_float = nums.iter().any(|n| matches!(n, Num::Float(_)));
    if has_int && has_float {
        return Err(Error::new(format!("{}: нельзя смешивать int и float", op)));
    }
    Ok(())
}

/// Сложение двух и более чисел.
///
/// int+int → int; float+float → float; смешивание — ошибка.
pub fn add(args: &[Value]) -> Result<Value, Error> {
    if args.len() < 2 {
        return Err(Error::new(format!(
            "(+ ...) требует минимум 2 аргумента, получено {}",
            args.len()
        )));
    }
    let nums: Vec<Num> = args
        .iter()
        .map(|a| as_num(a, "+"))
        .collect::<Result<Vec<_>, _>>()?;
    check_no_mix(&nums, "+")?;

    if let Some(Num::Float(_)) = nums.first() {
        let mut result = 0.0f64;
        for n in &nums {
            if let Num::Float(f) = n {
                result += f;
            }
        }
        return Ok(Value::Float(result));
    }

    let mut result = 0i64;
    for n in &nums {
        if let Num::Int(i) = n {
            result = result
                .checked_add(*i)
                .ok_or_else(|| Error::runtime("integer-overflow", "Переполнение при сложении"))?;
        }
    }
    Ok(Value::Int(result))
}

/// Вычитание: унарный минус (1 аргумент) или разность (2+ аргументов).
pub fn sub(args: &[Value]) -> Result<Value, Error> {
    if args.is_empty() {
        return Err(Error::new("(-) требует минимум 1 аргумент, получено 0"));
    }

    if args.len() == 1 {
        return match &args[0] {
            Value::Int(v) => v.checked_neg().map(Value::Int).ok_or_else(|| {
                Error::runtime("integer-overflow", "Переполнение при унарном минусе")
            }),
            Value::Float(v) => Ok(Value::Float(-v)),
            _ => Err(Error::new("- требует чисел")),
        };
    }

    let nums: Vec<Num> = args
        .iter()
        .map(|a| as_num(a, "-"))
        .collect::<Result<Vec<_>, _>>()?;
    check_no_mix(&nums, "-")?;

    if let Some(Num::Float(_)) = nums.first() {
        let mut result = 0.0f64;
        if let Num::Float(f) = nums[0] {
            result = f;
        }
        for n in &nums[1..] {
            if let Num::Float(f) = n {
                result -= f;
            }
        }
        return Ok(Value::Float(result));
    }

    let mut result = 0i64;
    if let Num::Int(i) = nums[0] {
        result = i;
    }
    for n in &nums[1..] {
        if let Num::Int(i) = n {
            result = result
                .checked_sub(*i)
                .ok_or_else(|| Error::runtime("integer-overflow", "Переполнение при вычитании"))?;
        }
    }
    Ok(Value::Int(result))
}

/// Умножение двух и более чисел.
pub fn mul(args: &[Value]) -> Result<Value, Error> {
    if args.len() < 2 {
        return Err(Error::new(format!(
            "(* ...) требует минимум 2 аргумента, получено {}",
            args.len()
        )));
    }
    let nums: Vec<Num> = args
        .iter()
        .map(|a| as_num(a, "*"))
        .collect::<Result<Vec<_>, _>>()?;
    check_no_mix(&nums, "*")?;

    if let Some(Num::Float(_)) = nums.first() {
        let mut result = 1.0f64;
        for n in &nums {
            if let Num::Float(f) = n {
                result *= f;
            }
        }
        return Ok(Value::Float(result));
    }

    let mut result = 1i64;
    for n in &nums {
        if let Num::Int(i) = n {
            result = result
                .checked_mul(*i)
                .ok_or_else(|| Error::runtime("integer-overflow", "Переполнение при умножении"))?;
        }
    }
    Ok(Value::Int(result))
}

/// Деление (ровно два аргумента).
///
/// int / int — целочисленное деление; деление int на ноль — ошибка.
/// float / float — IEEE-деление: `/ 0.0 → inf`, на `-0.0 → -inf` (НЕ ошибка).
/// Смешивание int и float — ошибка.
pub fn div(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::new(format!(
            "(/ ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }
    let a = as_num(&args[0], "/")?;
    let b = as_num(&args[1], "/")?;
    check_no_mix(&[a, b], "/")?;

    match (a, b) {
        (Num::Float(x), Num::Float(y)) => Ok(Value::Float(x / y)),
        (Num::Int(x), Num::Int(y)) => {
            if y == 0 {
                return Err(Error::runtime("divide-by-zero", "Деление на ноль"));
            }
            x.checked_div(y)
                .map(Value::Int)
                .ok_or_else(|| Error::runtime("integer-overflow", "Переполнение при делении"))
        }
        (Num::Int(_), Num::Float(_)) | (Num::Float(_), Num::Int(_)) => {
            unreachable!("смешивание уже отброшено check_no_mix")
        }
    }
}

/// Остаток от деления (ровно два аргумента). Работает только для int;
/// float `%` — ошибка.
pub fn rem(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::new(format!(
            "(% ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }
    if matches!(args[0], Value::Float(_)) || matches!(args[1], Value::Float(_)) {
        return Err(Error::new("% не поддерживается для float"));
    }
    let dividend = as_int(&args[0], "%")?;
    let divisor = as_int(&args[1], "%")?;
    if divisor == 0 {
        return Err(Error::runtime("divide-by-zero", "Деление на ноль"));
    }
    dividend
        .checked_rem(divisor)
        .map(Value::Int)
        .ok_or_else(|| Error::runtime("integer-overflow", "Переполнение при вычислении остатка"))
}

/// Возведение в степень `(pow основание показатель)`.
///
/// Показатель должен быть неотрицательным. Работает только с целыми числами.
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
    let result = base.checked_pow(exp as u32).ok_or_else(|| {
        Error::runtime("integer-overflow", "Переполнение при возведении в степень")
    })?;
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
    a.checked_abs()
        .map(Value::Int)
        .ok_or_else(|| Error::runtime("integer-overflow", "Переполнение при вычислении abs"))
}

/// Конверсия int → float `(int->float n)`.
///
/// # Ошибки
/// — не ровно один аргумент;
/// — аргумент не является целым числом.
pub fn int_to_float(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::new(format!(
            "(int->float ...) требует ровно 1 аргумент, получено {}",
            args.len()
        )));
    }
    match &args[0] {
        Value::Int(i) => Ok(Value::Float(*i as f64)),
        _ => Err(Error::new("int->float требует целого числа")),
    }
}

/// Конверсия float → int `(float->int x)` — усечение к нулю
/// (3.7 → 3, -3.7 → -3).
///
/// # Ошибки
/// — не ровно один аргумент;
/// — аргумент не является float.
pub fn float_to_int(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::new(format!(
            "(float->int ...) требует ровно 1 аргумент, получено {}",
            args.len()
        )));
    }
    match &args[0] {
        Value::Float(f) => Ok(Value::Int(*f as i64)),
        _ => Err(Error::new("float->int требует float")),
    }
}
