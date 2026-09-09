//! Типы ошибок интерпретатора Lupus.
//!
//! Все ошибки (лексические, синтаксические, runtime) представляются единым
//! типом [`Error`]. Интерпретатор никогда не паникует: любая ошибка
//! возвращается через `Result<T, Error>`.

use std::fmt;

/// Единый тип ошибки интерпретатора Lupus.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    /// Человекочитаемое сообщение об ошибке (на русском языке).
    pub message: String,
}

impl Error {
    /// Создаёт новую ошибку с заданным сообщением.
    pub fn new(message: impl Into<String>) -> Self {
        Error {
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Error {}
