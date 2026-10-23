//! Встроенные функции `Map` (V0.5).
//!
//! Map — неизменяемый ассоциативный массив с сохранением порядка вставки.
//! Ключом может быть любое значение, кроме `Float`-NaN; сравнение ключей
//! совпадает с оператором `=`.

use crate::error::Error;
use crate::value::Value;

/// Проверяет допустимость ключа Map.
fn validate_key(key: &Value, operation: &str) -> Result<(), Error> {
    if matches!(key, Value::Float(value) if value.is_nan()) {
        return Err(Error::type_mismatch(format!(
            "{}: ключ Map не может быть NaN",
            operation
        )));
    }
    Ok(())
}

/// Извлекает пары Map из значения.
fn as_map<'a>(value: &'a Value, operation: &str) -> Result<&'a [(Value, Value)], Error> {
    match value {
        Value::Map(pairs) => Ok(pairs),
        _ => Err(Error::type_mismatch(format!(
            "{} ожидает Map, получено {}",
            operation,
            crate::value::value_to_string(value)
        ))),
    }
}

/// (map (tuple ключ значение) ...)
pub fn make_map(args: &[Value]) -> Result<Value, Error> {
    let mut pairs: Vec<(Value, Value)> = Vec::with_capacity(args.len());
    for item in args {
        let Value::Tuple(elements) = item else {
            return Err(Error::type_mismatch(
                "Конструктор map ожидает пары (tuple ключ значение)",
            ));
        };
        if elements.len() != 2 {
            return Err(Error::arity(
                "Пара map должна быть (tuple ключ значение) — ровно 2 элемента",
            ));
        }
        let key = &elements[0];
        validate_key(key, "map")?;
        if let Some(existing) = pairs.iter_mut().find(|(other, _)| other == key) {
            existing.1 = elements[1].clone();
        } else {
            pairs.push((key.clone(), elements[1].clone()));
        }
    }
    Ok(Value::Map(pairs))
}

/// (map-get map ключ) → (some значение) | none
pub fn map_get(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity(format!(
            "(map-get ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }
    let pairs = as_map(&args[0], "map-get")?;
    validate_key(&args[1], "map-get")?;
    match pairs.iter().find(|(key, _)| *key == args[1]) {
        Some((_, value)) => Ok(Value::Some(Box::new(value.clone()))),
        None => Ok(Value::None),
    }
}

/// (map-set map ключ значение) → новый Map (неизменяемость)
pub fn map_set(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 3 {
        return Err(Error::arity(format!(
            "(map-set ...) требует ровно 3 аргумента, получено {}",
            args.len()
        )));
    }
    let pairs = as_map(&args[0], "map-set")?;
    validate_key(&args[1], "map-set")?;

    let mut result = pairs.to_vec();
    if let Some(existing) = result.iter_mut().find(|(key, _)| *key == args[1]) {
        existing.1 = args[2].clone();
    } else {
        result.push((args[1].clone(), args[2].clone()));
    }
    Ok(Value::Map(result))
}

/// (map-keys map) → (List ключей) в порядке вставки
pub fn map_keys(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity(format!(
            "(map-keys ...) требует ровно 1 аргумент, получено {}",
            args.len()
        )));
    }
    let pairs = as_map(&args[0], "map-keys")?;
    Ok(Value::List(
        pairs.iter().map(|(key, _)| key.clone()).collect(),
    ))
}

/// (map-values map) → (List значений) в порядке вставки
pub fn map_values(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::arity(format!(
            "(map-values ...) требует ровно 1 аргумент, получено {}",
            args.len()
        )));
    }
    let pairs = as_map(&args[0], "map-values")?;
    Ok(Value::List(
        pairs.iter().map(|(_, value)| value.clone()).collect(),
    ))
}

/// (map-has? map ключ) → bool
pub fn map_has(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::arity(format!(
            "(map-has? ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }
    let pairs = as_map(&args[0], "map-has?")?;
    validate_key(&args[1], "map-has?")?;
    Ok(Value::Bool(pairs.iter().any(|(key, _)| *key == args[1])))
}
