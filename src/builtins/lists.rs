//! Встроенные функции списков.
//!
//! Арность проверяется централизованно по таблице [`super::BUILTINS`].

use crate::error::Error;
use crate::value::Value;

/// Создаёт список `(list элем...)`.
pub fn list(args: &[Value]) -> Result<Value, Error> {
    Ok(Value::List(args.to_vec()))
}

/// Добавляет элемент в голову списка `(cons элемент список)`.
pub fn cons(args: &[Value]) -> Result<Value, Error> {
    let list = match &args[1] {
        Value::List(items) => items.clone(),
        _ => {
            return Err(Error::type_mismatch(
                "Второй аргумент cons должен быть списком",
            ))
        }
    };
    let mut result = vec![args[0].clone()];
    result.extend(list);
    Ok(Value::List(result))
}

/// Возвращает голову списка `(head список)` как `(some x)`.
///
/// Пустой список — `index-out-of-bounds`, не-список — `type-mismatch`.
pub fn head(args: &[Value]) -> Result<Value, Error> {
    match &args[0] {
        Value::List(items) if items.is_empty() => {
            Err(Error::runtime("index-out-of-bounds", "head: список пуст"))
        }
        Value::List(items) => Ok(Value::Some(Box::new(items[0].clone()))),
        _ => Err(Error::type_mismatch("head требует список")),
    }
}

/// Возвращает хвост списка `(tail список)` как `(some список)` или `none`
/// для списка из одного элемента.
///
/// Пустой список — `index-out-of-bounds`, не-список — `type-mismatch`.
pub fn tail(args: &[Value]) -> Result<Value, Error> {
    match &args[0] {
        Value::List(items) if items.is_empty() => {
            Err(Error::runtime("index-out-of-bounds", "tail: список пуст"))
        }
        Value::List(items) if items.len() > 1 => {
            Ok(Value::Some(Box::new(Value::List(items[1..].to_vec()))))
        }
        Value::List(_) => Ok(Value::None),
        _ => Err(Error::type_mismatch("tail требует список")),
    }
}

/// Проверяет, пуст ли список `(empty? список)`.
pub fn is_empty(args: &[Value]) -> Result<Value, Error> {
    match &args[0] {
        Value::List(items) => Ok(Value::Bool(items.is_empty())),
        _ => Err(Error::type_mismatch("empty? требует список")),
    }
}

/// Конкатенация двух списков `(append список список)`.
pub fn append(args: &[Value]) -> Result<Value, Error> {
    match (&args[0], &args[1]) {
        (Value::List(a), Value::List(b)) => {
            let mut result = a.clone();
            result.extend(b.clone());
            Ok(Value::List(result))
        }
        _ => Err(Error::type_mismatch("append требует два списка")),
    }
}

/// Возвращает элемент списка по индексу `(nth список индекс)` (0-based).
pub fn nth(args: &[Value]) -> Result<Value, Error> {
    let list = match &args[0] {
        Value::List(items) => items,
        _ => {
            return Err(Error::type_mismatch(
                "Первый аргумент nth должен быть списком",
            ))
        }
    };

    let index = match &args[1] {
        Value::Int(i) => *i,
        _ => {
            return Err(Error::type_mismatch(
                "Второй аргумент nth должен быть целым числом",
            ))
        }
    };

    if index < 0 || index as usize >= list.len() {
        return Err(Error::runtime(
            "index-out-of-bounds",
            format!("Индекс вне диапазона: {}", index),
        ));
    }

    Ok(list[index as usize].clone())
}

/// Возвращает длину списка `(length список)`. Синоним: `(len список)`.
pub fn length(args: &[Value]) -> Result<Value, Error> {
    let list = match &args[0] {
        Value::List(items) => items,
        _ => return Err(Error::type_mismatch("length требует списка")),
    };

    Ok(Value::Int(list.len() as i64))
}

/// Проверяет наличие значения в списке `(contains? список значение) -> bool`.
///
/// Возвращает `true`, если среди элементов списка есть элемент, равный
/// значению (по семантике оператора `=`).
pub fn contains(args: &[Value]) -> Result<Value, Error> {
    let list = match &args[0] {
        Value::List(items) => items,
        _ => {
            return Err(Error::type_mismatch(
                "Первый аргумент contains? должен быть списком",
            ))
        }
    };

    Ok(Value::Bool(list.iter().any(|item| *item == args[1])))
}
