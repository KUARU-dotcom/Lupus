//! Встроенные функции списков: `list`, `nth`, `length`, `contains?`.

use crate::error::Error;
use crate::value::Value;

/// Создаёт список `(list элем...)`.
pub fn make_list(args: &[Value]) -> Value {
    Value::List(args.to_vec())
}

/// Возвращает элемент списка по индексу `(nth список индекс)` (0-based).
///
/// # Ошибки
/// — не ровно два аргумента;
/// — первый аргумент не список;
/// — второй аргумент не целое число;
/// — индекс вне диапазона.
pub fn nth(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::new(format!(
            "(nth ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }

    let list = match &args[0] {
        Value::List(items) => items,
        _ => return Err(Error::new("Первый аргумент nth должен быть списком")),
    };

    let index = match &args[1] {
        Value::Int(i) => *i,
        _ => return Err(Error::new("Второй аргумент nth должен быть целым числом")),
    };

    if index < 0 || index as usize >= list.len() {
        return Err(Error::new(format!("Индекс вне диапазона: {}", index)));
    }

    Ok(list[index as usize].clone())
}

/// Возвращает длину списка `(length список)`. Синоним: `(len список)`.
///
/// # Ошибки
/// — не ровно один аргумент;
/// — аргумент не список.
pub fn length(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::new(format!(
            "(length ...) требует ровно 1 аргумент, получено {}",
            args.len()
        )));
    }

    let list = match &args[0] {
        Value::List(items) => items,
        _ => return Err(Error::new("length требует списка")),
    };

    Ok(Value::Int(list.len() as i64))
}

/// Проверяет наличие значения в списке `(contains? список значение) -> bool`.
///
/// Возвращает `true`, если среди элементов списка есть элемент, равный
/// значению (по семантике оператора `=`).
///
/// # Ошибки
/// — не ровно два аргумента;
/// — первый аргумент не список.
pub fn contains(args: &[Value]) -> Result<Value, Error> {
    if args.len() != 2 {
        return Err(Error::new(format!(
            "(contains? ...) требует ровно 2 аргумента, получено {}",
            args.len()
        )));
    }

    let list = match &args[0] {
        Value::List(items) => items,
        _ => return Err(Error::new("Первый аргумент contains? должен быть списком")),
    };

    Ok(Value::Bool(list.iter().any(|item| *item == args[1])))
}
