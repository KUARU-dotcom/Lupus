//! Минимальная JSON-сериализация без внешних зависимостей.
//!
//! Используется для диагностик (§6.1), AST (§7) и JSONL-вывода тестов
//! (§5.3). Порядок ключей объекта сохраняется как порядок вставки — это
//! требование детерминированной сериализации.

use crate::value::float_to_string;
use std::fmt;

/// JSON-значение.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// Целое число.
    Int(i64),
    /// Число с плавающей точкой.
    Float(f64),
    /// Строка.
    Str(String),
    /// Массив.
    Array(Vec<Json>),
    /// Объект с фиксированным порядком ключей.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Создаёт строковое значение.
    pub fn str(value: impl Into<String>) -> Self {
        Json::Str(value.into())
    }

    /// Создаёт объект из пар ключ-значение.
    pub fn object(entries: Vec<(&str, Json)>) -> Self {
        Json::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        )
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Json::Int(value) => out.push_str(&value.to_string()),
            Json::Float(value) => {
                if value.is_finite() {
                    out.push_str(&float_to_string(*value));
                } else {
                    // В JSON нет inf/nan; для диагностик это не встречается,
                    // но сериализация должна оставаться валидной.
                    out.push_str("null");
                }
            }
            Json::Str(value) => write_json_string(value, out),
            Json::Array(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Object(entries) => {
                out.push('{');
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    write_json_string(key, out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// Экранирует строку по правилам JSON (RFC 8259).
pub fn escape_json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    write_json_string(value, &mut out);
    out
}

fn write_json_string(value: &str, out: &mut String) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        self.write(&mut out);
        f.write_str(&out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escapes_and_preserves_key_order() {
        let value = Json::object(vec![
            ("kind", Json::str("LiteralStr")),
            ("value", Json::str("a\n\"b\"")),
        ]);
        assert_eq!(
            value.to_string(),
            r#"{"kind":"LiteralStr","value":"a\n\"b\""}"#
        );
    }
}
