//! Интерпретатор языка Lupus (tree-walk).
//!
//! Обходит AST и вычисляет формы. Содержит логику специальных форм
//! (`define`, `define-mutable`, `set!`, `if`, `while`, `begin`, `and`, `or`,
//! `lambda`) и вызов встроенных / пользовательских функций.

use crate::ast::{Expr, ExprKind};
use crate::builtins;
use crate::environment::Environment;
use crate::error::Error;
use crate::gc;
use crate::lexer::{Token, TokenKind};
use crate::modules;
use crate::native;
use crate::value::{is_truthy, Arity, BuiltinImpl, Closure, NativeImpl, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

/// Тип поля структуры, используемый конструктором для проверки аргументов.
#[derive(Debug, Clone, PartialEq)]
enum FieldType {
    /// Типовой параметр дженерика: принимает значение любого типа.
    Param,
    /// Именованный тип (`int`, `float`, `bool`, `str`, `unit`, `none`).
    Named(String),
    /// Параметризованный тип (`(List a)`, `(Option a)`, `(Map k v)`).
    Applied(String, Vec<FieldType>),
}

impl FieldType {
    /// Разбирает аннотацию поля; имена типовых параметров становятся `Param`.
    fn from_expr(expr: &Expr, type_params: &[String]) -> Self {
        match &expr.kind {
            ExprKind::None => Self::Named("none".to_string()),
            ExprKind::Symbol(name) if type_params.iter().any(|param| param == name) => Self::Param,
            ExprKind::Symbol(name) => Self::Named(name.clone()),
            ExprKind::List(items) => match items.first() {
                Some(Expr {
                    kind: ExprKind::Symbol(name),
                    ..
                }) => Self::Applied(
                    name.clone(),
                    items[1..]
                        .iter()
                        .map(|item| Self::from_expr(item, type_params))
                        .collect(),
                ),
                _ => Self::Param,
            },
            _ => Self::Param,
        }
    }

    /// Проверяет значение по объявленному типу.
    ///
    /// Неизвестные именованные типы не проверяются: в V0.5 нет HM-тайпчекера,
    /// и конструктор не должен отвергать пользовательские типы.
    fn accepts(&self, value: &Value) -> bool {
        match self {
            Self::Param => true,
            Self::Named(name) => match name.as_str() {
                "int" => matches!(value, Value::Int(_)),
                "float" => matches!(value, Value::Float(_)),
                "bool" => matches!(value, Value::Bool(_)),
                "str" => matches!(value, Value::Str(_)),
                "none" | "unit" => matches!(value, Value::None),
                _ => true,
            },
            Self::Applied(name, args) => match (name.as_str(), args.as_slice()) {
                ("List", [element]) => match value {
                    Value::List(items) => items.iter().all(|item| element.accepts(item)),
                    _ => false,
                },
                ("Option", [inner]) => match value {
                    Value::None => true,
                    Value::Some(item) => inner.accepts(item),
                    _ => false,
                },
                ("Result", [ok, err]) => match value {
                    Value::Success(item) => ok.accepts(item),
                    Value::Failure(item) => err.accepts(item),
                    _ => false,
                },
                ("Tuple", elements) => match value {
                    Value::Tuple(items) => {
                        items.len() == elements.len()
                            && items
                                .iter()
                                .zip(elements)
                                .all(|(item, element)| element.accepts(item))
                    }
                    _ => false,
                },
                ("Map", [key, val]) => match value {
                    Value::Map(pairs) => {
                        pairs.iter().all(|(k, v)| key.accepts(k) && val.accepts(v))
                    }
                    _ => false,
                },
                _ => true,
            },
        }
    }

    /// Представление типа для сообщения `type-mismatch`.
    fn display(&self) -> String {
        match self {
            Self::Param => "any".to_string(),
            Self::Named(name) => name.clone(),
            Self::Applied(name, args) => {
                let args = args
                    .iter()
                    .map(Self::display)
                    .collect::<Vec<String>>()
                    .join(" ");
                format!("({} {})", name, args)
            }
        }
    }
}

/// Описание структуры, объявленной через `defstruct`.
#[derive(Debug, Clone)]
pub struct StructDef {
    /// Имя типа (конструктора).
    pub name: String,
    /// Имена параметров-дженериков (не проверяются).
    pub type_params: Vec<String>,
    /// Имена полей в порядке объявления.
    pub fields: Vec<String>,
    /// Типы полей в порядке объявления.
    field_types: Vec<FieldType>,
}

/// Разобранный и загруженный тест.
#[derive(Debug, Clone)]
pub struct TestDef {
    /// Имя теста.
    pub name: String,
    /// Тело теста (последовательность выражений).
    pub body: Vec<Expr>,
}

/// Максимальная глубина вызовов пользовательских функций.
const MAX_CALL_DEPTH: u32 = 10_000;

/// Начальное состояние ГПСЧ `lappland/malice`.
///
/// `LUPUS_MALICE_SEED` принимает целое `u64` или произвольную строку
/// (хешируется FNV-1a); отсутствующая переменная заменяется энтропией
/// (время + pid). Ноль запрещён: xorshift с нулевым состоянием остаётся нулевым.
fn malice_seed() -> u64 {
    if let Ok(value) = std::env::var("LUPUS_MALICE_SEED") {
        return value
            .parse::<u64>()
            .map(|seed| seed.max(1))
            .unwrap_or_else(|_| {
                let mut hash = 0xcbf2_9ce4_8422_2325u64;
                for byte in value.as_bytes() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                }
                hash.max(1)
            });
    }

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    (nanos ^ (std::process::id() as u64).rotate_left(32)).max(1)
}

/// Результат выполнения одного теста.
#[derive(Debug, Clone)]
pub struct TestOutcome {
    /// Имя теста.
    pub name: String,
    /// Успешен ли тест.
    pub passed: bool,
    /// Ошибка (для проваленного теста).
    pub error: Option<Error>,
    /// Длительность в миллисекундах.
    pub duration_ms: f64,
}

/// Экспорты файлового модуля: имя → значение.
type ModuleExports = HashMap<String, Value>;

/// Кэш загруженных файловых модулей.
type ModuleCache = Rc<RefCell<HashMap<PathBuf, Rc<ModuleExports>>>>;

/// Импортированный модуль.
#[derive(Debug, Clone)]
enum ImportedModule {
    /// Нативный пакет.
    Native { package: String },
    /// Файловый модуль: только `define-public`.
    File(Rc<ModuleExports>),
}

/// Интерпретатор с глобальным окружением.
pub struct Interpreter {
    /// Глобальное окружение — точка отсчёта для поиска переменных.
    global: Rc<RefCell<Environment>>,
    /// Мягкий режим `--lenient-parens`: игнорировать лишние `)` вместо падения.
    lenient_parens: bool,
    /// Реестр объявленных структур: имя типа → описание.
    structs: Rc<RefCell<HashMap<String, StructDef>>>,
    /// Импортированные модули: префикс → модуль.
    imports: RefCell<HashMap<String, ImportedModule>>,
    /// Кэш файловых модулей по каноническому пути.
    module_cache: ModuleCache,
    /// Стек загрузки модулей (обнаружение циклов).
    module_stack: Rc<RefCell<Vec<PathBuf>>>,
    /// Каталог текущей программы (для поиска `stdlib/` рядом с файлом).
    program_dir: RefCell<Option<PathBuf>>,
    /// Коллектор экспортов загружаемого файлового модуля.
    exports: RefCell<Option<HashMap<String, Value>>>,
    /// Подавление `print` (режим `lupus test` без `--verbose`).
    suppress_print: Cell<bool>,
    /// Зарегистрированные тесты текущего файла.
    tests: RefCell<Vec<TestDef>>,
    /// Базовое окружение для запуска тестов (`w/run`, `w/run-all`).
    test_base_env: RefCell<Option<Rc<RefCell<Environment>>>>,
    /// Стек выполняющихся тестов (защита от рекурсии).
    running_tests: RefCell<Vec<String>>,
    /// Текущая глубина вызовов пользовательских функций/замыканий.
    depth: Rc<Cell<u32>>,
    /// Детерминированный ГПСЧ для `lappland/malice`.
    malice_rng: RefCell<u64>,
}

/// Уменьшает счётчик глубины при выходе из вызова (в том числе по ошибке).
struct DepthGuard<'a> {
    depth: &'a Cell<u32>,
}

impl Drop for DepthGuard<'_> {
    fn drop(&mut self) {
        self.depth.set(self.depth.get().saturating_sub(1));
    }
}

impl Interpreter {
    /// Создаёт новый интерпретатор с пустым глобальным окружением
    /// и выключенным lenient-режимом (по умолчанию).
    pub fn new() -> Self {
        Interpreter {
            global: Environment::new(),
            lenient_parens: false,
            structs: Rc::new(RefCell::new(HashMap::new())),
            imports: RefCell::new(HashMap::new()),
            module_cache: Rc::new(RefCell::new(HashMap::new())),
            module_stack: Rc::new(RefCell::new(Vec::new())),
            program_dir: RefCell::new(None),
            exports: RefCell::new(None),
            suppress_print: Cell::new(false),
            tests: RefCell::new(Vec::new()),
            test_base_env: RefCell::new(None),
            running_tests: RefCell::new(Vec::new()),
            depth: Rc::new(Cell::new(0)),
            malice_rng: RefCell::new(malice_seed()),
        }
    }

    /// Создаёт интерпретатор с явно заданным lenient-режимом.
    ///
    /// При `lenient = true` лишние закрывающие скобки `)` игнорируются
    /// (с предупреждением в stderr), интерпретатор не падает.
    pub fn with_lenient_parens(lenient: bool) -> Self {
        let mut interpreter = Interpreter::new();
        interpreter.lenient_parens = lenient;
        interpreter
    }

    /// Включает/выключает lenient-режим после создания интерпретатора.
    pub fn set_lenient_parens(&mut self, lenient: bool) {
        self.lenient_parens = lenient;
    }

    /// Включает/выключает подавление `print`.
    pub fn set_suppress_print(&mut self, suppress: bool) {
        self.suppress_print.set(suppress);
    }

    /// Задаёт каталог текущей программы (для поиска модулей рядом с файлом).
    pub fn set_program_dir(&mut self, dir: PathBuf) {
        *self.program_dir.borrow_mut() = Some(dir);
    }

    /// Возвращает копию shared-ссылки на глобальное окружение.
    fn global_env(&self) -> Rc<RefCell<Environment>> {
        self.global.clone()
    }

    /// Запускает сборку мусора вручную и возвращает статистику.
    pub fn gc_collect(&self) -> gc::GcStats {
        gc::collect()
    }

    /// Запускает сборку с обязательной строкой лога (для `gc-collect`).
    pub fn gc_collect_verbose(&self) -> gc::GcStats {
        gc::collect_verbose()
    }

    /// Текущее число аллокаций с прошлой сборки.
    pub fn gc_allocations(&self) -> usize {
        gc::allocations()
    }

    /// Псевдоним для API встраивания.
    pub fn collect_garbage(&self) -> gc::GcStats {
        gc::collect()
    }

    /// Бросает «монетку» для `lappland/malice`.
    ///
    /// При `probability <= 0.0` и `>= 1.0` генератор не расходуется: результат
    /// детерминирован без обращения к ГПСЧ. Иначе используется xorshift64*,
    /// инициализированный из `LUPUS_MALICE_SEED` или энтропии.
    pub(crate) fn net_malice_hits(&self, probability: f64) -> bool {
        if probability <= 0.0 {
            return false;
        }
        if probability >= 1.0 {
            return true;
        }
        let mut state = self.malice_rng.borrow_mut();
        let mut x = *state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        *state = x;
        let sample = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (sample as f64 / u64::MAX as f64) < probability
    }

    /// Выполняет AST-программу (список форм верхнего уровня).
    ///
    /// Формы `test` на верхнем уровне не выполняются (тесты запускает
    /// отдельная подкоманда `lupus test`).
    pub fn run(&mut self, forms: Vec<Expr>) -> Result<(), Error> {
        for form in forms {
            if is_test_form(&form) {
                continue;
            }
            self.eval(&form, self.global_env())?;
        }
        Ok(())
    }

    /// Лексирует, парсит и выполняет исходный код, возвращая значение
    /// последней формы. Состояние (глобальное окружение) сохраняется
    /// между вызовами.
    pub fn eval_source(&mut self, src: &str) -> Result<Value, Error> {
        let mut tokens = crate::lexer::lex(src)?;
        // В lenient-режиме лишние `)` убираются ещё до парсинга.
        if self.lenient_parens {
            tokens = sanitize_tokens_lenient(tokens);
        }
        let forms = crate::parser::parse(tokens)?;
        let mut last = Value::None;
        for form in &forms {
            last = self.eval(form, self.global_env())?;
        }
        Ok(last)
    }

    /// Оценка одного выражения в контексте окружения.
    ///
    /// Ошибка без готовой location получает span и позицию текущего узла.
    /// Это не перезаписывает location лексических/синтаксических ошибок.
    pub fn eval(&self, expr: &Expr, env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        let result = match &expr.kind {
            ExprKind::Number(n) => Ok(Value::Int(*n)),
            ExprKind::Float(f) => Ok(Value::Float(*f)),
            ExprKind::Str(s) => Ok(Value::Str(s.clone())),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::None => Ok(Value::None),
            ExprKind::Symbol(name) => self.eval_symbol(&env, name),
            ExprKind::List(elems) => self.eval_list(elems, env),
        };
        result.map_err(|error| attach_node_location(expr, error))
    }
}

impl Interpreter {
    /// Обработка списков (вызовов форм).
    fn eval_list(&self, elems: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if elems.is_empty() {
            // `()` — литерал пустого списка (как и `(list)`).
            return Ok(Value::List(Vec::new()));
        }

        let first = &elems[0];
        let rest = &elems[1..];

        // Специальные формы, опознаваемые по имени-символу.
        if let ExprKind::Symbol(name) = &first.kind {
            match name.as_str() {
                "define" => return self.do_define(rest, &env),
                "define-public" => return self.do_define_public(rest, &env),
                "define-const" => return self.do_define_const(rest, &env),
                "import" => return self.do_import(rest, &env),
                "defstruct" => return self.do_defstruct(rest, &env),
                "define-mutable" => return self.do_define_mutable(rest, &env),
                "set!" => return self.do_set(rest, &env),
                "lambda" => return self.do_lambda(rest, env),
                "#lupus" => return self.do_directive(rest),
                "if" => return self.do_if(rest, env),
                "while" => return self.do_while(rest, env),
                "loop" => return self.do_loop(rest, env),
                "exit" => return self.do_exit(rest, env),
                "begin" => return self.do_begin(rest, env),
                "and" => return self.do_and(rest, env),
                "or" => return self.do_or(rest, env),
                "cond" => return self.do_cond(rest, env),
                "for" => return self.do_for(rest, env),
                "range" => return self.do_range(rest, env),
                "let" => return self.do_let(rest, &env),
                "match" => return self.do_match(rest, env),
                "if-let" => return self.do_if_let(rest, env),
                "assert" => return self.do_assert(rest, env),
                "test" => return Ok(Value::None),
                _ => {}
            }

            // Остальные встроенные функции — из статической таблицы.
            if let Some(builtin) = builtins::lookup(name) {
                builtin.arity.check(builtin.name, rest.len())?;
                let args = self.eval_all(rest, &env)?;
                return self.invoke_native(&NativeImpl::Builtin(builtin.func), &args, &env);
            }

            // Конструкторы и аксессоры структур (ЭТАП 3 V0.4).
            if let Some(handled) = self.try_struct_call(name, rest, &env)? {
                return Ok(handled);
            }
        }

        // Пользовательская функция: первый элемент вычисляется как значение.
        self.eval_call(elems, env)
    }

    /// Вычисляет все формы и возвращает список значений.
    fn eval_all(
        &self,
        forms: &[Expr],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Vec<Value>, Error> {
        forms.iter().map(|f| self.eval(f, env.clone())).collect()
    }

    /// Вызов пользовательской функции.
    fn eval_call(&self, elems: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        let first = &elems[0];
        let actual_args = &elems[1..];

        let func_val = self.eval(first, env.clone())?;

        match func_val {
            Value::Closure(closure) => self.call_closure(&closure, actual_args, &env),
            Value::Native { name, arity, func } => {
                arity.check(&name, actual_args.len())?;
                let args = self.eval_all(actual_args, &env)?;
                self.invoke_native(&func, &args, &env)
            }
            _ => Err(Error::type_mismatch("Не могу вызвать не-функцию")),
        }
    }

    /// Проверяет арность и вызывает нативную функцию с готовыми аргументами.
    fn call_native(
        &self,
        name: &str,
        arity: Arity,
        func: &NativeImpl,
        args: &[Value],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Value, Error> {
        arity.check(name, args.len())?;
        self.invoke_native(func, args, env)
    }

    /// Вызывает нативную функцию; арность уже проверена вызывающим кодом.
    fn invoke_native(
        &self,
        func: &NativeImpl,
        args: &[Value],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Value, Error> {
        match func {
            NativeImpl::Builtin(BuiltinImpl::Pure(function)) => function(args),
            NativeImpl::Builtin(BuiltinImpl::Context(function)) => function(self, env, args),
            NativeImpl::Module { package, symbol } => {
                native::call(self, env, package, symbol, args)
            }
        }
    }

    /// Вызывает значение-функцию с уже вычисленными аргументами.
    ///
    /// Используется функциями высшего порядка (`list-map`, `list-filter`,
    /// `list-fold`): функция-аргумент может быть замыканием или builtin.
    pub(crate) fn call_value_with_values(
        &self,
        callable: &Value,
        args: Vec<Value>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Value, Error> {
        match callable {
            Value::Closure(closure) => self.call_closure_with_values(closure, args),
            Value::Native { name, arity, func } => self.call_native(name, *arity, func, &args, env),
            _ => Err(Error::type_mismatch("Не могу вызвать не-функцию")),
        }
    }

    /// Проверяет и увеличивает глубину вызова пользовательской функции.
    fn enter_call(&self) -> Result<DepthGuard<'_>, Error> {
        let current = self.depth.get();
        if current >= MAX_CALL_DEPTH {
            return Err(Error::runtime(
                "stack-depth-exceeded",
                "Превышена максимальная глубина рекурсии (10000). Проверьте условие выхода.",
            ));
        }
        self.depth.set(current + 1);
        Ok(DepthGuard { depth: &self.depth })
    }

    /// Вызывает замыкание с переданными формами-аргументами.
    fn call_closure(
        &self,
        closure: &Closure,
        actual_args: &[Expr],
        caller_env: &Rc<RefCell<Environment>>,
    ) -> Result<Value, Error> {
        let _depth = self.enter_call()?;
        if actual_args.len() != closure.params.len() {
            return Err(Error::arity(format!(
                "Функция ожидает {} аргументов, получено {}",
                closure.params.len(),
                actual_args.len()
            )));
        }

        // Аргументы вычисляются в окружении вызывающего кода.
        let arg_vals = self.eval_all(actual_args, caller_env)?;

        // Новое окружение вызова с родителем — окружением замыкания.
        let func_env = Environment::new_child(closure.env.clone());
        for (param, val) in closure.params.iter().zip(arg_vals.iter()) {
            Environment::define(&func_env, param, val.clone(), false);
        }

        self.eval(&closure.body, func_env)
    }

    /// Вызывает замыкание с уже вычисленными значениями аргументов.
    ///
    /// Используется [`list_map`], [`list_filter`], [`list_fold`]:
    /// аргументы для `f` уже определены (элементы списка), и их не нужно
    /// повторно вычислять из AST-форм.
    fn call_closure_with_values(
        &self,
        closure: &Closure,
        arg_vals: Vec<Value>,
    ) -> Result<Value, Error> {
        let _depth = self.enter_call()?;
        if arg_vals.len() != closure.params.len() {
            return Err(Error::arity(format!(
                "Функция ожидает {} аргументов, получено {}",
                closure.params.len(),
                arg_vals.len()
            )));
        }

        let func_env = Environment::new_child(closure.env.clone());
        for (param, val) in closure.params.iter().zip(arg_vals.iter()) {
            Environment::define(&func_env, param, val.clone(), false);
        }

        self.eval(&closure.body, func_env)
    }

    /// `(define имя значение)` или `(define (имя параметры...) тело...)`.
    fn do_define(&self, rest: &[Expr], env: &Rc<RefCell<Environment>>) -> Result<Value, Error> {
        self.do_define_like(rest, env, DefineKind::Private)
    }

    /// `(define-public (имя (параметр тип)...) -> тип тело...)`.
    ///
    /// Семантика выполнения совпадает с `define`; дополнительно имя попадает
    /// в экспорт файлового модуля. Проверка аннотаций — задача линта.
    fn do_define_public(
        &self,
        rest: &[Expr],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Value, Error> {
        self.do_define_like(rest, env, DefineKind::Public)
    }

    /// `(define-const имя значение)` — неизменяемая связь, как `define`.
    fn do_define_const(
        &self,
        rest: &[Expr],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Value, Error> {
        self.do_define_like(rest, env, DefineKind::Const)
    }

    fn do_define_like(
        &self,
        rest: &[Expr],
        env: &Rc<RefCell<Environment>>,
        kind: DefineKind,
    ) -> Result<Value, Error> {
        if rest.len() < 2 {
            return Err(Error::arity(format!(
                "({} ...) требует минимум 2 аргумента, получено {}",
                kind.form_name(),
                rest.len()
            )));
        }

        let (name, value) = match &rest[0].kind {
            // (define имя значение)
            ExprKind::Symbol(name) => {
                let value = self.eval(&rest[1], env.clone())?;
                (name.clone(), value)
            }
            // (define (имя параметры...) [-> тип] тело...)
            ExprKind::List(header) => {
                let (fn_name, param_names) = self.parse_function_header(header)?;

                // Необязательная аннотация возвращаемого типа `-> type`.
                let body_start = if matches!(rest.get(1), Some(Expr { kind: ExprKind::Symbol(s), .. }) if s == "->")
                {
                    if rest.len() < 3 {
                        return Err(Error::type_error(
                            "missing-return-type",
                            "После '->' ожидался тип возвращаемого значения",
                        ));
                    }
                    // Тип проверяется линтом; здесь только пропускаем его.
                    3
                } else {
                    1
                };

                let body_forms = &rest[body_start..];
                if body_forms.is_empty() {
                    return Err(Error::arity(format!(
                        "({} ...) требует тело функции",
                        kind.form_name()
                    )));
                }
                let body_expr = body_from_forms(body_forms);
                let closure = Closure {
                    params: param_names,
                    body: body_expr,
                    env: env.clone(),
                };
                gc::record_allocation();
                (fn_name, Value::Closure(closure))
            }
            _ => {
                return Err(Error::type_error(
                    "unexpected-token",
                    "Некорректное определение: ожидалось имя или заголовок функции",
                ))
            }
        };

        Environment::define(env, &name, value.clone(), false);

        if kind == DefineKind::Public {
            if let Some(exports) = self.exports.borrow_mut().as_mut() {
                exports.insert(name, value);
            }
        }

        Ok(Value::None)
    }

    /// Разбирает заголовок функции `(имя параметр...)`.
    ///
    /// Параметр — символ или `(имя тип)`; аннотация типа парсится, но не
    /// проверяется (HM-тайпчекера в V0.5 нет).
    fn parse_function_header(&self, header: &[Expr]) -> Result<(String, Vec<String>), Error> {
        if header.is_empty() {
            return Err(Error::type_error(
                "unexpected-token",
                "Пустой список в определении функции",
            ));
        }
        let name = match &header[0].kind {
            ExprKind::Symbol(name) => name.clone(),
            _ => {
                return Err(Error::type_error(
                    "unexpected-token",
                    "Имя функции должно быть символом",
                ))
            }
        };

        let mut params = Vec::with_capacity(header.len().saturating_sub(1));
        for param in &header[1..] {
            match &param.kind {
                ExprKind::Symbol(name) => params.push(name.clone()),
                ExprKind::List(items) if !items.is_empty() => match &items[0].kind {
                    ExprKind::Symbol(name) => params.push(name.clone()),
                    _ => {
                        return Err(Error::type_error(
                            "unexpected-token",
                            "Имя параметра должно быть символом",
                        ))
                    }
                },
                _ => {
                    return Err(Error::type_error(
                        "unexpected-token",
                        "Параметры должны быть символами или (имя тип)",
                    ))
                }
            }
        }
        Ok((name, params))
    }

    /// (defstruct имя (поле тип)...) или (defstruct (Имя a b) (поле тип)...)
    ///
    /// Регистрирует тип структуры и автогенерирует конвенцию: конструктор
    /// `(имя значение...)` и аксессоры `(имя-поле структура)`. Конструктор
    /// проверяет конкретные типы полей (`int`, `float`, `bool`, `str`,
    /// `none`/`unit` и составные `List`/`Option`/`Result`/`Tuple`/`Map`);
    /// типовые параметры дженериков принимают значение любого типа.
    /// Дубликат имени поля — ошибка `duplicate-struct-field`.
    fn do_defstruct(&self, rest: &[Expr], _env: &Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() < 2 {
            return Err(Error::arity(format!(
                "(defstruct ...) требует имя и минимум одно поле, получено {}",
                rest.len()
            )));
        }
        let (type_name, type_params) = match &rest[0].kind {
            ExprKind::Symbol(name) => (name.clone(), Vec::new()),
            ExprKind::List(header) => match header.first() {
                Some(Expr {
                    kind: ExprKind::Symbol(name),
                    ..
                }) => {
                    let mut params = Vec::new();
                    for param in &header[1..] {
                        match &param.kind {
                            ExprKind::Symbol(param) => params.push(param.clone()),
                            _ => {
                                return Err(Error::type_error(
                                    "unexpected-token",
                                    "Параметр дженерика должен быть идентификатором",
                                ))
                            }
                        }
                    }
                    (name.clone(), params)
                }
                _ => {
                    return Err(Error::type_error(
                        "unexpected-token",
                        "Имя структуры должно быть символом",
                    ))
                }
            },
            _ => {
                return Err(Error::type_error(
                    "unexpected-token",
                    "Имя структуры должно быть символом",
                ))
            }
        };

        let mut field_names: Vec<String> = Vec::with_capacity(rest.len() - 1);
        let mut field_types: Vec<FieldType> = Vec::with_capacity(rest.len() - 1);
        for field in &rest[1..] {
            let Some(items) = field.as_list() else {
                return Err(Error::type_error(
                    "unexpected-token",
                    "Каждое поле defstruct должно быть списком (имя тип)",
                ));
            };
            if items.len() != 2 {
                return Err(Error::type_error(
                    "unexpected-token",
                    "Поле defstruct должно быть списком из 2 элементов (имя тип)",
                ));
            }
            let field_name = match &items[0].kind {
                ExprKind::Symbol(n) => n.clone(),
                _ => {
                    return Err(Error::type_error(
                        "unexpected-token",
                        "Имя поля должно быть символом",
                    ))
                }
            };
            if field_names.contains(&field_name) {
                return Err(Error::type_error(
                    "duplicate-struct-field",
                    format!(
                        "duplicate-struct-field: поле '{}' уже объявлено в структуре '{}'",
                        field_name, type_name
                    ),
                ));
            }
            field_names.push(field_name);
            field_types.push(FieldType::from_expr(&items[1], &type_params));
        }

        self.structs.borrow_mut().insert(
            type_name.clone(),
            StructDef {
                name: type_name,
                type_params,
                fields: field_names,
                field_types,
            },
        );
        Ok(Value::None)
    }

    /// Обрабатывает вызов конструктора (`(point 10 20)`) или аксессора
    /// (`(point-x p)`) структуры. Возвращает `None`, если имя не относится
    /// к аксессору/конструктору ни одной зарегистрированной структуры.
    fn try_struct_call(
        &self,
        name: &str,
        rest: &[Expr],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Option<Value>, Error> {
        // Клонируем определение, чтобы не держать заимствование `structs`
        // во время вычисления аргументов: аргументы могут сами объявлять
        // структуры (`defstruct` — выражение) и брать `borrow_mut`.
        let constructor = self.structs.borrow().get(name).cloned();

        // Конструктор: имя совпадает с зарегистрированным типом.
        if let Some(definition) = constructor {
            let args = self.eval_all(rest, env)?;
            if args.len() != definition.fields.len() {
                return Err(Error::arity(format!(
                    "({} ...) требует {} аргументов, получено {}",
                    name,
                    definition.fields.len(),
                    args.len()
                )));
            }
            for (index, value) in args.iter().enumerate() {
                if !definition.field_types[index].accepts(value) {
                    return Err(Error::type_mismatch(format!(
                        "поле {}: ожидался {}",
                        definition.fields[index],
                        definition.field_types[index].display()
                    )));
                }
            }
            return Ok(Some(Value::Struct {
                name: name.to_string(),
                fields: args,
            }));
        }

        // Аксессор: имя вида `<тип>-<поле>`.
        if let Some(dash) = name.rfind('-') {
            let (type_part, field_part) = name.split_at(dash);
            let field_part = &field_part[1..];
            // Аксессор ищется по точному имени типа, а при отсутствии — без
            // учёта регистра (совместимость с `(node-value n)` для `Node`).
            let definition = {
                let types = self.structs.borrow();
                types.get(type_part).cloned().or_else(|| {
                    types
                        .values()
                        .find(|definition| definition.name.eq_ignore_ascii_case(type_part))
                        .cloned()
                })
            };
            if let Some(definition) = definition {
                let Some((idx, _)) = definition
                    .fields
                    .iter()
                    .enumerate()
                    .find(|(_, f)| f.as_str() == field_part)
                else {
                    return Ok(None);
                };
                if rest.len() != 1 {
                    return Err(Error::arity(format!(
                        "({} ...) требует ровно 1 аргумент, получено {}",
                        name,
                        rest.len()
                    )));
                }
                let value = self.eval(&rest[0], env.clone())?;
                match value {
                    Value::Struct {
                        name: sn,
                        fields: sf,
                    } if sn == definition.name => {
                        return Ok(Some(sf[idx].clone()));
                    }
                    _ => {
                        return Err(Error::type_mismatch(format!(
                            "Аксессор {} нельзя применить к чужой структуре",
                            name
                        )));
                    }
                }
            }
        }

        Ok(None)
    }

    /// (import пакет) / (import пакет :as префикс) / (import пакет :all)
    /// / (import (пакет модуль) ...).
    fn do_import(&self, rest: &[Expr], env: &Rc<RefCell<Environment>>) -> Result<Value, Error> {
        let spec = modules::parse_import(rest)?;
        let prefix = spec.prefix();

        let imported = if let Some(path) = modules::resolve_file_from(
            &spec.package,
            &self
                .program_dir
                .borrow()
                .iter()
                .cloned()
                .collect::<Vec<PathBuf>>(),
        ) {
            ImportedModule::File(self.load_file_module(&path)?)
        } else if native::is_native(&spec.package) {
            ImportedModule::Native {
                package: spec.package.clone(),
            }
        } else {
            return Err(Error::lint(
                "unknown-module",
                format!(
                    "Неизвестный модуль '{}': нет нативного пакета и файла stdlib/{}.lupus",
                    spec.package, spec.package
                ),
            )
            .with_hint(format!(
                "Доступные нативные пакеты V0.6: {}",
                native::PACKAGES.join(", ")
            )));
        };

        if spec.import_all {
            self.import_all_symbols(&imported, &prefix, env)?;
        }

        if !spec.import_all || spec.alias.is_some() {
            self.imports
                .borrow_mut()
                .insert(prefix.clone(), imported.clone());
        }

        Ok(Value::None)
    }

    /// Импортирует все имена модуля без префикса (модификатор `:all`).
    fn import_all_symbols(
        &self,
        module: &ImportedModule,
        prefix: &str,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<(), Error> {
        let mut entries: Vec<(String, Value)> = Vec::new();
        match module {
            ImportedModule::Native { package } => {
                for name in native::names(package) {
                    if let Some(value) = native::symbol_value(package, name) {
                        entries.push(((*name).to_string(), value));
                    }
                }
            }
            ImportedModule::File(exports) => {
                entries.extend(
                    exports
                        .iter()
                        .map(|(name, value)| (name.clone(), value.clone())),
                );
            }
        }

        for (name, value) in entries {
            if Environment::get(env, &name).is_ok() || builtins::is_core_name(&name) {
                return Err(Error::lint(
                    "import-collision",
                    format!(
                        "Имя '{}' из модуля '{}' уже существует в текущем окружении",
                        name, prefix
                    ),
                )
                .with_hint("используйте префиксный импорт или модификатор :as"));
            }
            Environment::define(env, &name, value, false);
        }
        Ok(())
    }

    /// Загружает файловый модуль один раз; экспортируются только define-public.
    fn load_file_module(
        &self,
        path: &std::path::Path,
    ) -> Result<Rc<HashMap<String, Value>>, Error> {
        let canonical = modules::canonical(path);
        if let Some(cached) = self.module_cache.borrow().get(&canonical) {
            return Ok(cached.clone());
        }
        if self.module_stack.borrow().contains(&canonical) {
            return Err(Error::lint(
                "import-cycle",
                format!("Циклический импорт модуля: {}", path.display()),
            ));
        }

        let source = std::fs::read_to_string(path).map_err(|error| {
            Error::lint(
                "unknown-module",
                format!("Не удалось прочитать модуль {}: {}", path.display(), error),
            )
        })?;
        let tokens = crate::lexer::lex(&source)
            .map_err(|error| error.with_file_if_located(path.display().to_string()))?;
        let forms = crate::parser::parse(tokens)
            .map_err(|error| error.with_file_if_located(path.display().to_string()))?;

        self.module_stack.borrow_mut().push(canonical.clone());
        let module_env = Environment::new();
        let previous_exports = self.exports.replace(Some(HashMap::new()));

        let mut outcome = Ok(());
        for form in &forms {
            if is_test_form(form) {
                continue;
            }
            if let Err(error) = self.eval(form, module_env.clone()) {
                outcome = Err(error);
                break;
            }
        }

        let exports = self.exports.replace(previous_exports).unwrap_or_default();
        self.module_stack.borrow_mut().pop();
        outcome?;

        let exports = Rc::new(exports);
        self.module_cache
            .borrow_mut()
            .insert(canonical, exports.clone());
        Ok(exports)
    }

    /// Обрабатывает директиву `(#lupus ...)`.
    ///
    /// FFI-python объявлен заменённым нативными модулями (ERRATA §2).
    fn do_directive(&self, rest: &[Expr]) -> Result<Value, Error> {
        let name = match rest.first() {
            Some(Expr {
                kind: ExprKind::Symbol(name),
                ..
            }) => name.as_str(),
            _ => "",
        };
        match name {
            "enable-check" => Ok(Value::None),
            "ffi" => Err(Error::lint(
                "invalid-directive",
                "Python-FFI объявлен заменённым нативными модулями",
            )
            .with_hint(
                "используйте (import senko), (import kaltsit) или (import w) вместо (#lupus ffi python ...)",
            )),
            "" => Err(Error::lint(
                "invalid-directive",
                "Директива #lupus требует имя",
            )),
            other => Err(Error::lint(
                "invalid-directive",
                format!("Неизвестная директива #lupus {}", other),
            )),
        }
    }

    /// (define-mutable имя значение).
    fn do_define_mutable(
        &self,
        rest: &[Expr],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Value, Error> {
        if rest.len() != 2 {
            return Err(Error::arity(format!(
                "(define-mutable ...) требует ровно 2 аргумента, получено {}",
                rest.len()
            )));
        }
        let name = match &rest[0].kind {
            ExprKind::Symbol(n) => n,
            _ => {
                return Err(Error::type_mismatch(
                    "Первый аргумент define-mutable должен быть символом",
                ))
            }
        };
        let value = self.eval(&rest[1], env.clone())?;
        Environment::define(env, name, value, true);
        Ok(Value::None)
    }

    /// (set! имя значение).
    fn do_set(&self, rest: &[Expr], env: &Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 2 {
            return Err(Error::arity(format!(
                "(set! ...) требует ровно 2 аргумента, получено {}",
                rest.len()
            )));
        }
        let name = match &rest[0].kind {
            ExprKind::Symbol(n) => n,
            _ => {
                return Err(Error::type_mismatch(
                    "Первый аргумент set! должен быть символом",
                ))
            }
        };
        let new_value = self.eval(&rest[1], env.clone())?;
        match Environment::set(env, name, new_value) {
            Ok(()) => Ok(Value::None),
            Err(e) if e.code == "unknown-identifier" => {
                // Улучшенное сообщение с подсказками по похожим именам.
                let error = self.undefined_variable_error(env, name);
                Err(attach_node_location(&rest[0], error))
            }
            Err(e) => Err(e),
        }
    }

    /// (lambda (параметры...) тело...).
    fn do_lambda(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() < 2 {
            return Err(Error::arity(format!(
                "(lambda ...) требует минимум 2 аргумента, получено {}",
                rest.len()
            )));
        }
        let param_list = match &rest[0].kind {
            ExprKind::List(items) => items,
            _ => {
                return Err(Error::type_mismatch(
                    "Первый аргумент lambda должен быть списком параметров",
                ))
            }
        };

        let mut param_names = Vec::new();
        for p in param_list {
            match &p.kind {
                ExprKind::Symbol(s) => param_names.push(s.clone()),
                ExprKind::List(items) if !items.is_empty() => match &items[0].kind {
                    ExprKind::Symbol(name) => param_names.push(name.clone()),
                    _ => {
                        return Err(Error::type_error(
                            "unexpected-token",
                            "Имя параметра lambda должно быть символом",
                        ))
                    }
                },
                _ => {
                    return Err(Error::type_error(
                        "unexpected-token",
                        "Параметры lambda должны быть символами или (имя тип)",
                    ))
                }
            }
        }

        let body_start = if matches!(rest.get(1), Some(Expr { kind: ExprKind::Symbol(s), .. }) if s == "->")
        {
            3
        } else {
            1
        };
        if rest.len() <= body_start {
            return Err(Error::arity(
                "(lambda ...) требует тело функции после параметров",
            ));
        }
        let body_expr = body_from_forms(&rest[body_start..]);

        gc::record_allocation();
        Ok(Value::Closure(Closure {
            params: param_names,
            body: body_expr,
            env,
        }))
    }

    /// (if условие then else).
    fn do_if(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 3 && rest.len() != 2 {
            return Err(Error::arity(format!(
                "(if ...) требует ровно 2 или 3 аргумента, получено {}",
                rest.len()
            )));
        }

        let cond_val = self.eval(&rest[0], env.clone())?;

        if is_truthy(&cond_val) {
            self.eval(&rest[1], env)
        } else if rest.len() == 3 {
            self.eval(&rest[2], env)
        } else {
            Ok(Value::None)
        }
    }

    /// (while условие тело...).
    fn do_while(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() < 2 {
            return Err(Error::arity(format!(
                "(while ...) требует минимум 2 аргумента, получено {}",
                rest.len()
            )));
        }

        let cond_form = &rest[0];
        let body_forms = &rest[1..];

        let mut result = Value::None;

        loop {
            let cond_val = self.eval(cond_form, env.clone())?;
            if !is_truthy(&cond_val) {
                break;
            }
            for form in body_forms {
                result = self.eval(form, env.clone())?;
            }
        }

        Ok(result)
    }

    /// (loop тело...) — бесконечный цикл. Выход — только через `(exit n)`.
    fn do_loop(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        loop {
            for form in rest {
                self.eval(form, env.clone())?;
            }
        }
    }

    /// (exit n) — немедленно завершает процесс с кодом n.
    fn do_exit(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 1 {
            return Err(Error::arity(format!(
                "(exit ...) требует ровно 1 аргумент, получено {}",
                rest.len()
            )));
        }
        let code = match self.eval(&rest[0], env)? {
            Value::Int(c) => c,
            _ => return Err(Error::type_mismatch("exit требует целого числа")),
        };
        if !(0..=255).contains(&code) {
            return Err(Error::runtime(
                "invalid-exit-code",
                format!(
                    "Код выхода должен быть в диапазоне 0..=255, получено {}",
                    code
                ),
            ));
        }
        std::process::exit(code as i32);
    }

    /// (begin форма...).
    fn do_begin(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        let mut result = Value::None;
        for form in rest {
            result = self.eval(form, env.clone())?;
        }
        Ok(result)
    }

    /// (and форма...) — короткое замыкание; возвращает значение.
    ///
    /// Возвращает последнее вычисленное значение, если все истинны, иначе —
    /// первое ложное. `(and)` → `true`.
    fn do_and(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        let mut result = Value::Bool(true);
        for form in rest {
            result = self.eval(form, env.clone())?;
            if !is_truthy(&result) {
                return Ok(result);
            }
        }
        Ok(result)
    }

    /// (or форма...) — короткое замыкание; возвращает значение.
    ///
    /// Возвращает первое истинное значение, иначе — последнее ложное.
    /// `(or)` → `false`.
    fn do_or(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        let mut result = Value::Bool(false);
        for form in rest {
            result = self.eval(form, env.clone())?;
            if is_truthy(&result) {
                return Ok(result);
            }
        }
        Ok(result)
    }

    /// (print строка).
    /// Печатает значение с учётом подавления вывода в тестах.
    pub(crate) fn print_value(&self, value: &Value) -> Result<(), Error> {
        if !self.suppress_print.get() {
            builtins::print_arg(value)?;
        }
        Ok(())
    }

    /// (cond (условие тело...) ... (else тело...))
    ///
    /// Ветки проверяются по порядку. Тело может содержать несколько форм
    /// (неявный `begin`). Если ни одна ветка не совпала и `else` отсутствует —
    /// возвращается `None`.
    fn do_cond(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        for clause in rest {
            let Some(items) = clause.as_list() else {
                return Err(Error::type_mismatch(
                    "Каждая ветка cond должна быть списком",
                ));
            };
            if items.is_empty() {
                return Err(Error::type_mismatch("Пустая ветка cond"));
            }
            let header = &items[0];
            let body = &items[1..];

            // Ветка (else тело...) — безусловная.
            if let Some(name) = header.as_symbol() {
                if name == "else" {
                    if body.is_empty() {
                        return Ok(Value::None);
                    }
                    let mut result = Value::None;
                    for form in body {
                        result = self.eval(form, env.clone())?;
                    }
                    return Ok(result);
                }
            }

            let cond_val = self.eval(header, env.clone())?;
            if is_truthy(&cond_val) {
                if body.is_empty() {
                    return Ok(Value::None);
                }
                let mut result = Value::None;
                for form in body {
                    result = self.eval(form, env.clone())?;
                }
                return Ok(result);
            }
        }
        Ok(Value::None)
    }

    /// (for переменная in iter-выражение тело...)
    ///
    /// Выражение обязано вычислиться в список, иначе — ошибка. Переменная
    /// связывается заново на каждой итерации. Возвращает `None`.
    fn do_for(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        // Ожидаем: [var, "in", iter, тело...]
        if rest.len() < 3 {
            return Err(Error::type_mismatch(
                "(for ...) требует форму: (for var in iter тело...)".to_string(),
            ));
        }
        let var = match &rest[0].kind {
            ExprKind::Symbol(s) => s.clone(),
            _ => {
                return Err(Error::type_mismatch(
                    "Переменная цикла for должна быть символом",
                ))
            }
        };
        if !matches!(&rest[1].kind, ExprKind::Symbol(s) if s == "in") {
            return Err(Error::type_mismatch("for: ожидалось ключевое слово 'in'"));
        }
        let iter_expr = &rest[2];
        let body_forms = &rest[3..];

        let iter_val = self.eval(iter_expr, env.clone())?;
        let items = match iter_val {
            Value::List(items) => items,
            _ => {
                return Err(Error::type_mismatch(
                    "for: выражение после 'in' должно вычисляться в список",
                ))
            }
        };

        for item in items {
            let iter_env = Environment::new_child(env.clone());
            Environment::define(&iter_env, &var, item, false);
            for form in body_forms {
                self.eval(form, iter_env.clone())?;
            }
        }
        Ok(Value::None)
    }

    /// (range start end) → список целых чисел [start, end).
    ///
    /// Если `start >= end` — пустой список.
    fn do_range(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 2 {
            return Err(Error::arity(format!(
                "(range ...) требует ровно 2 аргумента, получено {}",
                rest.len()
            )));
        }
        let start = match self.eval(&rest[0], env.clone())? {
            Value::Int(v) => v,
            _ => {
                return Err(Error::type_mismatch(
                    "Первый аргумент range должен быть целым числом",
                ))
            }
        };
        let end = match self.eval(&rest[1], env.clone())? {
            Value::Int(v) => v,
            _ => {
                return Err(Error::type_mismatch(
                    "Второй аргумент range должен быть целым числом",
                ))
            }
        };

        let mut items = Vec::new();
        let mut i = start;
        while i < end {
            items.push(Value::Int(i));
            i += 1;
        }
        Ok(Value::List(items))
    }

    /// (let ((переменная значение) ...) тело...)
    ///
    /// Все связи вычисляются в **внешнем** окружении (параллельно), затем
    /// создаётся дочернее окружение, где выполняются формы тела.
    fn do_let(&self, rest: &[Expr], env: &Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.is_empty() {
            return Err(Error::arity("(let ...) требует список связей и тело"));
        }
        let bindings = match &rest[0].kind {
            ExprKind::List(items) => items,
            _ => {
                return Err(Error::type_mismatch(
                    "Первым аргументом let должен быть список связей",
                ))
            }
        };

        // 1) Вычисляем все значения в ВНЕШНЕМ окружении (параллельно).
        let mut pairs: Vec<(String, Value)> = Vec::with_capacity(bindings.len());
        for b in bindings {
            let Some(inner) = b.as_list() else {
                return Err(Error::type_mismatch(
                    "Каждая связь let должна быть списком (имя значение)",
                ));
            };
            if inner.len() != 2 {
                return Err(Error::type_mismatch(
                    "Связь let должна быть списком из 2 элементов",
                ));
            }
            let name = match &inner[0].kind {
                ExprKind::Symbol(s) => s.clone(),
                _ => return Err(Error::type_mismatch("Имя в связи let должно быть символом")),
            };
            let value = self.eval(&inner[1], env.clone())?;
            pairs.push((name, value));
        }

        // 2) Новое дочернее окружение с body.
        let let_env = Environment::new_child(env.clone());
        for (name, value) in &pairs {
            Environment::define(&let_env, name, value.clone(), false);
        }

        let body_forms = &rest[1..];
        let mut result = Value::None;
        for form in body_forms {
            result = self.eval(form, let_env.clone())?;
        }
        Ok(result)
    }

    /// (match выражение (паттерн тело...) ...)
    ///
    /// Паттерны: `_`; переменная; литерал (число/строка/булево/none);
    /// `(none)`; `(some p)`; `(success p)`; `(failure p)` — где `p` — вложенный
    /// паттерн. Совпадает первая подходящая ветка; переменные паттерна
    /// связываются в окружении тела. Если ни одна ветка не совпала —
    /// runtime-ошибка.
    fn do_match(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() < 2 {
            return Err(Error::type_mismatch(
                "(match ...) требует выражение и минимум одну ветку".to_string(),
            ));
        }
        let expr_val = self.eval(&rest[0], env.clone())?;
        let clauses = &rest[1..];

        for clause in clauses {
            let Some(items) = clause.as_list() else {
                return Err(Error::type_mismatch(
                    "Каждая ветка match должна быть списком",
                ));
            };
            if items.is_empty() {
                return Err(Error::type_mismatch("Пустая ветка match"));
            }
            let pattern = &items[0];
            let body = &items[1..];

            if let Some(bindings) = self.match_pattern(pattern, &expr_val)? {
                let match_env = Environment::new_child(env.clone());
                for (name, value) in bindings {
                    Environment::define(&match_env, &name, value, false);
                }
                let mut result = Value::None;
                for form in body {
                    result = self.eval(form, match_env.clone())?;
                }
                return Ok(result);
            }
        }

        Err(Error::runtime(
            "match-non-exhaustive",
            "match: ни одна ветка не совпала",
        ))
    }

    /// Сопоставляет паттерн со значением, возвращая новые связывания
    /// переменных (или `None`, если паттерн не совпал).
    ///
    /// Возвращает `Result`: некоторые структурные ошибки сопоставления
    /// (например, несовпадение арности у `(tuple ...)` при совпавшем типе)
    /// трактуются как runtime-ошибки, а не как простое «не совпало».
    fn match_pattern(
        &self,
        pattern: &Expr,
        value: &Value,
    ) -> Result<Option<Vec<(String, Value)>>, Error> {
        match &pattern.kind {
            // `_` — совпадает с чем угодно, без связываний.
            ExprKind::Symbol(name) if name == "_" => Ok(Some(Vec::new())),
            // Переменная — совпадает с чем угодно, связывает имя со значением.
            ExprKind::Symbol(name) => Ok(Some(vec![(name.clone(), value.clone())])),
            ExprKind::Number(n) => {
                if *value == Value::Int(*n) {
                    Ok(Some(Vec::new()))
                } else {
                    Ok(None)
                }
            }
            ExprKind::Float(f) => {
                if *value == Value::Float(*f) {
                    Ok(Some(Vec::new()))
                } else {
                    Ok(None)
                }
            }
            ExprKind::Str(s) => {
                if *value == Value::Str(s.clone()) {
                    Ok(Some(Vec::new()))
                } else {
                    Ok(None)
                }
            }
            ExprKind::Bool(b) => {
                if *value == Value::Bool(*b) {
                    Ok(Some(Vec::new()))
                } else {
                    Ok(None)
                }
            }
            ExprKind::None => {
                if *value == Value::None {
                    Ok(Some(Vec::new()))
                } else {
                    Ok(None)
                }
            }
            // Составные паттерны: (tuple ...), (none), (some p), (success p),
            // (failure p).
            ExprKind::List(items) => {
                if let Some(name) = items.first().and_then(Expr::as_symbol) {
                    // (tuple подпаттерны...) — позиционное связывание.
                    if name == "tuple" {
                        let Value::Tuple(elems) = value else {
                            return Ok(None);
                        };
                        if items.len() - 1 != elems.len() {
                            return Err(Error::arity(format!(
                                "tuple-паттерн ожидает {} элементов, получено {}",
                                items.len() - 1,
                                elems.len()
                            )));
                        }
                        let mut bindings = Vec::new();
                        for (pat, val) in items[1..].iter().zip(elems.iter()) {
                            match self.match_pattern(pat, val)? {
                                Some(b) => bindings.extend(b),
                                None => return Ok(None),
                            }
                        }
                        return Ok(Some(bindings));
                    }
                    if name == "none" && *value == Value::None {
                        return Ok(Some(Vec::new()));
                    }
                    // (имя-структуры подпаттерны...) — позиционное связывание
                    // полей структуры (ЭТАП 3 V0.4).
                    if self.structs.borrow().contains_key(name) {
                        let Value::Struct { name: sn, fields } = value else {
                            return Ok(None);
                        };
                        if sn != name {
                            return Ok(None);
                        }
                        if items.len() - 1 != fields.len() {
                            return Err(Error::arity(format!(
                                "Паттерн структуры '{}' ожидает {} полей, получено {}",
                                name,
                                items.len() - 1,
                                fields.len()
                            )));
                        }
                        let mut bindings = Vec::new();
                        for (pat, val) in items[1..].iter().zip(fields.iter()) {
                            match self.match_pattern(pat, val)? {
                                Some(b) => bindings.extend(b),
                                None => return Ok(None),
                            }
                        }
                        return Ok(Some(bindings));
                    }
                }
                if items.len() == 2 {
                    if let Some(name) = items[0].as_symbol() {
                        let inner = match name {
                            "some" => match value {
                                Value::Some(v) => Some(v),
                                _ => None,
                            },
                            "success" => match value {
                                Value::Success(v) => Some(v),
                                _ => None,
                            },
                            "failure" => match value {
                                Value::Failure(v) => Some(v),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let Some(inner) = inner {
                            return self.match_pattern(&items[1], inner);
                        }
                    }
                }
                Ok(None)
            }
        }
    }

    /// (if-let (имя выражение) then else)
    ///
    /// Если выражение дало `(some v)` — связать `имя = v` и выполнить `then`,
    /// иначе — выполнить `else`. Работает только с Option (по спеке V1.0).
    fn do_if_let(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 3 {
            return Err(Error::arity(format!(
                "(if-let ...) требует ровно 3 аргумента, получено {}",
                rest.len()
            )));
        }
        let binding = match &rest[0].kind {
            ExprKind::List(items) if items.len() == 2 => items,
            _ => {
                return Err(Error::type_mismatch(
                    "Первый аргумент if-let должен быть списком (имя выражение)",
                ))
            }
        };
        let name = match &binding[0].kind {
            ExprKind::Symbol(s) => s.clone(),
            _ => return Err(Error::type_mismatch("Имя в if-let должно быть символом")),
        };
        let val = self.eval(&binding[1], env.clone())?;
        let then_form = &rest[1];
        let else_form = &rest[2];

        match val {
            Value::Some(v) => {
                let branch_env = Environment::new_child(env.clone());
                Environment::define(&branch_env, &name, *v, false);
                self.eval(then_form, branch_env)
            }
            Value::None => self.eval(else_form, env.clone()),
            _ => Err(Error::type_mismatch(
                "if-let: выражение должно вычисляться в (some v) или none",
            )),
        }
    }

    /// (assert выражение)
    ///
    /// Если выражение истинно — возвращает `None`, иначе — runtime-ошибка
    /// с указанием формы для отладки.
    fn do_assert(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 1 {
            return Err(Error::arity(format!(
                "(assert ...) требует ровно 1 аргумент, получено {}",
                rest.len()
            )));
        }
        let result = self.eval(&rest[0], env)?;
        if is_truthy(&result) {
            Ok(Value::None)
        } else {
            let form_text = crate::ast::expr_to_source(&rest[0]);
            Err(Error::runtime(
                "assert-failed",
                format!("Assert не выполнен: <{}>", form_text),
            ))
        }
    }
}

impl Interpreter {
    /// Собирает формы `(test "имя" тело...)` верхнего уровня.
    pub fn collect_tests(forms: &[Expr]) -> Result<Vec<TestDef>, Error> {
        let mut tests = Vec::new();
        for form in forms {
            let Some(items) = form.as_list() else {
                continue;
            };
            if !matches!(items.first(), Some(Expr { kind: ExprKind::Symbol(name), .. }) if name == "test")
            {
                continue;
            }
            if items.len() < 2 {
                return Err(Error::arity(
                    "(test ...) требует строковое имя и тело теста",
                ));
            }
            let name = match &items[1].kind {
                ExprKind::Str(name) => name.clone(),
                _ => {
                    return Err(Error::type_mismatch(
                        "Имя теста в форме (test ...) должно быть строкой",
                    ))
                }
            };
            tests.push(TestDef {
                name,
                body: items[2..].to_vec(),
            });
        }
        Ok(tests)
    }

    /// Выполняет программу и все тесты файла.
    ///
    /// Тесты изолированы: каждый запускается в дочернем окружении базового
    /// окружения, поэтому `define` из одного теста не виден другому.
    /// Формы верхнего уровня (кроме `test`) выполняются один раз до тестов.
    pub fn run_tests(&mut self, forms: &[Expr], verbose: bool) -> Result<Vec<TestOutcome>, Error> {
        let tests = Self::collect_tests(forms)?;
        self.suppress_print.set(!verbose);

        for form in forms {
            if is_test_form(form) {
                continue;
            }
            self.eval(form, self.global.clone())?;
        }

        *self.test_base_env.borrow_mut() = Some(self.global.clone());
        *self.tests.borrow_mut() = tests.clone();

        let mut outcomes = Vec::with_capacity(tests.len());
        for test in &tests {
            let started = Instant::now();
            let result = self.run_test_body(test);
            outcomes.push(TestOutcome {
                name: test.name.clone(),
                passed: result.is_ok(),
                error: result.err(),
                duration_ms: started.elapsed().as_secs_f64() * 1000.0,
            });
        }

        self.suppress_print.set(false);
        Ok(outcomes)
    }

    /// Выполняет тело одного теста в свежем окружении.
    fn run_test_body(&self, test: &TestDef) -> Result<(), Error> {
        let base = self
            .test_base_env
            .borrow()
            .clone()
            .unwrap_or_else(|| self.global.clone());
        let env = Environment::new_child(base);
        self.running_tests.borrow_mut().push(test.name.clone());
        let mut result = Ok(());
        for form in &test.body {
            if let Err(error) = self.eval(form, env.clone()) {
                result = Err(error);
                break;
            }
        }
        self.running_tests.borrow_mut().pop();
        result
    }

    /// Запускает тесты по именам (`w/run`/`w/run-all`).
    ///
    /// Пустой список имён означает «все тесты». Возвращает код выхода:
    /// 0 — все прошли, 1 — есть провалы. Тесты, уже находящиеся в стеке
    /// выполнения, пропускаются (защита от рекурсии).
    pub fn run_tests_named(&self, names: &[String]) -> i64 {
        let tests = self.tests.borrow().clone();
        let running = self.running_tests.borrow().clone();
        let selected: Vec<&TestDef> = tests
            .iter()
            .filter(|test| names.is_empty() || names.contains(&test.name))
            .filter(|test| !running.contains(&test.name))
            .collect();

        let previous = self.suppress_print.replace(true);
        let mut failed = false;
        for test in selected {
            if self.run_test_body(test).is_err() {
                failed = true;
            }
        }
        self.suppress_print.set(previous);
        if failed {
            1
        } else {
            0
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Interpreter {
    fn drop(&mut self) {
        // Сначала убираем принадлежащие интерпретатору корни, чтобы цикл,
        // замкнутый на global/модуль, не считался внешне достижимым.
        self.imports.borrow_mut().clear();
        self.module_cache.borrow_mut().clear();
        *self.exports.borrow_mut() = None;
        *self.test_base_env.borrow_mut() = None;

        // `global` — сильный корень, пока поле живо. Освобождаем поле до
        // сборки; если снаружи остался closure, он удержит окружение сам.
        let global = std::mem::replace(&mut self.global, Environment::new_unregistered());
        drop(global);
        let _ = gc::collect();
    }
}

impl Interpreter {
    /// Вычисляет символ (переменную), добавляя в сообщение об ошибке
    /// подсказки с похожими именами из окружения.
    fn eval_symbol(&self, env: &Rc<RefCell<Environment>>, name: &str) -> Result<Value, Error> {
        // Квалифицированное имя `<префикс>/<символ>`.
        if let Some((prefix, symbol)) = name.split_once('/') {
            return self.eval_qualified(prefix, symbol);
        }

        match Environment::get(env, name) {
            Ok(value) => Ok(value),
            Err(e) if e.code == "unknown-identifier" => {
                if let Some(builtin) = builtins::lookup(name) {
                    Ok(builtin.value())
                } else {
                    Err(self.undefined_variable_error(env, name))
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Вычисляет квалифицированное имя импортированного модуля.
    fn eval_qualified(&self, prefix: &str, symbol: &str) -> Result<Value, Error> {
        let module = self.imports.borrow().get(prefix).cloned();
        let Some(module) = module else {
            return Err(Error::unknown_identifier(format!(
                "Префикс модуля '{}' не импортирован",
                prefix
            ))
            .with_hint(format!("добавьте (import {})", prefix)));
        };

        match module {
            ImportedModule::Native { package } => native::symbol_value(&package, symbol)
                .ok_or_else(|| {
                    Error::runtime(
                        "unknown-module-symbol",
                        format!("{}/{}: неизвестный символ модуля", prefix, symbol),
                    )
                }),
            ImportedModule::File(exports) => exports.get(symbol).cloned().ok_or_else(|| {
                Error::runtime(
                    "unknown-module-symbol",
                    format!("{}/{}: неизвестный символ модуля", prefix, symbol),
                )
            }),
        }
    }

    /// Строит подробное сообщение о неопределённой переменной со списком
    /// вариантов, похожих по написанию Levenshtein-расстоянию.
    fn undefined_variable_error(&self, env: &Rc<RefCell<Environment>>, name: &str) -> Error {
        let known = Environment::all_names(env);

        let mut scored: Vec<(usize, String)> = known
            .into_iter()
            .map(|candidate| (levenshtein(name, &candidate), candidate))
            .collect();
        scored.sort_by_key(|(dist, _)| *dist);

        let threshold = (name.len() / 2).clamp(2, 4);
        let suggestions: Vec<String> = scored
            .into_iter()
            .filter(|(dist, _)| *dist <= threshold)
            .take(3)
            .map(|(_, candidate)| format!("'{}'", candidate))
            .collect();

        let mut message = format!("Переменная '{}' не определена", name);
        if !suggestions.is_empty() {
            message.push_str(&format!(
                ". Возможно, вы имели в виду: {}",
                suggestions.join(", ")
            ));
        }
        Error::unknown_identifier(message)
    }
}

/// Удаляет «лишние» закрывающие скобки `)` из потока токенов.
///
/// Используется в lenient-режиме (`--lenient-parens`): если в коде, сгенерённом
/// моделью, встречается несбалансированная `)` (например, `...))` после цикла),
/// она игнорируется с предупреждением в stderr, а не вызывает падение парсера.
///
/// Замечание: требование «игнорировать RPAREN в `eval_list()`» архитектурно
/// невыполнимо — скобки потребляются парсером до построения AST, поэтому
/// чистка производится на уровне токенов, до парсинга.
pub fn sanitize_tokens_lenient(tokens: Vec<Token>) -> Vec<Token> {
    let mut depth: i64 = 0;
    let mut out = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token.kind {
            TokenKind::LParen => {
                depth += 1;
                out.push(token);
            }
            TokenKind::RParen if depth <= 0 => {
                eprintln!(
                    "Предупреждение: игнорирую лишнюю ')' на строке {}, колонке {}",
                    token.line, token.col
                );
            }
            TokenKind::RParen => {
                depth -= 1;
                out.push(token);
            }
            _ => out.push(token),
        }
    }

    out
}

/// Вид определения (`define` / `define-public` / `define-const`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefineKind {
    Private,
    Public,
    Const,
}

impl DefineKind {
    fn form_name(self) -> &'static str {
        match self {
            DefineKind::Private => "define",
            DefineKind::Public => "define-public",
            DefineKind::Const => "define-const",
        }
    }
}

/// Собирает тело из одной или нескольких форм (неявный `begin`).
fn body_from_forms(forms: &[Expr]) -> Expr {
    if forms.len() == 1 {
        forms[0].clone()
    } else {
        Expr::unspanned(ExprKind::List(
            std::iter::once(Expr::unspanned(ExprKind::Symbol("begin".to_string())))
                .chain(forms.iter().cloned())
                .collect(),
        ))
    }
}

/// Прикрепляет к ошибке span и позицию узла, если error ещё не локализована.
fn attach_node_location(expr: &Expr, error: Error) -> Error {
    if error.location.is_some() {
        return error;
    }
    match expr.source_location() {
        Some((span, line, col)) => error.with_span_location(line, col, span),
        None => error,
    }
}

/// Проверяет, является ли форма объявлением теста.
fn is_test_form(form: &Expr) -> bool {
    matches!(&form.kind, ExprKind::List(items) if matches!(items.first(), Some(Expr { kind: ExprKind::Symbol(name), .. }) if name == "test"))
}

/// Расстояние Левенштейна (для подсказок по неопределённым переменным).
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];

    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let del = prev[j] + 1; // удаление
            let ins = curr[j - 1] + 1; // вставка
            let sub = prev[j - 1] + cost; // замена
            curr[j] = del.min(ins).min(sub);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b.len()]
}
