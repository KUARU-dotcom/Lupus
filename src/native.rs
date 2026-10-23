//! Нативная стандартная библиотека V0.5: пакеты `senko`, `kaltsit`, `w`.
//!
//! Все функции реализованы средствами `std` (внешних зависимостей нет):
//! `senko` — математика на `f64`, `kaltsit` — файловые операции через
//! `std::fs`, `w` — утверждения тестовой системы.

use crate::error::Error;
use crate::interpreter::Interpreter;
use crate::value::{value_to_string, NativeFn, Value};

/// Нативные пакеты, доступные в V0.5.
pub const PACKAGES: &[&str] = &["senko", "kaltsit", "w"];

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
    Some(Value::Native(NativeFn {
        module: package.to_string(),
        name: name.to_string(),
    }))
}

/// Вычисляет значение символа пакета (константа или функция).
pub fn symbol_value(package: &str, name: &str) -> Option<Value> {
    constant(package, name).or_else(|| function(package, name))
}

fn require_arity(package: &str, name: &str, args: &[Value], expected: usize) -> Result<(), Error> {
    if args.len() != expected {
        return Err(Error::arity(format!(
            "{}/{} ожидает {} аргумент(ов), получено {}",
            package,
            name,
            expected,
            args.len()
        )));
    }
    Ok(())
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

/// Вызывает нативную функцию.
pub fn call(
    interpreter: &Interpreter,
    function: &NativeFn,
    args: &[Value],
) -> Result<Value, Error> {
    match function.module.as_str() {
        "senko" => call_senko(&function.name, args),
        "kaltsit" => call_kaltsit(&function.name, args),
        "w" => call_w(interpreter, &function.name, args),
        other => Err(Error::runtime(
            "unknown-module-symbol",
            format!("Неизвестный нативный пакет: {}", other),
        )),
    }
}

/// Математические функции `senko`.
fn call_senko(name: &str, args: &[Value]) -> Result<Value, Error> {
    let unary = |op: fn(f64) -> f64| -> Result<Value, Error> {
        require_arity("senko", name, args, 1)?;
        let value = float_arg("senko", name, args, 0)?;
        if value.is_nan() && !matches!(name, "abs") {
            return Err(domain_error(name, "аргумент NaN"));
        }
        Ok(Value::Float(op(value)))
    };

    match name {
        "sqrt" => {
            require_arity("senko", name, args, 1)?;
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
            require_arity("senko", name, args, 1)?;
            let value = float_arg("senko", name, args, 0)?;
            if value < 0.0 || value.is_nan() {
                return Err(domain_error(name, "логарифм отрицательного числа"));
            }
            Ok(Value::Float(value.ln()))
        }
        "log10" => {
            require_arity("senko", name, args, 1)?;
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
            require_arity("senko", name, args, 2)?;
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
            require_arity("senko", name, args, 2)?;
            let a = float_arg("senko", name, args, 0)?;
            let b = float_arg("senko", name, args, 1)?;
            Ok(Value::Float(a.min(b)))
        }
        "max" => {
            require_arity("senko", name, args, 2)?;
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
            require_arity("kaltsit", name, args, 1)?;
            let path = string_arg("kaltsit", name, args, 0)?;
            Ok(match std::fs::read_to_string(path) {
                Ok(content) => Value::Success(Box::new(Value::Str(content))),
                Err(error) => io_failure(error),
            })
        }
        "write-file" | "append-file" => {
            require_arity("kaltsit", name, args, 2)?;
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
            require_arity("kaltsit", name, args, 1)?;
            let path = string_arg("kaltsit", name, args, 0)?;
            Ok(Value::Bool(std::path::Path::new(&path).exists()))
        }
        "is-dir?" => {
            require_arity("kaltsit", name, args, 1)?;
            let path = string_arg("kaltsit", name, args, 0)?;
            Ok(Value::Bool(std::path::Path::new(&path).is_dir()))
        }
        "list-dir" => {
            require_arity("kaltsit", name, args, 1)?;
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
            require_arity("kaltsit", name, args, 1)?;
            let path = string_arg("kaltsit", name, args, 0)?;
            Ok(match std::fs::create_dir_all(path) {
                Ok(()) => unit_success(),
                Err(error) => io_failure(error),
            })
        }
        "delete" => {
            require_arity("kaltsit", name, args, 1)?;
            let path = string_arg("kaltsit", name, args, 0)?;
            Ok(match std::fs::remove_file(path) {
                Ok(()) => unit_success(),
                Err(error) => io_failure(error),
            })
        }
        "size" => {
            require_arity("kaltsit", name, args, 1)?;
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
            require_arity("w", name, args, 2)?;
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
            require_arity("w", name, args, 1)?;
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
            require_arity("w", name, args, 1)?;
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
            require_arity("w", name, args, 1)?;
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
        "run-all" => {
            require_arity("w", name, args, 0)?;
            Ok(Value::Int(interpreter.run_tests_named(&[])))
        }
        other => Err(Error::runtime(
            "unknown-module-symbol",
            format!("w/{}: неизвестный символ модуля", other),
        )),
    }
}
