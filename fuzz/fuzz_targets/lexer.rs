//! Fuzz-таргет: только лексер, без парсера и выполнения.
//!
//! Запуск: `cargo +nightly fuzz run lexer`
//!
//! Смысл отдельного таргета: лексер — единственное место, где есть работа с
//! байтами UTF-8, `char_indices`, комбинирующими знаками и позициями в
//! символах. Мутации байтового потока здесь доходят до кода быстрее, чем через
//! полный конвейер, а проверки дешёвые: позиция каждого токена обязана быть
//! монотонной по (line, col), а `col` — не больше длины строки в символах.

#![no_main]

use libfuzzer_sys::fuzz_target;
use lupus::lexer;

const MAX_INPUT: usize = 8 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT {
        return;
    }
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };

    let Ok(tokens) = lexer::lex(src) else {
        return;
    };

    let mut last_line = 0usize;
    let mut last_col = 0usize;
    for token in &tokens {
        // Позиции 1-based и не должны откатываться назад.
        assert!(
            token.line >= 1 && token.col >= 1,
            "позиция токена не 1-based: line={} col={}",
            token.line,
            token.col
        );
        assert!(
            (token.line, token.col) >= (last_line, last_col),
            "позиции токенов идут назад: ({}, {}) после ({}, {})",
            token.line,
            token.col,
            last_line,
            last_col
        );
        last_line = token.line;
        last_col = token.col;
    }

    // Колонка не может вылезать за длину строки в символах.
    let lines: Vec<&str> = src.lines().collect();
    if let Some(token) = tokens.last() {
        if let Some(line) = lines.get(token.line - 1) {
            assert!(
                token.col <= line.chars().count() + 1,
                "col={} больше длины строки {} в символах (строка {})",
                token.col,
                line.chars().count(),
                token.line
            );
        }
    }
});
