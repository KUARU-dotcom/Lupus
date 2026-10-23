//! Парсер — построение AST из списка токенов.
//!
//! Реализует рекурсивный спуск: числа, строки, булевы литералы, `none`,
//! символы и списки (формы). Обрабатывает несбалансированные скобки с ошибкой
//! `missing-rparen` и лишние `)` — ошибкой `unexpected-token`.

use crate::ast::Expr;
use crate::error::{Error, Span};
use crate::lexer::{Token, TokenKind};

/// Максимальная глубина вложенности скобок.
const MAX_NESTING: u32 = 1_000;

/// Парсер, преобразующий токены в AST.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    /// Позиция последнего успешно прочитанного токена (для ошибок в EOF).
    last_line: usize,
    last_col: usize,
    /// Текущая глубина вложенности скобок.
    nesting: u32,
}

impl Parser {
    /// Создаёт новый парсер для списка токенов.
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            pos: 0,
            last_line: 1,
            last_col: 1,
            nesting: 0,
        }
    }

    /// Возвращает текущий токен, если он есть.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// Возвращает текущий токен и продвигает позицию.
    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if let Some(token) = &token {
            self.last_line = token.line;
            self.last_col = token.col;
        }
        self.pos += 1;
        token
    }

    /// Формирует ошибку парсера в текущей позиции.
    fn error(&self, code: &str, message: &str) -> Error {
        let (line, col) = match self.peek() {
            Some(token) => (token.line, token.col),
            None => (self.last_line, self.last_col),
        };
        Error::parse(code, message, line, col)
    }

    /// Разбирает одну форму и возвращает соответствующее выражение.
    fn parse_form(&mut self) -> Result<Expr, Error> {
        let token = match self.peek() {
            Some(t) => t.clone(),
            None => {
                return Err(self.error(
                    "unexpected-token",
                    "Неожиданный конец файла: ожидалось выражение",
                ))
            }
        };

        match token.kind {
            TokenKind::Number(n) => {
                self.next();
                Ok(Expr::Number(n))
            }
            TokenKind::Float(f) => {
                self.next();
                Ok(Expr::Float(f))
            }
            TokenKind::Str(s) => {
                self.next();
                Ok(Expr::Str(s))
            }
            TokenKind::Bool(b) => {
                self.next();
                Ok(Expr::Bool(b))
            }
            TokenKind::None => {
                self.next();
                Ok(Expr::None)
            }
            TokenKind::Symbol(name) => {
                self.next();
                Ok(Expr::Symbol(name))
            }
            TokenKind::LParen => {
                self.next();
                self.nesting += 1;
                if self.nesting > MAX_NESTING {
                    return Err(Error::parse_with_span(
                        "nesting-too-deep",
                        "Слишком глубокая вложенность скобок (>1000).",
                        token.line,
                        token.col,
                        Span {
                            start: token.start,
                            end: token.end,
                        },
                    ));
                }
                let mut elements = Vec::new();

                loop {
                    match self.peek() {
                        None => {
                            return Err(
                                self.error("missing-rparen", "Пропущена закрывающая скобка ')'")
                            );
                        }
                        Some(token) if token.kind == TokenKind::RParen => {
                            self.next();
                            break;
                        }
                        _ => {
                            let form = self.parse_form()?;
                            elements.push(form);
                        }
                    }
                }

                self.nesting -= 1;
                Ok(Expr::List(elements))
            }
            TokenKind::RParen => {
                self.next();
                Err(self.error("unexpected-token", "Неожиданная закрывающая скобка ')'"))
            }
        }
    }

    /// Разбирает весь файл и возвращает список форм верхнего уровня.
    pub fn parse(mut self) -> Result<Vec<Expr>, Error> {
        let mut forms = Vec::new();

        while self.pos < self.tokens.len() {
            let form = self.parse_form()?;
            forms.push(form);
        }

        Ok(forms)
    }
}

/// Удобная функция синтаксического анализа.
///
/// # Примеры
/// ```
/// use lupus::parser::parse;
/// let forms = parse(vec![]).unwrap();
/// assert!(forms.is_empty());
/// ```
pub fn parse(tokens: Vec<Token>) -> Result<Vec<Expr>, Error> {
    Parser::new(tokens).parse()
}
