//! Парсер — построение AST из списка токенов.
//!
//! Реализует рекурсивный спуск: числа, строки, булевы литералы, `none`,
//! символы и списки (формы). Обрабатывает несбалансированные скобки с ошибкой.

use crate::ast::Expr;
use crate::error::Error;
use crate::lexer::{Token, TokenKind};

/// Парсер, преобразующий токены в AST.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    /// Создаёт новый парсер для списка токенов.
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    /// Возвращает текущий токен, если он есть.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// Возвращает текущий токен и продвигает позицию.
    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        token
    }

    /// Формирует ошибку парсера.
    fn error(&self, message: &str) -> Error {
        if let Some(token) = self.peek() {
            Error::new(format!(
                "Ошибка парсера на строке {}, колонке {}: {}",
                token.line, token.col, message
            ))
        } else {
            Error::new(format!("Ошибка парсера: {}", message))
        }
    }

    /// Разбирает одну форму и возвращает соответствующее выражение.
    fn parse_form(&mut self) -> Result<Expr, Error> {
        let token = match self.peek() {
            Some(t) => t.clone(),
            None => return Err(self.error("Неожиданный конец файла")),
        };

        match token.kind {
            TokenKind::Number(n) => {
                self.next();
                Ok(Expr::Number(n))
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
                let mut elements = Vec::new();

                loop {
                    match self.peek() {
                        None => {
                            return Err(
                                self.error("Неожиданный конец файла: ожидалась закрывающая скобка")
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

                Ok(Expr::List(elements))
            }
            TokenKind::RParen => {
                self.next();
                Err(self.error("Неожиданная закрывающая скобка"))
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
