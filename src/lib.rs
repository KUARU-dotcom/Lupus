//! Библиотека интерпретатора языка Lupus.
//!
//! Точка входа для запуска программ и использования интерпретатора
//! из внешних программ / интеграционных тестов.

pub mod ast;
pub mod ast_json;
pub mod builtins;
pub mod differential;
pub mod environment;
pub mod error;
pub mod gc;
pub mod generator;
pub mod interpreter;
pub mod json;
pub mod kennel;
pub mod lexer;
pub mod lint;
pub mod marshal;
pub mod modules;
pub mod native;
pub mod parser;
pub mod property;
pub mod shrinker;
pub mod unicode_marks;
pub mod value;

pub use error::Error;
pub use interpreter::Interpreter;

use std::path::Path;

/// Выполняет исходный код Lupus из строки.
///
/// Возвращает `Result<(), Error>`: при ошибке печатать JSON через
/// [`Error::to_json`] и завершаться с кодом 1 должен вызывающий код.
pub fn run_source(src: &str) -> Result<(), Error> {
    run_source_lenient(src, false)
}

/// Выполняет исходный код Lupus с возможностью lenient-режима
/// (игнорирование лишних `)` — см. [`interpreter::sanitize_tokens_lenient`]).
pub fn run_source_lenient(src: &str, lenient: bool) -> Result<(), Error> {
    run_source_named(src, lenient, None)
}

/// Выполняет исходный код с привязкой ошибок к имени файла.
pub fn run_source_named(src: &str, lenient: bool, file: Option<&Path>) -> Result<(), Error> {
    let tokens = lexer::lex(src)?;
    let tokens = if lenient {
        interpreter::sanitize_tokens_lenient(tokens)
    } else {
        tokens
    };
    let program = parser::parse(tokens).map_err(|error| attach_file(error, file))?;
    let mut interpreter = Interpreter::with_lenient_parens(lenient);
    if let Some(path) = file {
        if let Some(parent) = path.parent() {
            interpreter.set_program_dir(parent.to_path_buf());
        }
    }
    interpreter
        .run(program)
        .map_err(|error| error.with_file_if_located(display_path(file)))
}

/// Выполняет программу Lupus из файла по пути.
pub fn run_file(path: &str) -> Result<(), Error> {
    run_file_lenient(path, false)
}

/// Выполняет программу Lupus из файла с lenient-режимом.
pub fn run_file_lenient(path: &str, lenient: bool) -> Result<(), Error> {
    let file = Path::new(path);
    let src = std::fs::read_to_string(file).map_err(|error| {
        Error::lint(
            "unknown-module",
            format!("Не удалось прочитать файл {}: {}", path, error),
        )
    })?;
    run_source_named(&src, lenient, Some(file))
}

/// Лексирует и парсит исходный код без выполнения.
pub fn parse_source(src: &str) -> Result<Vec<ast::Expr>, Error> {
    parse_source_lenient(src, false)
}

/// Лексирует и парсит исходный код с поддержкой lenient-режима.
pub fn parse_source_lenient(src: &str, lenient: bool) -> Result<Vec<ast::Expr>, Error> {
    let tokens = lexer::lex(src)?;
    let tokens = if lenient {
        interpreter::sanitize_tokens_lenient(tokens)
    } else {
        tokens
    };
    parser::parse(tokens)
}

fn attach_file(mut error: Error, file: Option<&Path>) -> Error {
    if let Some(path) = file {
        error = error.with_file_if_located(path.display().to_string());
    }
    error
}

fn display_path(file: Option<&Path>) -> String {
    file.map(|path| path.display().to_string())
        .unwrap_or_else(|| "<input>".to_string())
}
