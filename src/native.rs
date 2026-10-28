//! Нативная стандартная библиотека V0.6: пакеты `senko`, `kaltsit`, `w`,
//! `texas` (сеть) и `lappland` (инъекция сетевых отказов).
//!
//! Все функции реализованы средствами `std` (внешних зависимостей нет):
//! `senko` — математика на `f64`, `kaltsit` — файловые операции через
//! `std::fs`, `w` — утверждения тестовой системы, `texas` — TCP/UDP через
//! `std::net`, `lappland` — управляемые отказы/задержки net-вызовов.

use crate::builtins::{lappland, texas};
use crate::environment::Environment;
use crate::error::Error;
use crate::interpreter::Interpreter;
use crate::value::{value_to_string, Arity, NativeImpl, Value};
use std::cell::RefCell;
use std::rc::Rc;

/// Нативные пакеты, доступные в V0.6.
pub const PACKAGES: &[&str] = &["senko", "kaltsit", "w", "texas", "lappland"];

/// Проверяет, является ли имя нативным пакетом.
pub fn is_native(package: &str) -> bool {
    PACKAGES.contains(&package)
}

/// Список экспортируемых имён пакета (для `:all` и диагностик).
pub fn names(package: &str) -> &'static [&'static str] {
    match package {
        "senko" => &[
            "pi", "e", "sqrt", "sin", "cos", "tan", "log", "log10", "pow", "abs", "floor", "ceil",
            "round", "min", "max",
        ],
        "kaltsit" => &[
            "read-file",
            "write-file",
            "append-file",
            "exists?",
            "is-dir?",
            "list-dir",
            "mkdir",
            "delete",
            "size",
        ],
        "w" => &["assert-eq", "assert-true", "assert-false", "run", "run-all"],
        "texas" => texas::names(),
        "lappland" => lappland::names(),
        _ => &[],
    }
}

/// Проверяет наличие символа в нативном пакете.
pub fn has_symbol(package: &str, name: &str) -> bool {
    names(package).contains(&name)
}

/// Возвращает значение константы пакета, если символ — константа.
pub fn constant(package: &str, name: &str) -> Option<Value> {
    match (package, name) {
        ("senko", "pi") => Some(Value::Float(std::f64::consts::PI)),
        ("senko", "e") => Some(Value::Float(std::f64::consts::E)),
        _ => None,
    }
}

/// Создаёт значение нативной функции, если символ — функция.
pub fn function(package: &str, name: &str) -> Option<Value> {
    if constant(package, name).is_some() || !has_symbol(package, name) {
        return None;
    }
    Some(Value::Native {
        name: format!("{}/{}", package, name),
        arity: function_arity(package, name),
        func: NativeImpl::Module {
            package: package.to_string(),
            symbol: name.to_string(),
        },
    })
}

/// Арность функции нативного пакета. Проверяется централизованно в
/// `Interpreter::call_native`.
fn function_arity(package: &str, name: &str) -> Arity {
    match (package, name) {
        ("senko", "pow" | "min" | "max") => Arity::Exact(2),
        ("kaltsit", "write-file" | "append-file") => Arity::Exact(2),
        ("w", "assert-eq") => Arity::Exact(2),
        ("w", "run-all") => Arity::Exact(0),
        ("w", "assert-true" | "assert-false" | "run") => Arity::Exact(1),
        ("texas", name) => texas::arity(name).unwrap_or(Arity::Exact(1)),
        ("lappland", name) => lappland::arity(name).unwrap_or(Arity::Exact(1)),
        _ => Arity::Exact(1),
    }
}

/// Вычисляет значение символа пакета (константа или функция).
pub fn symbol_value(package: &str, name: &str) -> Option<Value> {
    constant(package, name).or_else(|| function(package, name))
}

fn float_arg(package: &str, name: &str, args: &[Value], index: usize) -> Result<f64, Error> {
    match args.get(index) {
        Some(Value::Float(value)) => Ok(*value),
        Some(Value::Int(value)) => Ok(*value as f64),
        Some(_) => Err(Error::type_mismatch(format!(
            "{}/{} ожидает float, получено {}",
            package,
            name,
            value_to_string(&args[index])
        ))),
        None => Err(Error::arity(format!(
            "{}/{}: недостаточно аргументов",
            package, name
        ))),
    }
}

/// Доменная ошибка математической функции.
fn domain_error(name: &str, message: &str) -> Error {
    Error::runtime(
        "math-domain-error",
        format!("senko/{}: доменная ошибка: {}", name, message),
    )
}

/// Вызывает нативную функцию пакета.
pub fn call(
    interpreter: &Interpreter,
    env: &Rc<RefCell<Environment>>,
    package: &str,
    symbol: &str,
    args: &[Value],
) -> Result<Value, Error> {
    match package {
        "senko" => call_senko(symbol, args),
        "kaltsit" => call_kaltsit(symbol, args),
        "w" => call_w(interpreter, symbol, args),
        "texas" => texas::call(interpreter, env, symbol, args),
        "lappland" => lappland::call(interpreter, env, symbol, args),
        other => Err(Error::runtime(
            "unknown-module-symbol",
            format!("Неизвестный нативный пакет: {}", other),
        )),
    }
}

/// Математические функции `senko`.
fn call_senko(name: &str, args: &[Value]) -> Result<Value, Error> {
    let unary = |op: fn(f64) -> f64| -> Result<Value, Error> {
        let value = float_arg("senko", name, args, 0)?;
        if value.is_nan() && !matches!(name, "abs") {
            return Err(domain_error(name, "аргумент NaN"));
        }
        Ok(Value::Float(op(value)))
    };

    match name {
        "sqrt" => {
            let value = float_arg("senko", name, args, 0)?;
            if value < 0.0 || value.is_nan() {
                return Err(domain_error(
                    name,
                    "квадратный корень из отрицательного числа",
                ));
            }
            Ok(Value::Float(value.sqrt()))
        }
        "sin" => unary(f64::sin),
        "cos" => unary(f64::cos),
        "tan" => unary(f64::tan),
        "log" => {
            let value = float_arg("senko", name, args, 0)?;
            if value < 0.0 || value.is_nan() {
                return Err(domain_error(name, "логарифм отрицательного числа"));
            }
            Ok(Value::Float(value.ln()))
        }
        "log10" => {
            let value = float_arg("senko", name, args, 0)?;
            if value < 0.0 || value.is_nan() {
                return Err(domain_error(
                    name,
                    "десятичный логарифм отрицательного числа",
                ));
            }
            Ok(Value::Float(value.log10()))
        }
        "abs" => unary(f64::abs),
        "floor" => unary(f64::floor),
        "ceil" => unary(f64::ceil),
        "round" => unary(f64::round),
        "pow" => {
            let base = float_arg("senko", name, args, 0)?;
            let exponent = float_arg("senko", name, args, 1)?;
            let result = base.powf(exponent);
            if result.is_nan() {
                return Err(domain_error(
                    name,
                    "результат возведения в степень не определён",
                ));
            }
            Ok(Value::Float(result))
        }
        "min" => {
            let a = float_arg("senko", name, args, 0)?;
            let b = float_arg("senko", name, args, 1)?;
            Ok(Value::Float(a.min(b)))
        }
        "max" => {
            let a = float_arg("senko", name, args, 0)?;
            let b = float_arg("senko", name, args, 1)?;
            Ok(Value::Float(a.max(b)))
        }
        other => Err(Error::runtime(
            "unknown-module-symbol",
            format!("senko/{}: неизвестный символ модуля", other),
        )),
    }
}

/// Строковый аргумент `kaltsit`/`w`.
fn string_arg(package: &str, name: &str, args: &[Value], index: usize) -> Result<String, Error> {
    match args.get(index) {
        Some(Value::Str(value)) => Ok(value.clone()),
        Some(_) => Err(Error::type_mismatch(format!(
            "{}/{} ожидает str, получено {}",
            package,
            name,
            value_to_string(&args[index])
        ))),
        None => Err(Error::arity(format!(
            "{}/{}: недостаточно аргументов",
            package, name
        ))),
    }
}

fn io_failure(error: std::io::Error) -> Value {
    Value::Failure(Box::new(Value::Str(format!("io: {}", error))))
}

fn unit_success() -> Value {
    Value::Success(Box::new(Value::None))
}

/// Файловые операции `kaltsit`.
fn call_kaltsit(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "read-file" => {
            let path = string_arg("kaltsit", name, args, 0)?;
            Ok(match std::fs::read_to_string(path) {
                Ok(content) => Value::Success(Box::new(Value::Str(content))),
                Err(error) => io_failure(error),
            })
        }
        "write-file" | "append-file" => {
            let path = string_arg("kaltsit", name, args, 0)?;
            let content = string_arg("kaltsit", name, args, 1)?;
            let result = if name == "write-file" {
                std::fs::write(path, content)
            } else {
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .and_then(|mut file| file.write_all(content.as_bytes()))
            };
            Ok(match result {
                Ok(()) => unit_success(),
                Err(error) => io_failure(error),
            })
        }
        "exists?" => {
            let path = string_arg("kaltsit", name, args, 0)?;
            Ok(Value::Bool(std::path::Path::new(&path).exists()))
        }
        "is-dir?" => {
            let path = string_arg("kaltsit", name, args, 0)?;
            Ok(Value::Bool(std::path::Path::new(&path).is_dir()))
        }
        "list-dir" => {
            let path = string_arg("kaltsit", name, args, 0)?;
            match std::fs::read_dir(path) {
                Ok(entries) => {
                    let mut names: Vec<String> = Vec::new();
                    for entry in entries {
                        match entry {
                            Ok(entry) => names.push(entry.file_name().to_string_lossy().into()),
                            Err(error) => return Ok(io_failure(error)),
                        }
                    }
                    names.sort();
                    Ok(Value::Success(Box::new(Value::List(
                        names.into_iter().map(Value::Str).collect(),
                    ))))
                }
                Err(error) => Ok(io_failure(error)),
            }
        }
        "mkdir" => {
            let path = string_arg("kaltsit", name, args, 0)?;
            Ok(match std::fs::create_dir_all(path) {
                Ok(()) => unit_success(),
                Err(error) => io_failure(error),
            })
        }
        "delete" => {
            let path = string_arg("kaltsit", name, args, 0)?;
            Ok(match std::fs::remove_file(path) {
                Ok(()) => unit_success(),
                Err(error) => io_failure(error),
            })
        }
        "size" => {
            let path = string_arg("kaltsit", name, args, 0)?;
            Ok(match std::fs::metadata(path) {
                Ok(metadata) => Value::Success(Box::new(Value::Int(metadata.len() as i64))),
                Err(error) => io_failure(error),
            })
        }
        other => Err(Error::runtime(
            "unknown-module-symbol",
            format!("kaltsit/{}: неизвестный символ модуля", other),
        )),
    }
}

/// Тестовые утверждения `w`.
fn call_w(interpreter: &Interpreter, name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "assert-eq" => {
            if args[0] == args[1] {
                Ok(Value::None)
            } else {
                Err(Error::runtime(
                    "assert-failed",
                    format!(
                        "assert-eq: ожидалось {}, получено {}",
                        value_to_string(&args[0]),
                        value_to_string(&args[1])
                    ),
                )
                .with_token(value_to_string(&args[1])))
            }
        }
        "assert-true" => {
            if args[0] == Value::Bool(true) {
                Ok(Value::None)
            } else {
                Err(
                    Error::runtime("assert-failed", "assert-true: ожидалось true")
                        .with_token(value_to_string(&args[0])),
                )
            }
        }
        "assert-false" => {
            if args[0] == Value::Bool(false) {
                Ok(Value::None)
            } else {
                Err(
                    Error::runtime("assert-failed", "assert-false: ожидалось false")
                        .with_token(value_to_string(&args[0])),
                )
            }
        }
        "run" => {
            let Value::List(items) = &args[0] else {
                return Err(Error::type_mismatch(
                    "w/run ожидает список имён тестов (List str)",
                ));
            };
            let mut names = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::Str(value) => names.push(value.clone()),
                    _ => {
                        return Err(Error::type_mismatch(
                            "w/run ожидает список строк (List str)",
                        ))
                    }
                }
            }
            Ok(Value::Int(interpreter.run_tests_named(&names)))
        }
        "run-all" => Ok(Value::Int(interpreter.run_tests_named(&[]))),
        other => Err(Error::runtime(
            "unknown-module-symbol",
            format!("w/{}: неизвестный символ модуля", other),
        )),
    }
}
