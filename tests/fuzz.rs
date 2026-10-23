//! LiF фаза 2: fuzz-smoke интерпретатора (этап F1).
//!
//! Генерирует 2000 случайных программ (смесь валидных форм и token-soup)
//! и запускает каждую с таймаутом 2 секунды. Допустимые исходы: код 0 или 1
//! с JSON-ошибкой в stderr. Panic, сигнал, код 101 или зависание — провал.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const ITERATIONS: usize = 2000;
const TIMEOUT: Duration = Duration::from_secs(2);

fn temp_file() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lupus_fuzz_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.join("fuzz.lupus")
}

/// Детерминированный xorshift64* — без внешних зависимостей.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

fn atom(rng: &mut Rng) -> String {
    match rng.below(10) {
        0 => format!("{}", rng.below(41) as i64 - 20),
        1 => format!("{}.{}", rng.below(10), rng.below(10)),
        2 => {
            let strings = [
                "\"\"",
                "\"abc\"",
                "\"привет\"",
                "\"🐺\"",
                "\"a b c\"",
                "\"line\\nbreak\"",
            ];
            strings[rng.below(strings.len())].to_string()
        }
        3 => ["true", "false"][rng.below(2)].to_string(),
        4 => "none".to_string(),
        5 => ["x", "y", "z"][rng.below(3)].to_string(),
        6 => ["+", "-", "*", "/", "%", "=", "<", ">"][rng.below(8)].to_string(),
        7 => ["list", "tuple", "length", "not", "int->str"][rng.below(5)].to_string(),
        8 => "senko/sqrt".to_string(),
        _ => ["(list 1 2)", "(tuple 1 2)", "(map (tuple \"k\" 1))"][rng.below(3)].to_string(),
    }
}

fn call(rng: &mut Rng, depth: usize) -> String {
    let operators = [
        "+",
        "-",
        "*",
        "/",
        "%",
        "=",
        "!=",
        "<",
        ">",
        "<=",
        ">=",
        "min",
        "max",
        "abs",
        "pow",
        "not",
        "list",
        "tuple",
        "length",
        "nth",
        "contains?",
        "string-append",
        "string-length",
        "string-split",
        "string-reverse",
        "int->str",
        "float->str",
        "int->float",
        "float->int",
    ];
    let operator = operators[rng.below(operators.len())];
    let count = rng.below(4);
    let mut args = Vec::new();
    for _ in 0..count {
        args.push(form(rng, depth.saturating_sub(1)));
    }
    format!("({} {})", operator, args.join(" "))
}

fn form(rng: &mut Rng, depth: usize) -> String {
    if depth == 0 || rng.chance(45) {
        return atom(rng);
    }
    match rng.below(12) {
        0 | 1 => call(rng, depth),
        2 => format!("(print {})", form(rng, depth - 1)),
        3 => format!(
            "(if {} {} {})",
            form(rng, depth - 1),
            form(rng, depth - 1),
            form(rng, depth - 1)
        ),
        4 => format!(
            "(define {} {})",
            ["x", "y", "z"][rng.below(3)],
            form(rng, depth - 1)
        ),
        5 => format!(
            "(let ((x {}) (y {})) {})",
            form(rng, depth - 1),
            form(rng, depth - 1),
            form(rng, depth - 1)
        ),
        6 => format!(
            "(match {} ((some v) v) (none 0) (_ -1))",
            form(rng, depth - 1)
        ),
        7 => format!(
            "(map (tuple {} {}) (tuple {} {}))",
            form(rng, depth - 1),
            form(rng, depth - 1),
            form(rng, depth - 1),
            form(rng, depth - 1)
        ),
        8 => [
            "(import senko)",
            "(import kaltsit)",
            "(import w)",
            "(import util)",
        ][rng.below(4)]
        .to_string(),
        9 => format!(
            "(defstruct Point{} (x int) (y int))",
            if rng.chance(50) { " (a)" } else { "" }
        ),
        10 => format!("(test \"t{}\" {})", rng.below(5), form(rng, depth - 1)),
        _ => format!(
            "(defstruct Pair (first int) (second int))\n(print (Pair {} {}))",
            form(rng, depth - 1),
            form(rng, depth - 1)
        ),
    }
}

fn token_soup(rng: &mut Rng) -> String {
    let tokens = [
        "(",
        ")",
        "print",
        "define",
        "if",
        "let",
        "lambda",
        "senko/sqrt",
        "1",
        "2.5",
        "\"s\"",
        "none",
        "true",
        "+",
        "=",
        "@",
        "#",
        "$",
        "%",
        ":",
        "\\",
        "unterminated",
        "#|",
        "senko",
        "w",
        "util",
        "match",
        "some",
    ];
    let count = 1 + rng.below(30);
    let mut parts = Vec::with_capacity(count);
    for _ in 0..count {
        parts.push(tokens[rng.below(tokens.len())]);
    }
    parts.join(" ")
}

fn generate(rng: &mut Rng) -> String {
    if rng.chance(30) {
        return token_soup(rng);
    }
    let count = 1 + rng.below(12);
    let mut forms = Vec::with_capacity(count);
    for _ in 0..count {
        forms.push(form(rng, 3));
    }
    forms.join("\n")
}

enum Outcome {
    Exited { code: i32, stderr: String },
    TimedOut,
}

fn run_with_timeout(path: &PathBuf) -> Outcome {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lupus"))
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn lupus");

    let started = Instant::now();
    loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => {
                let mut stderr = String::new();
                if let Some(mut pipe) = child.stderr.take() {
                    let _ = pipe.read_to_string(&mut stderr);
                }
                return Outcome::Exited {
                    code: status.code().unwrap_or(-1),
                    stderr,
                };
            }
            None => {
                if started.elapsed() > TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Outcome::TimedOut;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

fn assert_valid_outcome(source: &str, outcome: Outcome) {
    match outcome {
        Outcome::TimedOut => {
            panic!("fuzz: зависание >2с на программе:\n{source}");
        }
        Outcome::Exited { code, stderr } => {
            assert!(
                !stderr.contains("panicked at"),
                "fuzz: panic на программе:\n{source}\nstderr:\n{stderr}"
            );
            if code == 0 {
                return;
            }
            assert_eq!(
                code, 1,
                "fuzz: недопустимый код {code} (ожидался 0/1) на программе:\n{source}\nstderr:\n{stderr}"
            );
            let trimmed = stderr.trim();
            assert!(
                trimmed.starts_with('{') && trimmed.ends_with('}'),
                "fuzz: stderr не JSON на программе:\n{source}\nstderr:\n{stderr}"
            );
            assert!(
                trimmed.contains("\"code\":\""),
                "fuzz: в JSON нет code на программе:\n{source}\nstderr:\n{stderr}"
            );
        }
    }
}

#[test]
fn fuzz_smoke_2000_random_programs() {
    let path = temp_file();
    let mut rng = Rng::new(0x5EED_0F00_D1D1_u64);
    for iteration in 0..ITERATIONS {
        let source = generate(&mut rng);
        std::fs::write(&path, &source).expect("write fuzz program");
        let outcome = run_with_timeout(&path);
        assert_valid_outcome(&source, outcome);
        let _ = iteration;
    }
}
