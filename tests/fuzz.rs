//! LiF фаза 2: fuzz-smoke интерпретатора (этап F1, V0.7 этап 1).
//!
//! Программы строит grammar-based генератор [`lupus::generator`]: он соблюдает
//! БНФ спеки, баланс скобок и лимит вложенности, а также использует только
//! реально существующий API ядра. Количество программ задаётся
//! `LUPUS_FUZZ_N` (по умолчанию 10 000). Допустимые исходы: код 0 или 1 с
//! JSON-ошибкой в stderr. Panic, сигнал, код 101 или зависание — провал.
//!
//! Дополнительно проверяются валидность (≥ 90% exit 0), баланс скобок и
//! соблюдение лимита глубины.

use lupus::generator::{is_balanced, max_nesting_depth, Generator};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const DEFAULT_ITERATIONS: usize = 10_000;
const TIMEOUT: Duration = Duration::from_secs(2);
const GENERATORS: [&str; 1] = ["grammar"];
const MIN_VALID_PERCENT: usize = 90;

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

/// Запускает программу так же, как CLI: парсинг, линт, интерпретация.
fn run_in_process(source: &str) -> Result<(), String> {
    let forms = lupus::parse_source(source).map_err(|error| error.to_json())?;
    lupus::lint::check(&forms, None).map_err(|error| error.to_json())?;
    lupus::run_source(source).map_err(|error| error.to_json())
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
    let mut generator = Generator::new(0x5EED_0F00_D1D1_u64);
    let mut stats = FuzzStats::default();
    let started = Instant::now();

    for iteration in 0..total {
        let index = iteration % GENERATORS.len();
        let name = GENERATORS[index];
        let source = generator.generate();
        stats.programs += 1;
        stats.per_generator[index] += 1;
        std::fs::write(&path, &source).expect("write fuzz program");
        let outcome = run_with_timeout(&path);
        stats.record(&source, name, outcome);
    }

    let elapsed = started.elapsed();
    let distribution = GENERATORS
        .iter()
        .zip(stats.per_generator.iter())
        .map(|(name, count)| format!("{name}:{count}"))
        .collect::<Vec<_>>()
        .join(",");
    let valid_percent = stats.exit0 * 100 / stats.programs;
    println!(
        "FUZZ_REPORT programs={} elapsed_ms={} generators={} exit0={} exit1={} valid_percent={} panics={} abnormal={} timeouts={} invalid_json={}",
        stats.programs,
        elapsed.as_millis(),
        distribution,
        stats.exit0,
        stats.exit1,
        valid_percent,
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

    assert!(
        valid_percent >= MIN_VALID_PERCENT,
        "fuzz: валидных программ {valid_percent}% < {MIN_VALID_PERCENT}%"
    );
}

#[test]
fn test_generator_validity() {
    let total = iterations();
    let mut generator = Generator::new(0x0BAD_C0DE_5EED_u64);
    let mut valid = 0usize;
    let mut failures = Vec::new();

    for iteration in 0..total {
        let source = generator.generate();
        match run_in_process(&source) {
            Ok(()) => valid += 1,
            Err(error) => {
                if failures.len() < 10 {
                    failures.push(format!(
                        "#{iteration}: {error}\nprogram:\n{}",
                        short_source(&source)
                    ));
                }
            }
        }
    }

    let valid_percent = valid * 100 / total;
    println!(
        "GENERATOR_VALIDITY programs={total} exit0={valid} exit1={} valid_percent={valid_percent}",
        total - valid
    );
    assert!(
        valid_percent >= MIN_VALID_PERCENT,
        "генератор: валидных программ {valid_percent}% < {MIN_VALID_PERCENT}%\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn test_generator_balance() {
    let mut generator = Generator::new(0xBA1A_5CED_u64);
    for iteration in 0..2_000 {
        let source = generator.generate();
        assert!(
            is_balanced(&source),
            "программа #{iteration}: несбалансированные скобки:\n{}",
            short_source(&source)
        );
    }
}

#[test]
fn test_generator_depth() {
    for depth in 1..=lupus::generator::MAX_DEPTH {
        let mut generator = Generator::with_depth(0xDEEB_0000 + depth as u64, depth);
        for iteration in 0..500 {
            let source = generator.generate();
            let actual = max_nesting_depth(&source);
            assert!(
                actual <= depth,
                "программа #{iteration} при лимите {depth}: глубина {actual}\n{}",
                short_source(&source)
            );
        }
    }
}

#[test]
fn json_parser_reads_error_object() {
    let json = r#"{"severity":"error","phase":"runtime","code":"x","message":"a\"b\n"}"#;
    assert!(validate_error_json(json).is_ok());
    assert!(validate_error_json(r#"{"severity":"error"}"#).is_err());
}
