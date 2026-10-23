//! LiF фаза 2: fuzz-smoke интерпретатора (этап F1).
//!
//! Генерирует структурированные программы (random, edge_cases, type_mix,
//! modules, strings) и запускает каждую с таймаутом 2 секунды. Количество
//! программ задаётся `LUPUS_FUZZ_N` (по умолчанию 10 000). Допустимые исходы:
//! код 0 или 1 с JSON-ошибкой в stderr. Panic, сигнал, код 101 или зависание —
//! провал.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const DEFAULT_ITERATIONS: usize = 10_000;
const TIMEOUT: Duration = Duration::from_secs(2);
const GENERATORS: [&str; 5] = ["random", "edge_cases", "type_mix", "modules", "strings"];

fn temp_file() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lupus_fuzz_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.join("fuzz.lupus")
}

/// Количество fuzz-программ: `LUPUS_FUZZ_N` или 10 000 по умолчанию.
fn iterations() -> usize {
    std::env::var("LUPUS_FUZZ_N")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_ITERATIONS)
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

fn generate_random(rng: &mut Rng) -> String {
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

/// Граничные литералы, глубокая вложенность, смешение типов и рекурсия.
fn generate_edge_cases(rng: &mut Rng) -> String {
    if rng.chance(2) {
        return generate_recursion(rng);
    }
    match rng.below(5) {
        0 => {
            let max = "9223372036854775807";
            let min = "-9223372036854775808";
            match rng.below(6) {
                0 => format!("(print {max})\n(print {min})\n"),
                1 => format!("(+ {max} 1)\n"),
                2 => format!("(- {min} 1)\n"),
                3 => format!("(* {max} 2)\n"),
                4 => format!("(- {min})\n"),
                _ => format!("(pow {max} 2)\n"),
            }
        }
        1 => {
            let forms = [
                "(print 1e308)",
                "(print -1e308)",
                "(print (* 1e308 10.0))",
                "(print (/ 1e308 1e-308))",
                "(print (+ 1e308 1e308))",
                "(print (int->float 9223372036854775807))",
            ];
            forms[rng.below(forms.len())].to_string() + "\n"
        }
        2 => {
            let depth = 990 + rng.below(16);
            format!(
                "{}{}{}\n",
                "(".repeat(depth),
                "(print 1)",
                ")".repeat(depth)
            )
        }
        3 => {
            let values = [
                "1",
                "1.5",
                "\"str\"",
                "(list 1 2)",
                "true",
                "none",
                "9223372036854775807",
                "-9223372036854775808",
            ];
            let ops = [
                "+",
                "-",
                "*",
                "/",
                "%",
                "=",
                "string-append",
                "list",
                "nth",
                "contains?",
            ];
            let op = ops[rng.below(ops.len())];
            let a = values[rng.below(values.len())];
            let b = values[rng.below(values.len())];
            format!("(print ({op} {a} {b}))\n")
        }
        _ => format!(
            "(print (list {} {} {}))\n",
            rng.below(1000),
            rng.below(1000) as f64 / 10.0,
            ["\"x\"", "true", "none"][rng.below(3)]
        ),
    }
}

fn generate_recursion(rng: &mut Rng) -> String {
    match rng.below(3) {
        0 => "(define (f n) (f (+ n 1)))\n(f 0)\n".to_string(),
        1 => "(define (f n) (if (= n 0) 0 (f (- n 1))))\n(f 10001)\n".to_string(),
        _ => "(define (f n) (g n))\n(define (g n) (f n))\n(f 0)\n".to_string(),
    }
}

/// Намеренное смешивание int/float/str/list для type-mismatch.
fn generate_type_mix(rng: &mut Rng) -> String {
    let values = [
        "1",
        "1.5",
        "\"str\"",
        "(list 1 2)",
        "true",
        "none",
        "9223372036854775807",
        "-9223372036854775808",
    ];
    let ops = [
        "+",
        "-",
        "*",
        "/",
        "%",
        "<",
        "=",
        "string-append",
        "string-length",
        "list",
        "nth",
        "contains?",
        "not",
    ];
    let mut forms = vec![
        "(print (+ 1 1.5))".to_string(),
        "(print (string-append \"a\" (list 1 2)))".to_string(),
    ];
    let extra = 1 + rng.below(3);
    for _ in 0..extra {
        let op = ops[rng.below(ops.len())];
        let a = values[rng.below(values.len())];
        let b = values[rng.below(values.len())];
        if op == "string-length" || op == "not" {
            forms.push(format!("(print ({op} {a}))"));
        } else {
            forms.push(format!("(print ({op} {a} {b}))"));
        }
    }
    forms.join("\n")
}

/// `(import senko)` и вызовы с граничными значениями.
fn generate_modules(rng: &mut Rng) -> String {
    let calls = [
        "(print (senko/sqrt 16.0))",
        "(print (senko/sqrt -1.0))",
        "(print (senko/sqrt 1e308))",
        "(print (senko/sqrt 1e-308))",
        "(print (senko/pow 2.0 8.0))",
        "(print (senko/pow 1e308 2.0))",
        "(print (senko/pow 1e308 0.5))",
        "(print (senko/pow -1e308 0.5))",
        "(print (senko/pow 0.0 -1.0))",
        "(print (senko/sqrt 1))",
    ];
    let mut forms = vec!["(import senko)".to_string()];
    let count = 1 + rng.below(3);
    for _ in 0..count {
        forms.push(calls[rng.below(calls.len())].to_string());
    }
    forms.join("\n")
}

/// Строки с emoji, кириллицей и escape-последовательностями.
fn generate_strings(rng: &mut Rng) -> String {
    let strings = [
        "\"\"",
        "\"🐺 Волк\"",
        "\"Привет, мир!\"",
        "\"a\\nb\"",
        "\"tab\\there\"",
        "\"quote\\\"x\\\"\"",
        "\"backslash\\\\\"",
        "\"тест\"",
        "\"hello🐺\"",
        "\"\\u0432\\u043e\\u043b\\u043a\"",
    ];
    let value = strings[rng.below(strings.len())];
    let mut forms = Vec::new();
    let count = 1 + rng.below(3);
    for _ in 0..count {
        let expr = match rng.below(5) {
            0 => format!("(print {value})"),
            1 => format!("(print (string-length {value}))"),
            2 => format!("(print (string-reverse {value}))"),
            3 => format!("(print (string-append {value} {value}))"),
            _ => format!("(print (string-split {value} \" \"))"),
        };
        forms.push(expr);
    }
    forms.join("\n")
}

fn generate_for(index: usize, rng: &mut Rng) -> String {
    match index {
        0 => generate_random(rng),
        1 => generate_edge_cases(rng),
        2 => generate_type_mix(rng),
        3 => generate_modules(rng),
        _ => generate_strings(rng),
    }
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

/// Минимальное JSON-значение для разбора stderr без внешних зависимостей.
#[derive(Debug)]
enum JsonValue {
    Null,
    Bool,
    Number,
    Str(String),
    Array,
    Object(Vec<(String, JsonValue)>),
}

struct JsonParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        JsonParser { input, pos: 0 }
    }

    fn skip_ws(&mut self) {
        while self
            .input
            .as_bytes()
            .get(self.pos)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "ожидался байт {:?} в позиции {}",
                byte as char, self.pos
            ))
        }
    }

    fn parse(&mut self) -> Result<JsonValue, String> {
        self.skip_ws();
        let value = match self.peek() {
            Some(b'{') => self.object()?,
            Some(b'[') => self.array()?,
            Some(b'"') => JsonValue::Str(self.string()?),
            Some(b't') => {
                self.literal("true")?;
                JsonValue::Bool
            }
            Some(b'f') => {
                self.literal("false")?;
                JsonValue::Bool
            }
            Some(b'n') => {
                self.literal("null")?;
                JsonValue::Null
            }
            Some(b'-' | b'0'..=b'9') => {
                self.number()?;
                JsonValue::Number
            }
            Some(other) => {
                return Err(format!(
                    "неожиданный байт {:?} в позиции {}",
                    other as char, self.pos
                ));
            }
            None => return Err("неожиданный конец JSON".to_string()),
        };
        self.skip_ws();
        Ok(value)
    }

    fn literal(&mut self, text: &str) -> Result<(), String> {
        if self.input[self.pos..].starts_with(text) {
            self.pos += text.len();
            Ok(())
        } else {
            Err(format!("ожидался литерал {text} в позиции {}", self.pos))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let rest = &self.input[self.pos..];
            let ch = rest
                .chars()
                .next()
                .ok_or_else(|| "незавершённая строка JSON".to_string())?;
            self.pos += ch.len_utf8();
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let escape = self
                        .input
                        .as_bytes()
                        .get(self.pos)
                        .copied()
                        .ok_or_else(|| "незавершённый escape JSON".to_string())?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let end = self.pos + 4;
                            let hex = self
                                .input
                                .get(self.pos..end)
                                .ok_or_else(|| "короткий \\u escape".to_string())?;
                            let code = u32::from_str_radix(hex, 16)
                                .map_err(|_| format!("неверный \\u escape: {hex}"))?;
                            let decoded = char::from_u32(code)
                                .ok_or_else(|| format!("недопустимый код \\u: {code}"))?;
                            out.push(decoded);
                            self.pos = end;
                        }
                        other => {
                            return Err(format!("неизвестный escape \\{}", other as char));
                        }
                    }
                }
                c if (c as u32) < 0x20 => {
                    return Err(format!("управляющий символ в строке JSON: {c:?}"));
                }
                c => out.push(c),
            }
        }
    }

    fn number(&mut self) -> Result<(), String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        let mut digits = 0;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.pos += 1;
            digits += 1;
        }
        if digits == 0 {
            return Err(format!("ожидалась цифра в позиции {start}"));
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            let mut frac = 0;
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.pos += 1;
                frac += 1;
            }
            if frac == 0 {
                return Err(format!("ожидалась дробная часть в позиции {}", self.pos));
            }
        }
        if self.peek().is_some_and(|b| b == b'e' || b == b'E') {
            self.pos += 1;
            if self.peek().is_some_and(|b| b == b'+' || b == b'-') {
                self.pos += 1;
            }
            let mut exp = 0;
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.pos += 1;
                exp += 1;
            }
            if exp == 0 {
                return Err(format!("ожидалась экспонента в позиции {}", self.pos));
            }
        }
        self.input[start..self.pos]
            .parse::<f64>()
            .map_err(|_| format!("неверное число JSON: {}", &self.input[start..self.pos]))?;
        Ok(())
    }

    fn array(&mut self) -> Result<JsonValue, String> {
        self.expect(b'[')?;
        self.skip_ws();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonValue::Array);
        }
        loop {
            items.push(self.parse()?);
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonValue::Array);
                }
                _ => return Err(format!("ожидалась ',' или ']' в позиции {}", self.pos)),
            }
        }
    }

    fn object(&mut self) -> Result<JsonValue, String> {
        self.expect(b'{')?;
        self.skip_ws();
        let mut entries = Vec::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonValue::Object(entries));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            let value = self.parse()?;
            entries.push((key, value));
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonValue::Object(entries));
                }
                _ => return Err(format!("ожидалась ',' или '}}' в позиции {}", self.pos)),
            }
        }
    }
}

fn parse_json(input: &str) -> Result<JsonValue, String> {
    let mut parser = JsonParser::new(input);
    let value = parser.parse()?;
    if parser.pos != input.len() {
        return Err(format!("лишние данные после JSON в позиции {}", parser.pos));
    }
    Ok(value)
}

fn field<'a>(object: &'a [(String, JsonValue)], name: &str) -> Result<&'a str, String> {
    match object.iter().find(|(key, _)| key == name) {
        Some((_, JsonValue::Str(value))) if !value.is_empty() => Ok(value),
        Some((_, _)) => Err(format!("поле {name} не является непустой строкой")),
        None => Err(format!("в JSON нет поля {name}")),
    }
}

fn validate_error_json(stderr: &str) -> Result<(), String> {
    let JsonValue::Object(object) = parse_json(stderr.trim())? else {
        return Err("stderr не является JSON-объектом".to_string());
    };
    let severity = field(&object, "severity")?;
    field(&object, "phase")?;
    field(&object, "code")?;
    field(&object, "message")?;
    if severity != "error" {
        return Err(format!("severity = {severity:?}, ожидалось \"error\""));
    }
    Ok(())
}

#[derive(Default)]
struct FuzzStats {
    programs: usize,
    exit0: usize,
    exit1: usize,
    panics: usize,
    abnormal: usize,
    timeouts: usize,
    invalid_json: usize,
    per_generator: [usize; GENERATORS.len()],
    failures: Vec<String>,
}

fn short_source(source: &str) -> String {
    let mut short: String = source.chars().take(300).collect();
    if source.chars().count() > 300 {
        short.push_str("...");
    }
    short
}

impl FuzzStats {
    fn record(&mut self, source: &str, generator: &str, outcome: Outcome) {
        match outcome {
            Outcome::TimedOut => {
                self.timeouts += 1;
                self.failures.push(format!(
                    "[{generator}] зависание >2с:\n{}",
                    short_source(source)
                ));
            }
            Outcome::Exited { code, stderr } => match code {
                0 => self.exit0 += 1,
                1 => {
                    self.exit1 += 1;
                    if let Err(error) = validate_error_json(&stderr) {
                        self.invalid_json += 1;
                        self.failures.push(format!(
                            "[{generator}] некорректный JSON ошибки: {error}\nprogram:\n{}\nstderr:\n{stderr}",
                            short_source(source)
                        ));
                    }
                }
                101 => {
                    self.panics += 1;
                    self.failures.push(format!(
                        "[{generator}] panic/exit 101:\nprogram:\n{}\nstderr:\n{stderr}",
                        short_source(source)
                    ));
                }
                _ => {
                    self.abnormal += 1;
                    self.failures.push(format!(
                        "[{generator}] аварийный код {code} (ожидался 0/1):\nprogram:\n{}\nstderr:\n{stderr}",
                        short_source(source)
                    ));
                }
            },
        }
    }
}

#[test]
fn fuzz_smoke_structured_generators() {
    let total = iterations();
    let path = temp_file();
    let mut rng = Rng::new(0x5EED_0F00_D1D1_u64);
    let mut stats = FuzzStats::default();
    let started = Instant::now();

    for iteration in 0..total {
        let index = iteration % GENERATORS.len();
        let generator = GENERATORS[index];
        let source = generate_for(index, &mut rng);
        stats.programs += 1;
        stats.per_generator[index] += 1;
        std::fs::write(&path, &source).expect("write fuzz program");
        let outcome = run_with_timeout(&path);
        stats.record(&source, generator, outcome);
    }

    let elapsed = started.elapsed();
    let distribution = GENERATORS
        .iter()
        .zip(stats.per_generator.iter())
        .map(|(name, count)| format!("{name}:{count}"))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "FUZZ_REPORT programs={} elapsed_ms={} generators={} exit0={} exit1={} panics={} abnormal={} timeouts={} invalid_json={}",
        stats.programs,
        elapsed.as_millis(),
        distribution,
        stats.exit0,
        stats.exit1,
        stats.panics,
        stats.abnormal,
        stats.timeouts,
        stats.invalid_json
    );

    if !stats.failures.is_empty() {
        let shown = stats
            .failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n");
        panic!(
            "fuzz: ошибок {} (показаны первые 10):\n{}",
            stats.failures.len(),
            shown
        );
    }
}

#[test]
fn json_parser_reads_error_object() {
    let json = r#"{"severity":"error","phase":"runtime","code":"x","message":"a\"b\n"}"#;
    assert!(validate_error_json(json).is_ok());
    assert!(validate_error_json(r#"{"severity":"error"}"#).is_err());
}
