//! Библиотека интерпретатора языка Lupus.
//!
//! Точка входа для запуска программ и использования интерпретатора
//! из внешних программ / интеграционных тестов.

pub mod ast;
pub mod builtins;
pub mod environment;
pub mod error;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod value;

pub use error::Error;
pub use interpreter::Interpreter;

/// Выполняет исходный код Lupus из строки.
///
/// Возвращает `Result<(), Error>`: при ошибке печатать сообщение через
/// [`Error::message`] и завершаться с кодом 1 должен вызывающий код.
pub fn run_source(src: &str) -> Result<(), Error> {
    run_source_lenient(src, false)
}

/// Выполняет исходный код Lupus с возможностью lenient-режима
/// (игнорирование лишних `)` — см. [`interpreter::sanitize_tokens_lenient`]).
pub fn run_source_lenient(src: &str, lenient: bool) -> Result<(), Error> {
    let tokens = lexer::lex(src)?;
    let tokens = if lenient {
        interpreter::sanitize_tokens_lenient(tokens)
    } else {
        tokens
    };
    let program = parser::parse(tokens)?;
    let mut interpreter = Interpreter::with_lenient_parens(lenient);
    interpreter.run(program)
}

/// Выполняет программу Lupus из файла по пути.
pub fn run_file(path: &str) -> Result<(), Error> {
    run_file_lenient(path, false)
}

/// Выполняет программу Lupus из файла с lenient-режимом.
pub fn run_file_lenient(path: &str, lenient: bool) -> Result<(), Error> {
    let src = std::fs::read_to_string(path)
        .map_err(|_| Error::new(format!("Ошибка: файл не найден: {}", path)))?;
    run_source_lenient(&src, lenient)
}
