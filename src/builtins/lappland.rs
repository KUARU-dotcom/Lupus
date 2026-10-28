//! Нативный пакет `lappland` (инъекция сетевых отказов).
//!
//! Политика привязана к окружению: `(lappland/malice p)` и
//! `(lappland/delay k)` меняют поведение последующих `texas`-вызовов,
//! выполненных в этом окружении или его потомках в той же лексической цепочке.
//! Настройки не наследуются между независимыми интерпретаторами.

use crate::environment::Environment;
use crate::error::Error;
use crate::interpreter::Interpreter;
use crate::value::{value_to_string, Arity, Value};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// Базовая задержка, на которую умножается `(lappland/delay k)`.
///
/// Фактическая дополнительная пауза = `BASE_DELAY * (k - 1)`, поэтому
/// `k = 1.0` не добавляет задержки.
pub const BASE_DELAY: Duration = Duration::from_millis(10);

/// Экспортируемые имена пакета.
pub fn names() -> &'static [&'static str] {
    &["delay", "malice"]
}

/// Арность функции lappland.
pub fn arity(name: &str) -> Option<Arity> {
    match name {
        "malice" | "delay" => Some(Arity::Exact(1)),
        _ => None,
    }
}

/// Вызывает функцию пакета.
pub fn call(
    _interpreter: &Interpreter,
    env: &Rc<RefCell<Environment>>,
    name: &str,
    args: &[Value],
) -> Result<Value, Error> {
    let Value::Float(value) = &args[0] else {
        return Err(Error::type_mismatch(format!(
            "lappland/{} ожидает float, получено {}",
            name,
            value_to_string(&args[0])
        )));
    };
    let value = *value;

    match name {
        "malice" => {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(Error::type_mismatch(format!(
                    "lappland/malice: вероятность должна быть float в диапазоне 0.0..=1.0, получено {}",
                    value
                )));
            }
            Environment::set_net_malice(env, value);
        }
        "delay" => {
            if !value.is_finite() || value < 1.0 {
                return Err(Error::type_mismatch(format!(
                    "lappland/delay: множитель должен быть float >= 1.0, получено {}",
                    value
                )));
            }
            Environment::set_net_delay(env, value);
        }
        other => {
            return Err(Error::runtime(
                "unknown-module-symbol",
                format!("lappland/{}: неизвестный символ модуля", other),
            ))
        }
    }

    Ok(Value::None)
}

/// Применяет задержку и, если выпал отказ, возвращает failure-значение.
///
/// Вызывается из `texas` перед выполнением сетевой операции. `returns_result`
/// ложно только для `close`, который по спецификации возвращает `unit`.
pub(crate) fn before_net_call(
    interpreter: &Interpreter,
    env: &Rc<RefCell<Environment>>,
    returns_result: bool,
) -> Option<Value> {
    let (malice, delay) = Environment::net_policy(env);

    if delay > 1.0 {
        std::thread::sleep(BASE_DELAY.mul_f64(delay - 1.0));
    }

    if returns_result && interpreter.net_malice_hits(malice) {
        return Some(Value::Failure(Box::new(Value::Str(
            "net: injected malice".to_string(),
        ))));
    }

    None
}
