//! Лексер — токенизация исходного кода Lupus.
//!
//! Производит токены: скобки, целые числа, строки, булевы литералы,
//! литерал `none` и символы. Игнорирует пробелы и комментарии `;;`.

use crate::error::Error;

/// Вид токена.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// Открывающая скобка `(`.
    LParen,
    /// Закрывающая скобка `)`.
    RParen,
    /// Целое число (включая отрицательные литералы вида `-5`).
    Number(i64),
    /// Строковый литерал.
    Str(String),
    /// Булев литерал `true` / `false`.
    Bool(bool),
    /// Литерал `none`.
    None,
    /// Символ (имя переменной, оператор, ключевое слово).
    Symbol(String),
}

/// Токен с позицией в исходном коде.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// Вид токена.
    pub kind: TokenKind,
    /// Номер строки (1-based).
    pub line: usize,
    /// Номер колонки (1-based).
    pub col: usize,
}

/// Лексер, разбирающий исходный код на токены.
pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    /// Создаёт новый лексер для исходного кода.
    pub fn new(src: &'a str) -> Self {
        Lexer {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    /// Возвращает текущий символ (байт) без продвижения позиции.
    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    /// Возвращает следующий символ и продвигает позицию.
    fn next(&mut self) -> Option<u8> {
        let ch = self.peek()?;
        self.pos += 1;
        if ch == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    /// Вызывает ошибку лексера с указанием позиции.
    fn error(&self, message: &str) -> Error {
        Error::new(format!(
            "Ошибка лексера на строке {}, колонке {}: {}",
            self.line, self.col, message
        ))
    }
}

impl<'a> Lexer<'a> {
    /// Пропускает пробелы и комментарии `;;`.
    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_whitespace() => {
                    self.next();
                }
                Some(b';') if self.src.get(self.pos + 1) == Some(&b';') => {
                    // Комментарий до конца строки.
                    while self.peek().is_some() && self.peek() != Some(b'\n') {
                        self.next();
                    }
                }
                _ => break,
            }
        }
    }

    /// Разбирает строковый литерал (открывающая кавычка уже в позиции).
    fn parse_string(&mut self) -> Result<String, Error> {
        self.next(); // открывающая кавычка
        let mut value = String::new();

        while let Some(ch) = self.peek() {
            match ch {
                b'"' => {
                    self.next();
                    return Ok(value);
                }
                b'\\' => {
                    self.next();
                    let esc = self.next().ok_or_else(|| self.error("Незакрытая строка"))?;
                    match esc {
                        b'n' => value.push('\n'),
                        b't' => value.push('\t'),
                        b'"' => value.push('"'),
                        b'\\' => value.push('\\'),
                        other => value.push(other as char),
                    }
                }
                _ => {
                    let b = ch;
                    // Длина UTF-8-последовательности по ведущему байту.
                    let len = if b < 0x80 {
                        1
                    } else if b < 0xE0 {
                        2
                    } else if b < 0xF0 {
                        3
                    } else {
                        4
                    };
                    // Собираем полную последовательность байтов и декодируем её
                    // как единый code point (иначе многобайтовые UTF-8 символы
                    // превращались бы в отдельные Latin-1-подобные символы).
                    let mut bytes = [0u8; 4];
                    let mut got = 0;
                    for slot in bytes.iter_mut().take(len) {
                        if let Some(byte) = self.peek() {
                            *slot = byte;
                            self.next();
                            got += 1;
                        } else {
                            break;
                        }
                    }
                    match std::str::from_utf8(&bytes[..got])
                        .ok()
                        .and_then(|s| s.chars().next())
                    {
                        Some(c) => value.push(c),
                        None => {
                            return Err(self.error("Некорректная UTF-8 последовательность в строке"))
                        }
                    }
                }
            }
        }

        Err(self.error("Незакрытая строка"))
    }

    /// Разбирает целое число (первый символ — цифра или знак минус).
    fn parse_number(&mut self) -> Result<i64, Error> {
        let mut value = String::new();

        if self.peek() == Some(b'-') {
            value.push('-');
            self.next();
        }

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                value.push(ch as char);
                self.next();
            } else {
                break;
            }
        }

        value
            .parse::<i64>()
            .map_err(|_| self.error("Некорректное число"))
    }

    /// Разбирает символ до разделителя (скобка, пробел, точка с запятой).
    fn parse_symbol(&mut self) -> String {
        let mut value = String::new();

        while let Some(ch) = self.peek() {
            if ch == b'(' || ch == b')' || ch.is_ascii_whitespace() || ch == b';' {
                break;
            }
            value.push(ch as char);
            self.next();
        }
        value
    }
}

impl<'a> Lexer<'a> {
    /// Лексический анализ всего исходного кода → список токенов.
    pub fn tokenize(mut self) -> Result<Vec<Token>, Error> {
        let mut tokens = Vec::new();

        while self.pos < self.src.len() {
            self.skip_whitespace_and_comments();
            if self.pos >= self.src.len() {
                break;
            }

            let line = self.line;
            let col = self.col;
            let ch = self.peek().unwrap();

            match ch {
                b'(' => {
                    self.next();
                    tokens.push(Token {
                        kind: TokenKind::LParen,
                        line,
                        col,
                    });
                }
                b')' => {
                    self.next();
                    tokens.push(Token {
                        kind: TokenKind::RParen,
                        line,
                        col,
                    });
                }
                b'"' => {
                    let value = self.parse_string()?;
                    tokens.push(Token {
                        kind: TokenKind::Str(value),
                        line,
                        col,
                    });
                }
                b'-' if self
                    .src
                    .get(self.pos + 1)
                    .is_some_and(|c| c.is_ascii_digit()) =>
                {
                    let value = self.parse_number()?;
                    tokens.push(Token {
                        kind: TokenKind::Number(value),
                        line,
                        col,
                    });
                }
                c if c.is_ascii_digit() => {
                    let value = self.parse_number()?;
                    tokens.push(Token {
                        kind: TokenKind::Number(value),
                        line,
                        col,
                    });
                }
                _ => {
                    let symbol = self.parse_symbol();
                    let kind = match symbol.as_str() {
                        "true" => TokenKind::Bool(true),
                        "false" => TokenKind::Bool(false),
                        "none" => TokenKind::None,
                        _ => TokenKind::Symbol(symbol),
                    };
                    tokens.push(Token { kind, line, col });
                }
            }
        }

        Ok(tokens)
    }
}

/// Удобная функция лексического анализа.
///
/// # Примеры
/// ```
/// use lupus::lexer::lex;
/// let tokens = lex("(define x 5)").unwrap();
/// assert_eq!(tokens.len(), 5);
/// ```
pub fn lex(src: &str) -> Result<Vec<Token>, Error> {
    Lexer::new(src).tokenize()
}
