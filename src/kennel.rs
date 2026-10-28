//! Публичный host-API встраивания интерпретатора (kennel).
//!
//! Модуль предназначен для программ на Rust, которые запускают код Lupus
//! внутри своего процесса. Он даёт три вещи, которых нет в CLI:
//! профиль возможностей ([`Profile`]), лимиты выполнения ([`Config`]) и
//! регистрацию нативных функций хоста ([`Host`]).
//!
//! ```no_run
//! use lupus::kennel::{self, Config, Host, Profile};
//! use lupus::value::{Arity, Value};
//!
//! # fn main() -> Result<(), kennel::JsonError> {
//! let mut host = Host::new();
//! host.register_native("double", Arity::Exact(1), |args| match &args[0] {
//!     Value::Int(value) => Ok(Value::Int(value * 2)),
//!     _ => Err("host/double ожидает int".to_string()),
//! });
//!
//! let config = Config {
//!     profile: Profile::Full,
//!     fuel: Some(10_000),
//!     ..Config::default()
//! };
//! let output = kennel::run_program("(print (host/double 21))", &config, &host)?;
//! assert_eq!(output, "42\n");
//! # Ok(())
//! # }
//! ```

use crate::error::Error;
use crate::interpreter::Interpreter;
use crate::json::Json;
use crate::value::{Arity, HostFn};
use std::collections::HashMap;
use std::fmt;

/// Профиль доступных возможностей интерпретатора.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    /// Только core-биндинги; любые импорты запрещены.
    Pure,
    /// Core + нативный пакет `kaltsit`; файловые модули разрешены.
    Core,
    /// Все нативные пакеты и host-нативы. Профиль CLI по умолчанию.
    #[default]
    Full,
}

impl Profile {
    /// Строковое имя профиля для диагностик и документации.
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Pure => "pure",
            Profile::Core => "core",
            Profile::Full => "full",
        }
    }

    /// Разрешён ли импорт указанного пакета/модуля.
    pub(crate) fn allows_import(self, package: &str, is_native: bool) -> bool {
        match self {
            Profile::Pure => false,
            Profile::Core => !is_native || package == "kaltsit",
            Profile::Full => true,
        }
    }

    /// Разрешены ли host-нативы (`host/<имя>`).
    pub(crate) fn allows_host(self) -> bool {
        self == Profile::Full
    }
}

/// Настройки запуска программы через [`run_program`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Config {
    /// Профиль возможностей. По умолчанию [`Profile::Full`] (как у CLI).
    pub profile: Profile,
    /// Начальный лимит шагов. Синоним [`Config::fuel`]; при задании обоих
    /// действует меньшее значение.
    pub max_steps: Option<u64>,
    /// Приблизительный лимит живых/созданных `Value`.
    pub max_memory_values: Option<u64>,
    /// Максимальная глубина вызовов пользовательских функций.
    pub max_depth: Option<u32>,
    /// Начальный бюджет шагов. Может только уменьшаться формой `set-fuel!`.
    pub fuel: Option<u64>,
}

impl Config {
    /// Эффективный стартовый бюджет шагов.
    ///
    /// `fuel` и `max_steps` — два имени одного лимита; если заданы оба,
    /// выбирается более строгий.
    pub(crate) fn effective_fuel(self) -> Option<u64> {
        match (self.fuel, self.max_steps) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(value), None) | (None, Some(value)) => Some(value),
            (None, None) => None,
        }
    }
}

/// Нативная функция хоста.
///
/// Ошибка `Err(String)` становится runtime-диагностикой `host-error`.
type NativeEntry = (Arity, HostFn);

/// Реестр нативных функций, предоставленных встраивающим приложением.
#[derive(Clone, Default)]
pub struct Host {
    natives: HashMap<String, NativeEntry>,
}

impl Host {
    /// Создаёт пустой реестр.
    pub fn new() -> Self {
        Host::default()
    }

    /// Регистрирует функцию, доступную из Lupus как `host/<имя>`.
    ///
    /// Префикс `host/` в имени допускается и нормализуется. Повторная
    /// регистрация заменяет предыдущую функцию.
    pub fn register_native(&mut self, name: impl Into<String>, arity: Arity, function: HostFn) {
        let raw = name.into();
        let name = raw
            .strip_prefix("host/")
            .unwrap_or(raw.as_str())
            .to_string();
        self.natives.insert(name, (arity, function));
    }

    /// Возвращает `true`, если функция с таким именем зарегистрирована.
    pub fn has_native(&self, name: &str) -> bool {
        self.natives
            .contains_key(name.strip_prefix("host/").unwrap_or(name))
    }

    /// Арность и реализация зарегистрированной функции.
    pub(crate) fn resolve(&self, name: &str) -> Option<NativeEntry> {
        self.natives.get(name).copied()
    }
}

impl fmt::Debug for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut names: Vec<&str> = self.natives.keys().map(String::as_str).collect();
        names.sort_unstable();
        f.debug_struct("Host").field("natives", &names).finish()
    }
}

/// Ошибка встраивания, сериализуемая в строгий JSON-диагностический формат.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonError {
    /// Исходная диагностика интерпретатора.
    pub error: Error,
}

impl JsonError {
    /// Создаёт обёртку над диагностикой.
    pub fn new(error: Error) -> Self {
        JsonError { error }
    }

    /// Код диагностики (например, `exhausted` или `profile-denied`).
    pub fn code(&self) -> &str {
        &self.error.code
    }

    /// Человекочитаемое сообщение.
    pub fn message(&self) -> &str {
        &self.error.message
    }

    /// Строгий JSON диагностики.
    pub fn to_json(&self) -> String {
        self.error.to_json()
    }

    /// JSON-значение диагностики.
    pub fn to_json_value(&self) -> Json {
        self.error.to_json_value()
    }
}

impl From<Error> for JsonError {
    fn from(error: Error) -> Self {
        JsonError { error }
    }
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl std::error::Error for JsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl std::ops::Deref for JsonError {
    type Target = Error;

    fn deref(&self) -> &Self::Target {
        &self.error
    }
}

/// Запускает исходник Lupus в изолированном интерпретаторе.
///
/// Возвращает весь вывод `(print ...)` как строку. Диагностики (лексика,
/// синтаксис, runtime, лимиты) возвращаются как [`JsonError`]. Линт при
/// встраивании не запускается: это отдельная стадия CLI.
pub fn run_program(source: &str, config: &Config, host: &Host) -> Result<String, JsonError> {
    let tokens = crate::lexer::lex(source).map_err(JsonError::from)?;
    let forms = crate::parser::parse(tokens).map_err(JsonError::from)?;

    let mut interpreter = Interpreter::new();
    interpreter.apply_config(config, host);
    interpreter.set_capture_output(true);
    interpreter.run(forms).map_err(JsonError::from)?;
    Ok(interpreter.captured_output())
}
