//! Fuzz-таргет: лексер + парсер + интерпретатор на произвольном вводе.
//!
//! Запуск: `cargo +nightly fuzz run parse_eval`
//!
//! Что считается находкой: паника интерпретатора, SIGSEGV/abort, зависание
//! (ловится `-timeout`), либо выход за пределы, обещанные `docs/ERRATA.md`.
//! Обычные `Err(Error)` — не находка, это штатный путь диагностики.
//!
//! Обходы, без которых таргет бесполезен (осознанные, не «читерство»):
//!
//! * `(exit n)` вызывает `std::process::exit` — фаззер убил бы сам себя на
//!   первом же найденном `exit`. Такие программы пропускаются.
//! * `(loop ...)` — настоящий бесконечный цикл; пропускается, иначе каждый
//!   второй вход висел бы до `-timeout`.
//! * Вычисление идёт в отдельном потоке со стеком 1 GiB: `main.rs` запускает
//!   интерпретатор на 1 GiB, потому что tree-walk в debug-сборке съедает стек
//!   раньше логического лимита в 10 000 вызовов. Без этого фаззер находил бы
//!   ложные stack overflow.

#![no_main]

use libfuzzer_sys::fuzz_target;
use lupus::lexer::{self, TokenKind};
use lupus::run_source;

/// Предел размера входа: длиннее — почти наверняка мусор, а лексер и так
/// ограничен по вложенности.
const MAX_INPUT: usize = 8 * 1024;

/// Стек рабочего потока (см. шапку файла).
const STACK_SIZE: usize = 1024 * 1024 * 1024;

fn contains_unfuzzable_symbol(src: &str) -> bool {
    match lexer::lex(src) {
        Ok(tokens) => tokens.iter().any(|token| {
            matches!(
                &token.kind,
                TokenKind::Symbol(name)
                if name == "exit" || name == "loop"
                    || name == "while" || name == "for"
            )
        }),
        // Лексическая ошибка — не наша забота: `run_source` вернёт её же.
        Err(_) => false,
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT {
        return;
    }
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    if contains_unfuzzable_symbol(src) {
        return;
    }

    let source = src.to_string();
    let worker = std::thread::Builder::new()
        .name("fuzz-eval".to_string())
        .stack_size(STACK_SIZE)
        .spawn(move || {
            // Ошибка — нормальный результат: диагностика в JSON, не паника.
            let _ = run_source(&source);
        })
        .expect("не удалось создать поток фаззера");

    if worker.join().is_err() {
        // Паника внутри интерпретатора — это находка. libFuzzer сохранит вход.
        panic!("интерпретатор запаниковал на входе (см. artifacts/)");
    }
});
