//! Типы ошибок и диагностик интерпретатора Lupus.
//!
//! Все ошибки (лексические, синтаксические, runtime, линта) представляются
//! единым типом [`Error`] и сериализуются в строгий JSON по спецификации §6.1.
//! Интерпретатор никогда не паникует: любая ошибка возвращается через
//! `Result<T, Error>`.

use crate::json::Json;
use std::fmt;

/// Важность диагностики.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Информационное сообщение.
    Info,
    /// Предупреждение.
    Warning,
    /// Ошибка.
    Error,
}

impl Severity {
    /// Строковое представление для JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// Фаза, в которой возникла диагностика.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Лексический анализ.
    Lex,
    /// Синтаксический анализ.
    Parse,
    /// Типовая проверка (в V0.5 — только синтаксические проверки аннотаций).
    Type,
    /// Линт.
    Lint,
    /// Выполнение программы.
    Runtime,
    /// FFI (объявлен заменённым, коды не выдаются в V0.5).
    Ffi,
}

impl Phase {
    /// Строковое представление для JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Lex => "lex",
            Phase::Parse => "parse",
            Phase::Type => "type",
            Phase::Lint => "lint",
            Phase::Runtime => "runtime",
            Phase::Ffi => "ffi",
        }
    }
}

/// Байтовый span в исходном тексте.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// Начальный байтовый offset (включительно).
    pub start: usize,
    /// Конечный байтовый offset (исключительно).
    pub end: usize,
}

/// Позиция ошибки в исходном коде.
///
/// Для runtime/lint-ошибок location равен `None` (span'ы в AST появятся
/// в V0.6 — см. `docs/ERRATA.md`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Location {
    /// Путь к файлу, если известен.
    pub file: Option<String>,
    /// Номер строки (1-based).
    pub line: usize,
    /// Номер колонки (1-based).
    pub col: usize,
    /// Байтовый span, если известен.
    pub span: Option<Span>,
}

/// Дополнительный контекст диагностики.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Context {
    /// Текст строки исходника.
    pub line_text: Option<String>,
    /// Проблемный токен.
    pub token: Option<String>,
}

/// Единый тип ошибки/диагностики интерпретатора Lupus.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    /// Важность диагностики.
    pub severity: Severity,
    /// Фаза, в которой возникла диагностика.
    pub phase: Phase,
    /// Уникальный код из каталога §6.2 (и расширений из ERRATA).
    pub code: String,
    /// Человекочитаемое сообщение (на русском языке).
    pub message: String,
    /// Позиция в исходном коде (только lex/parse).
    pub location: Option<Box<Location>>,
    /// Подсказка по исправлению.
    pub hint: Option<String>,
    /// Дополнительный контекст.
    pub context: Option<Box<Context>>,
}

impl Error {
    /// Создаёт runtime-ошибку, подбирая код по тексту сообщения.
    ///
    /// Старые места вызова (V0.4) не передавали код; для обратной
    /// совместимости классификатор ниже сопоставляет типовые сообщения
    /// каталогу §6.2. Новый код должен использовать явные хелперы
    /// [`Error::runtime`], [`Error::arity`] и т.п.
    pub fn new(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = infer_code(&message);
        Error::runtime(code, message)
    }

    /// Создаёт диагностику с произвольными полями.
    pub fn diagnostic(
        severity: Severity,
        phase: Phase,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Error {
            severity,
            phase,
            code: code.into(),
            message: message.into(),
            location: None,
            hint: None,
            context: None,
        }
    }

    /// Runtime-ошибка с заданным кодом.
    pub fn runtime(code: impl Into<String>, message: impl Into<String>) -> Self {
        Error::diagnostic(Severity::Error, Phase::Runtime, code, message)
    }

    /// Линт-ошибка с заданным кодом.
    pub fn lint(code: impl Into<String>, message: impl Into<String>) -> Self {
        Error::diagnostic(Severity::Error, Phase::Lint, code, message)
    }

    /// Ошибка фазы типов (синтаксические проверки аннотаций).
    pub fn type_error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Error::diagnostic(Severity::Error, Phase::Type, code, message)
    }

    /// Лексическая ошибка с позицией.
    pub fn lex(
        code: impl Into<String>,
        message: impl Into<String>,
        line: usize,
        col: usize,
    ) -> Self {
        let mut error = Error::diagnostic(Severity::Error, Phase::Lex, code, message);
        error.location = Some(Box::new(Location {
            file: None,
            line,
            col,
            span: None,
        }));
        error
    }

    /// Синтаксическая ошибка с позицией.
    pub fn parse(
        code: impl Into<String>,
        message: impl Into<String>,
        line: usize,
        col: usize,
    ) -> Self {
        let mut error = Error::diagnostic(Severity::Error, Phase::Parse, code, message);
        error.location = Some(Box::new(Location {
            file: None,
            line,
            col,
            span: None,
        }));
        error
    }

    /// Синтаксическая ошибка с позицией и байтовым span.
    pub fn parse_with_span(
        code: impl Into<String>,
        message: impl Into<String>,
        line: usize,
        col: usize,
        span: Span,
    ) -> Self {
        let mut error = Error::parse(code, message, line, col);
        if let Some(location) = error.location.as_mut() {
            location.span = Some(span);
        }
        error
    }

    /// Ошибка неверной арности.
    pub fn arity(message: impl Into<String>) -> Self {
        Error::runtime("arity-mismatch", message)
    }

    /// Ошибка несовместимого типа значения.
    pub fn type_mismatch(message: impl Into<String>) -> Self {
        Error::runtime("type-mismatch", message)
    }

    /// Неизвестный идентификатор.
    pub fn unknown_identifier(message: impl Into<String>) -> Self {
        Error::runtime("unknown-identifier", message)
    }

    /// Добавляет подсказку.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Добавляет текст исходной строки в контекст.
    pub fn with_line_text(mut self, text: impl Into<String>) -> Self {
        let context = self
            .context
            .get_or_insert_with(|| Box::new(Context::default()));
        context.line_text = Some(text.into());
        self
    }

    /// Добавляет проблемный токен в контекст.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        let context = self
            .context
            .get_or_insert_with(|| Box::new(Context::default()));
        context.token = Some(token.into());
        self
    }

    /// Прикрепляет путь к файлу к location (если location уже есть).
    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        let file = file.into();
        match &mut self.location {
            Some(location) => {
                if location.file.is_none() {
                    location.file = Some(file);
                }
            }
            None => {
                self.location = Some(Box::new(Location {
                    file: Some(file),
                    line: 0,
                    col: 0,
                    span: None,
                }));
            }
        }
        self
    }

    /// Прикрепляет путь к файлу, не создавая location для runtime-ошибок.
    ///
    /// Используется CLI: runtime/lint-ошибки по ERRATA имеют `location = null`,
    /// даже если известно имя файла.
    pub fn with_file_if_located(mut self, file: impl Into<String>) -> Self {
        if let Some(location) = &mut self.location {
            if location.file.is_none() {
                location.file = Some(file.into());
            }
        }
        self
    }

    /// Сериализует диагностику в строгий JSON (одна строка).
    pub fn to_json(&self) -> String {
        self.to_json_value().to_string()
    }

    /// Представление диагностики в виде JSON-значения.
    pub fn to_json_value(&self) -> Json {
        let mut object = vec![
            ("severity".to_string(), Json::str(self.severity.as_str())),
            ("phase".to_string(), Json::str(self.phase.as_str())),
            ("code".to_string(), Json::str(self.code.clone())),
            ("message".to_string(), Json::str(self.message.clone())),
        ];

        let location = match &self.location {
            None => Json::Null,
            Some(location) => {
                let span = match location.span {
                    None => Json::Null,
                    Some(span) => Json::Object(vec![
                        ("start".to_string(), Json::Int(span.start as i64)),
                        ("end".to_string(), Json::Int(span.end as i64)),
                    ]),
                };
                Json::Object(vec![
                    (
                        "file".to_string(),
                        location.file.clone().map(Json::str).unwrap_or(Json::Null),
                    ),
                    ("line".to_string(), Json::Int(location.line as i64)),
                    ("col".to_string(), Json::Int(location.col as i64)),
                    ("span".to_string(), span),
                ])
            }
        };
        object.push(("location".to_string(), location));

        object.push((
            "hint".to_string(),
            self.hint.clone().map(Json::str).unwrap_or(Json::Null),
        ));

        let context = match &self.context {
            None => Json::Null,
            Some(context) => Json::Object(vec![
                (
                    "line_text".to_string(),
                    context
                        .line_text
                        .clone()
                        .map(Json::str)
                        .unwrap_or(Json::Null),
                ),
                (
                    "token".to_string(),
                    context.token.clone().map(Json::str).unwrap_or(Json::Null),
                ),
            ]),
        };
        object.push(("context".to_string(), context));

        Json::Object(object)
    }

    /// Человекочитаемый рендер (для флага `--pretty`).
    pub fn to_pretty(&self) -> String {
        let mut out = format!(
            "{}[{}] {}: {}",
            self.severity.as_str(),
            self.phase.as_str(),
            self.code,
            self.message
        );
        if let Some(location) = &self.location {
            let file = location.file.as_deref().unwrap_or("<input>");
            out.push_str(&format!(
                "\n  --> {}:{}:{}",
                file, location.line, location.col
            ));
            if let Some(context) = &self.context {
                if let Some(line_text) = &context.line_text {
                    out.push_str(&format!("\n  | {}", line_text));
                    if location.col > 0 {
                        out.push_str(&format!(
                            "\n  | {}^",
                            " ".repeat(location.col.saturating_sub(1))
                        ));
                    }
                }
            }
        }
        if let Some(hint) = &self.hint {
            out.push_str(&format!("\n  hint: {}", hint));
        }
        out
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Error {}

/// Классифицирует сообщение старого образца по каталогу кодов §6.2.
fn infer_code(message: &str) -> &'static str {
    if message.contains("Индекс вне диапазона") {
        return "index-out-of-bounds";
    }
    if message.contains("Деление на ноль") {
        return "divide-by-zero";
    }
    if message.contains("не изменяемая") {
        return "immutable-assignment";
    }
    if message.contains("Assert не выполнен") {
        return "assert-failed";
    }
    if message.contains("ни одна ветка") {
        return "match-non-exhaustive";
    }
    if message.contains("не определена") || message.contains("не определён")
    {
        return "unknown-identifier";
    }
    if message.contains("duplicate-struct-field") {
        return "duplicate-struct-field";
    }
    let arity_words =
        message.contains("аргумент") || message.contains("элементов") || message.contains("полей");
    if arity_words
        && (message.contains("получено")
            || message.contains("ровно")
            || message.contains("минимум")
            || message.contains("ожидает"))
    {
        return "arity-mismatch";
    }
    "type-mismatch"
}
