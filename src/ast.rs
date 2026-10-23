//! Определения узлов абстрактного синтаксического дерева (AST) Lupus.

/// Узел абстрактного синтаксического дерева.
///
/// Язык Lupus использует префиксную нотацию, поэтому любой вызов или
/// управляющая конструкция представлены списком [`Expr::List`], первый элемент
/// которого — имя формы (символ) или произвольное выражение (например, lambda).
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
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

/// Преобразует узел AST обратно в исходный текст Lupus.
///
/// Списки печатаются в скобках через пробел, строки — в кавычках с
/// экранированием, литералы — в их обычной записи. Используется в
/// диагностиках (`assert`, JSONL `test-fail`) вместо Rust `Debug`.
pub fn expr_to_source(expr: &Expr) -> String {
    match expr {
        Expr::Number(value) => value.to_string(),
        Expr::Float(value) => crate::value::float_to_string(*value),
        Expr::Str(value) => {
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
        Expr::Bool(true) => "true".to_string(),
        Expr::Bool(false) => "false".to_string(),
        Expr::None => "none".to_string(),
        Expr::Symbol(name) => name.clone(),
        Expr::List(items) => {
            let body: Vec<String> = items.iter().map(expr_to_source).collect();
            format!("({})", body.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expr_to_source_round_trips_forms() {
        let expr = Expr::List(vec![
            Expr::Symbol("=".to_string()),
            Expr::List(vec![
                Expr::Symbol("+".to_string()),
                Expr::Number(1),
                Expr::Number(2),
            ]),
            Expr::Number(4),
        ]);
        assert_eq!(expr_to_source(&expr), "(= (+ 1 2) 4)");
        assert_eq!(
            expr_to_source(&Expr::Str("a\n\"b\"".to_string())),
            "\"a\\n\\\"b\\\"\""
        );
        assert_eq!(expr_to_source(&Expr::Float(1.0)), "1.0");
        assert_eq!(expr_to_source(&Expr::List(vec![])), "()");
    }
}
