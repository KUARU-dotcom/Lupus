//! Определения узлов абстрактного синтаксического дерева (AST) Lupus.

use crate::error::Span;

/// Вид узла абстрактного синтаксического дерева.
///
/// Язык Lupus использует префиксную нотацию, поэтому любой вызов или
/// управляющая конструкция представлены списком [`ExprKind::List`], первый
/// элемент которого — имя формы (символ) или произвольное выражение
/// (например, lambda).
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// Целочисленный литерал.
    Number(i64),
    /// Литерал с плавающей точкой.
    Float(f64),
    /// Строковый литерал.
    Str(String),
    /// Булев литерал (`true` / `false`).
    Bool(bool),
    /// Литерал `none`.
    None,
    /// Символ — имя переменной или функции.
    Symbol(String),
    /// Список форм (обычно вызов функции или специальная форма).
    List(Vec<Expr>),
}

/// Узел AST с необязательной привязкой к исходному тексту.
///
/// `span` хранит байтовые границы узла, `line`/`col` — позицию его начала
/// (1-based, `col` в символах, как у лексера). Синтезированные узлы без
/// исходного текста создаются через [`Expr::unspanned`].
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    /// Вид узла.
    pub kind: ExprKind,
    /// Байтовый span узла в исходнике.
    pub span: Option<Span>,
    /// Строка начала узла (1-based; 0 — позиция неизвестна).
    pub line: usize,
    /// Колонка начала узла в символах (1-based; 0 — позиция неизвестна).
    pub col: usize,
}

impl Expr {
    /// Создаёт узел с байтовым span и позицией начала.
    pub fn new(kind: ExprKind, span: Span, line: usize, col: usize) -> Self {
        Expr {
            kind,
            span: Some(span),
            line,
            col,
        }
    }

    /// Создаёт синтезированный узел без привязки к исходному тексту.
    pub fn unspanned(kind: ExprKind) -> Self {
        Expr {
            kind,
            span: None,
            line: 0,
            col: 0,
        }
    }

    /// Возвращает имя символа, если узел — символ.
    pub fn as_symbol(&self) -> Option<&str> {
        match &self.kind {
            ExprKind::Symbol(name) => Some(name),
            _ => None,
        }
    }

    /// Возвращает элементы списка, если узел — список.
    pub fn as_list(&self) -> Option<&[Expr]> {
        match &self.kind {
            ExprKind::List(items) => Some(items),
            _ => None,
        }
    }

    /// Возвращает байтовый span и позицию начала, если узел привязан к
    /// исходному тексту.
    pub fn source_location(&self) -> Option<(Span, usize, usize)> {
        let span = self.span?;
        if self.line == 0 {
            return None;
        }
        Some((span, self.line, self.col))
    }
}

/// Преобразует узел AST обратно в исходный текст Lupus.
///
/// Списки печатаются в скобках через пробел, строки — в кавычках с
/// экранированием, литералы — в их обычной записи. Используется в
/// диагностиках (`assert`, JSONL `test-fail`) вместо Rust `Debug`.
pub fn expr_to_source(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Number(value) => value.to_string(),
        ExprKind::Float(value) => crate::value::float_to_string(*value),
        ExprKind::Str(value) => {
            let mut out = String::with_capacity(value.len() + 2);
            out.push('"');
            for ch in value.chars() {
                match ch {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                    c => out.push(c),
                }
            }
            out.push('"');
            out
        }
        ExprKind::Bool(true) => "true".to_string(),
        ExprKind::Bool(false) => "false".to_string(),
        ExprKind::None => "none".to_string(),
        ExprKind::Symbol(name) => name.clone(),
        ExprKind::List(items) => {
            let body: Vec<String> = items.iter().map(expr_to_source).collect();
            format!("({})", body.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unspanned(kind: ExprKind) -> Expr {
        Expr::unspanned(kind)
    }

    #[test]
    fn expr_to_source_round_trips_forms() {
        let expr = unspanned(ExprKind::List(vec![
            unspanned(ExprKind::Symbol("=".to_string())),
            unspanned(ExprKind::List(vec![
                unspanned(ExprKind::Symbol("+".to_string())),
                unspanned(ExprKind::Number(1)),
                unspanned(ExprKind::Number(2)),
            ])),
            unspanned(ExprKind::Number(4)),
        ]));
        assert_eq!(expr_to_source(&expr), "(= (+ 1 2) 4)");
        assert_eq!(
            expr_to_source(&unspanned(ExprKind::Str("a\n\"b\"".to_string()))),
            "\"a\\n\\\"b\\\"\""
        );
        assert_eq!(expr_to_source(&unspanned(ExprKind::Float(1.0))), "1.0");
        assert_eq!(expr_to_source(&unspanned(ExprKind::List(vec![]))), "()");
    }
}
