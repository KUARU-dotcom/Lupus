//! Лексер — токенизация исходного кода Lupus.
//!
//! Производит токены: скобки, целые числа, строки, булевы литералы,
//! литерал `none` и символы. Игнорирует пробелы, строчные комментарии
//! (`;` или `;;` до конца строки) и блочные комментарии `#| ... |#`.
//! Символы вне алфавита языка диагностируются кодом `unknown-token`.

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
    /// Литерал с плавающей точкой (например, `3.14`, `2.5e-3`, `0.5`).
    Float(f64),
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
    /// Начальный байтовый offset токена (включительно).
    pub start: usize,
    /// Конечный байтовый offset токена (исключительно).
    pub end: usize,
}

/// Лексер, разбирающий исходный код на токены.
pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}

/// Проверяет, может ли символ входить в простой (неквалифицированный) символ.
///
/// Квалифицированные имена (`senko/sqrt`) пока представлены одним токеном:
/// `/` входит в алфавит, а разделение выполняется интерпретатором/парсером.
fn is_symbol_char(ch: u8) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(
            ch,
            b'_' | b'-' | b'+' | b'*' | b'/' | b'%' | b'=' | b'!' | b'<' | b'>' | b'?' | b':'
        )
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

    /// Возвращает байт по смещению от текущей позиции.
    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.src.get(self.pos + offset).copied()
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

    /// Вызывает ошибку лексера с кодом.
    fn error(&self, code: &str, message: &str) -> Error {
        Error::lex(code, message, self.line, self.col)
    }

    /// Пропускает пробелы и комментарии.
    fn skip_whitespace_and_comments(&mut self) -> Result<(), Error> {
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_whitespace() => {
                    self.next();
                }
                // Одиночная `;` тоже начинает комментарий (ERRATA: иначе
                // лексер зациклился бы на одном символе).
                Some(b';') => {
                    while self.peek().is_some() && self.peek() != Some(b'\n') {
                        self.next();
                    }
                }
                // Блочный комментарий `#| ... |#`.
                Some(b'#') if self.peek_at(1) == Some(b'|') => {
                    self.next();
                    self.next();
                    let (start_line, start_col) = (self.line, self.col);
                    loop {
                        match self.peek() {
                            None => {
                                return Err(Error::lex(
                                    "unclosed-comment",
                                    "Незакрытый блочный комментарий (#| ... |#)",
                                    start_line,
                                    start_col,
                                ));
                            }
                            Some(b'|') if self.peek_at(1) == Some(b'#') => {
                                self.next();
                                self.next();
                                break;
                            }
                            Some(_) => {
                                self.next();
                            }
                        }
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Читает ровно `count` hex-цифр и возвращает значение.
    fn read_hex(&mut self, count: usize) -> Option<u32> {
        let mut value = 0u32;
        for _ in 0..count {
            let ch = self.next()?;
            let digit = (ch as char).to_digit(16)?;
            value = value * 16 + digit;
        }
        Some(value)
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
                    let esc = self
                        .next()
                        .ok_or_else(|| self.error("unclosed-string", "Незакрытая строка"))?;
                    match esc {
                        b'n' => value.push('\n'),
                        b't' => value.push('\t'),
                        b'r' => value.push('\r'),
                        b'0' => value.push('\0'),
                        b'"' => value.push('"'),
                        b'\\' => value.push('\\'),
                        b'x' => {
                            let code = self.read_hex(2).ok_or_else(|| {
                                self.error("unknown-token", "Некорректный \\x-escape в строке")
                            })?;
                            if let Some(c) = char::from_u32(code) {
                                value.push(c);
                            }
                        }
                        b'u' => {
                            let code = self.read_hex(4).ok_or_else(|| {
                                self.error("unknown-token", "Некорректный \\u-escape в строке")
                            })?;
                            if let Some(c) = char::from_u32(code) {
                                value.push(c);
                            }
                        }
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
                            return Err(self.error(
                                "unclosed-string",
                                "Некорректная UTF-8 последовательность в строке",
                            ))
                        }
                    }
                }
            }
        }

        Err(self.error("unclosed-string", "Незакрытая строка"))
    }

    /// Разбирает число (целое или float). Первый символ — цифра или знак минус.
    fn parse_number(&mut self) -> Result<TokenKind, Error> {
        let mut is_float = false;
        let mut value = String::new();

        if self.peek() == Some(b'-') {
            value.push('-');
            self.next();
        }

        // Целая часть.
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            value.push(self.next().unwrap() as char);
        }

        // Дробная часть (после '.').
        if self.peek() == Some(b'.') {
            is_float = true;
            value.push('.');
            self.next();
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                value.push(self.next().unwrap() as char);
            }
        }

        // Экспонента (после e/E), допускает знак +/-.
        if self.peek().is_some_and(|c| c == b'e' || c == b'E') {
            is_float = true;
            value.push('e');
            self.next();
            if self.peek().is_some_and(|c| c == b'+' || c == b'-') {
                value.push(self.next().unwrap() as char);
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                value.push(self.next().unwrap() as char);
            }
        }

        if is_float {
            value
                .parse::<f64>()
                .map(TokenKind::Float)
                .map_err(|_| self.error("unknown-token", "Некорректное число с плавающей точкой"))
        } else {
            value
                .parse::<i64>()
                .map(TokenKind::Number)
                .map_err(|_| self.error("unknown-token", "Некорректное число"))
        }
    }

    /// Разбирает символ до разделителя (скобка, пробел, комментарий).
    ///
    /// Возвращает ошибку `unknown-token`, если встречается символ вне
    /// алфавита языка (например, `@`, `$`, `.` вне числа).
    fn parse_symbol(&mut self) -> Result<String, Error> {
        let mut value = String::new();

        while let Some(ch) = self.peek() {
            if ch == b'(' || ch == b')' || ch.is_ascii_whitespace() || ch == b';' {
                break;
            }
            if !is_symbol_char(ch) {
                return Err(self.error(
                    "unknown-token",
                    &format!("Неизвестный токен: '{}'", ch as char),
                ));
            }
            value.push(ch as char);
            self.next();
        }
        Ok(value)
    }
}

impl<'a> Lexer<'a> {
    /// Лексический анализ всего исходного кода → список токенов.
    pub fn tokenize(mut self) -> Result<Vec<Token>, Error> {
        let mut tokens = Vec::new();

        while self.pos < self.src.len() {
            self.skip_whitespace_and_comments()?;
            if self.pos >= self.src.len() {
                break;
            }

            let line = self.line;
            let col = self.col;
            let start = self.pos;
            let ch = self.peek().unwrap();

            match ch {
                b'(' => {
                    self.next();
                    tokens.push(Token {
                        kind: TokenKind::LParen,
                        line,
                        col,
                        start,
                        end: self.pos,
                    });
                }
                b')' => {
                    self.next();
                    tokens.push(Token {
                        kind: TokenKind::RParen,
                        line,
                        col,
                        start,
                        end: self.pos,
                    });
                }
                b'"' => {
                    let value = self.parse_string()?;
                    tokens.push(Token {
                        kind: TokenKind::Str(value),
                        line,
                        col,
                        start,
                        end: self.pos,
                    });
                }
                b'-' if self
                    .src
                    .get(self.pos + 1)
                    .is_some_and(|c| c.is_ascii_digit()) =>
                {
                    let kind = self.parse_number()?;
                    tokens.push(Token {
                        kind,
                        line,
                        col,
                        start,
                        end: self.pos,
                    });
                }
                c if c.is_ascii_digit() => {
                    let kind = self.parse_number()?;
                    tokens.push(Token {
                        kind,
                        line,
                        col,
                        start,
                        end: self.pos,
                    });
                }
                b'#' => {
                    // Разрешены только `#lupus` (директива) и `#| ... |#`
                    // (блочный комментарий, обработан выше).
                    let is_lupus = self.src[self.pos..].starts_with(b"#lupus")
                        && !self
                            .src
                            .get(self.pos + 6)
                            .is_some_and(|c| is_symbol_char(*c));
                    if is_lupus {
                        for _ in 0..6 {
                            self.next();
                        }
                        tokens.push(Token {
                            kind: TokenKind::Symbol("#lupus".to_string()),
                            line,
                            col,
                            start,
                            end: self.pos,
                        });
                    } else {
                        return Err(self.error(
                            "unknown-token",
                            "Неизвестный токен: '#' (допустимы '#lupus' и '#| ... |#')",
                        ));
                    }
                }
                c if is_symbol_char(c) => {
                    let symbol = self.parse_symbol()?;
                    let kind = match symbol.as_str() {
                        "true" => TokenKind::Bool(true),
                        "false" => TokenKind::Bool(false),
                        "none" => TokenKind::None,
                        _ => TokenKind::Symbol(symbol),
                    };
                    tokens.push(Token {
                        kind,
                        line,
                        col,
                        start,
                        end: self.pos,
                    });
                }
                _ => {
                    return Err(self.error(
                        "unknown-token",
                        &format!("Неизвестный токен: '{}'", ch as char),
                    ));
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
