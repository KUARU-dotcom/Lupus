//! Функции высшего порядка: `list-map`, `list-filter`, `list-fold`.
//!
//! Функция-аргумент может быть как замыканием, так и нативной функцией
//! (например, `abs` или `senko/sqrt`).

use crate::environment::Environment;
use crate::error::Error;
use crate::interpreter::Interpreter;
use crate::value::{is_truthy, Value};
use std::cell::RefCell;
use std::rc::Rc;

fn list_arg<'a>(args: &'a [Value], function: &str) -> Result<&'a [Value], Error> {
    match &args[0] {
        Value::List(items) => Ok(items),
        _ => Err(Error::type_mismatch(format!(
            "Первый аргумент {function} должен быть списком"
        ))),
    }
}

fn callable_arg<'a>(value: &'a Value, function: &str, position: &str) -> Result<&'a Value, Error> {
    match value {
        Value::Closure(_) | Value::Native { .. } => Ok(value),
        _ => Err(Error::type_mismatch(format!(
            "{position} аргумент {function} должен быть функцией"
        ))),
    }
}

/// `(list-map список функция)`.
pub fn map(
    interpreter: &Interpreter,
    env: &Rc<RefCell<Environment>>,
    args: &[Value],
) -> Result<Value, Error> {
    let list = list_arg(args, "list-map")?;
    let function = callable_arg(&args[1], "list-map", "Второй")?;
    let mut result = Vec::with_capacity(list.len());
    for item in list {
        result.push(interpreter.call_value_with_values(function, vec![item.clone()], env)?);
    }
    Ok(Value::List(result))
}

/// `(list-filter список предикат)`.
pub fn filter(
    interpreter: &Interpreter,
    env: &Rc<RefCell<Environment>>,
    args: &[Value],
) -> Result<Value, Error> {
    let list = list_arg(args, "list-filter")?;
    let function = callable_arg(&args[1], "list-filter", "Второй")?;
    let mut result = Vec::new();
    for item in list {
        let keep = interpreter.call_value_with_values(function, vec![item.clone()], env)?;
        if is_truthy(&keep) {
            result.push(item.clone());
        }
    }
    Ok(Value::List(result))
}

/// `(list-fold список начальное значение функция)`.
pub fn fold(
    interpreter: &Interpreter,
    env: &Rc<RefCell<Environment>>,
    args: &[Value],
) -> Result<Value, Error> {
    let list = list_arg(args, "list-fold")?;
    let function = callable_arg(&args[2], "list-fold", "Третий")?;
    let mut accumulator = args[1].clone();
    for item in list {
        accumulator =
            interpreter.call_value_with_values(function, vec![accumulator, item.clone()], env)?;
    }
    Ok(accumulator)
}
