//! Окружения переменных с поддержкой лексических областей видимости.
//!
//! Окружение — это иерархическая цепочка связей «имя → значение».
//! Каждое окружение хранит ссылку на родителя; поиск переменной идёт вверх
//! по цепочке. Замыкания захватывают окружение как `Rc<RefCell<Environment>>`,
//! что позволяет `set!` изменять переменные во внешних областях видимости.

use crate::ast::Expr;
use crate::error::Error;
use crate::gc;
use crate::value::{Closure, Traversable, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// Значение, хранимое в связи окружения.
///
/// Замыкание, захватившее **это же** окружение, хранится без сильной ссылки
/// на него ([`StoredValue::WeakClosure`]). Иначе возникает `Rc`-цикл
/// `env → closure → env`, который не освобождается при выходе: ровно это
/// происходило с функциями из файловых модулей (`module_env → exports →
/// Closure.env → module_env`, issue #45).
///
/// Слабая ссылка безопасна: пока связь доступна, само окружение живо, а при
/// любом чтении через [`Environment::get`] замыкание «поднимается» обратно
/// до сильной ссылки. Наружу (в том числе в экспорты модуля и в значения,
/// покидающие кадр вызова) всегда уходит [`Value::Closure`] с сильным `env`.
#[derive(Debug)]
enum StoredValue {
    /// Обычное значение.
    Value(Value),
    /// Замыкание, чьё лексическое окружение — окружение-владелец связи.
    WeakClosure {
        /// Имена параметров.
        params: Vec<String>,
        /// Тело функции.
        body: Expr,
        /// Слабая ссылка на окружение, в котором лежит связь.
        env: Weak<RefCell<Environment>>,
    },
}

impl StoredValue {
    /// Готовит значение к хранению в `env`.
    ///
    /// Если замыкание захватывает ровно `env`, сильная ссылка заменяется
    /// слабой; во всех остальных случаях значение хранится как есть, чтобы
    /// не сломать замыкания, переживающие свой кадр вызова.
    fn for_env(env: &Rc<RefCell<Environment>>, value: Value) -> Self {
        match value {
            Value::Closure(closure) if Rc::ptr_eq(&closure.env, env) => Self::WeakClosure {
                params: closure.params,
                body: closure.body,
                env: Rc::downgrade(env),
            },
            other => Self::Value(other),
        }
    }

    /// Возвращает значение для чтения, восстанавливая сильную ссылку.
    fn to_value(&self) -> Result<Value, Error> {
        match self {
            Self::Value(value) => Ok(value.clone()),
            Self::WeakClosure { params, body, env } => {
                let Some(env) = env.upgrade() else {
                    return Err(Error::runtime(
                        "unknown-identifier",
                        "Окружение замыкания уже освобождено",
                    ));
                };
                Ok(Value::Closure(Closure {
                    params: params.clone(),
                    body: body.clone(),
                    env,
                }))
            }
        }
    }

    /// Приблизительный размер связи для отчёта GC.
    fn estimated_bytes(&self) -> usize {
        match self {
            Self::Value(value) => value.estimated_size(),
            Self::WeakClosure { params, body, env } => {
                std::mem::size_of::<Self>()
                    + std::mem::size_of_val(body)
                    + std::mem::size_of_val(env)
                    + params.capacity() * std::mem::size_of::<String>()
                    + params.iter().map(|param| param.capacity()).sum::<usize>()
            }
        }
    }
}

/// Окружение переменных.
#[derive(Debug, Default)]
pub struct Environment {
    /// Родительское окружение (для лексического scoping).
    parent: Option<Rc<RefCell<Environment>>>,
    /// Связи имя → (значение, изменяемость).
    bindings: HashMap<String, (StoredValue, bool)>,
    /// Локальная настройка `lappland/malice`; `None` — наследовать/0.0.
    net_malice: Option<f64>,
    /// Локальная настройка `lappland/delay`; `None` — наследовать/1.0.
    net_delay: Option<f64>,
}

impl Traversable for Environment {
    fn visit_values(&self, visitor: &mut dyn FnMut(&Value)) {
        for (stored, _) in self.bindings.values() {
            if let StoredValue::Value(value) = stored {
                visitor(value);
            }
        }
    }

    fn visit_envs(&self, visitor: &mut dyn FnMut(&Rc<RefCell<Environment>>)) {
        if let Some(parent) = &self.parent {
            visitor(parent);
        }
    }
}

impl Environment {
    /// Создаёт корневое (глобальное) окружение без родителя.
    pub fn new() -> Rc<RefCell<Self>> {
        let env = Rc::new(RefCell::new(Environment {
            parent: None,
            bindings: HashMap::new(),
            net_malice: None,
            net_delay: None,
        }));
        gc::register(&env);
        env
    }

    /// Создаёт дочернее окружение с заданным родителем.
    pub fn new_child(parent: Rc<RefCell<Environment>>) -> Rc<RefCell<Self>> {
        let env = Rc::new(RefCell::new(Environment {
            parent: Some(parent),
            bindings: HashMap::new(),
            net_malice: None,
            net_delay: None,
        }));
        gc::register(&env);
        env
    }

    /// Создаёт окружение, не попадающее в реестр GC.
    ///
    /// Нужно только для временной замены `global` в `Drop` интерпретатора.
    pub(crate) fn new_unregistered() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Environment {
            parent: None,
            bindings: HashMap::new(),
            net_malice: None,
            net_delay: None,
        }))
    }

    /// Родительское окружение (для снимка окружения в `amiya/spawn`).
    pub(crate) fn parent(env: &Rc<RefCell<Environment>>) -> Option<Rc<RefCell<Environment>>> {
        env.borrow().parent.clone()
    }

    /// Устанавливает родителя у ещё не зарегистрированного окружения.
    ///
    /// Используется только при демаршалинге: сначала создаётся placeholder,
    /// затем восстанавливается цепочка родителей и связи.
    pub(crate) fn set_parent_unregistered(
        env: &Rc<RefCell<Environment>>,
        parent: Option<Rc<RefCell<Environment>>>,
    ) {
        env.borrow_mut().parent = parent;
    }

    /// Возвращает связи текущего окружения для снимка в фоновую задачу.
    pub(crate) fn snapshot_entries(
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Vec<(String, Value, bool)>, Error> {
        let this = env.borrow();
        this.bindings
            .iter()
            .map(|(name, (stored, mutable))| Ok((name.clone(), stored.to_value()?, *mutable)))
            .collect()
    }

    /// Локальные (не унаследованные) настройки `lappland`.
    pub(crate) fn net_settings(env: &Rc<RefCell<Environment>>) -> (Option<f64>, Option<f64>) {
        let this = env.borrow();
        (this.net_malice, this.net_delay)
    }

    /// Очищает окружение в sweep-фазе и возвращает (объекты, байты).
    ///
    /// Удаление `bindings` и `parent` разрывает сильные циклы, после чего
    /// обычный `Rc` освобождает недостижимый граф.
    pub(crate) fn gc_clear(&mut self) -> (usize, usize) {
        let mut objects = 1;
        let mut bytes = std::mem::size_of::<Environment>();

        for (name, (stored, _)) in self.bindings.drain() {
            objects += 1;
            bytes += name.capacity()
                + std::mem::size_of::<String>()
                + stored.estimated_bytes()
                + std::mem::size_of::<bool>();
        }
        if self.parent.take().is_some() {
            bytes += std::mem::size_of::<Option<Rc<RefCell<Environment>>>>();
        }

        (objects, bytes)
    }

    /// Устанавливает вероятность инъекции сетевого отказа в этом окружении.
    pub(crate) fn set_net_malice(env: &Rc<RefCell<Environment>>, value: f64) {
        env.borrow_mut().net_malice = Some(value);
    }

    /// Устанавливает множитель задержки сетевых вызовов в этом окружении.
    pub(crate) fn set_net_delay(env: &Rc<RefCell<Environment>>, value: f64) {
        env.borrow_mut().net_delay = Some(value);
    }

    /// Возвращает эффективную политику `(malice, delay)`.
    ///
    /// Поиск идёт по лексической цепочке: ближайшая явная настройка
    /// перекрывает родительскую. По умолчанию `malice = 0.0`, `delay = 1.0`.
    pub(crate) fn net_policy(env: &Rc<RefCell<Environment>>) -> (f64, f64) {
        let mut malice = None;
        let mut delay = None;
        let mut current = Some(env.clone());

        while let Some(item) = current {
            let borrowed = item.borrow();
            if malice.is_none() {
                malice = borrowed.net_malice;
            }
            if delay.is_none() {
                delay = borrowed.net_delay;
            }
            let parent = borrowed.parent.clone();
            drop(borrowed);
            if malice.is_some() && delay.is_some() {
                break;
            }
            current = parent;
        }

        (malice.unwrap_or(0.0), delay.unwrap_or(1.0))
    }

    /// Определяет новую (или перезаписывает) переменную в текущем окружении.
    ///
    /// Если `mutable` равен `false` — переменная неизменяемая, и `set!`
    /// над ней вызовет ошибку.
    pub fn define(env: &Rc<RefCell<Environment>>, name: &str, value: Value, mutable: bool) {
        let stored = StoredValue::for_env(env, value);
        env.borrow_mut()
            .bindings
            .insert(name.to_string(), (stored, mutable));
    }

    /// Возвращает значение переменной, ища её в текущем и родительских окружениях.
    pub fn get(env: &Rc<RefCell<Environment>>, name: &str) -> Result<Value, Error> {
        let this = env.borrow();
        if let Some(binding) = this.bindings.get(name) {
            return binding.0.to_value();
        }

        if let Some(parent) = &this.parent {
            return Self::get(parent, name);
        }

        Err(Error::unknown_identifier(format!(
            "Переменная не определена: {}",
            name
        )))
    }

    /// Возвращает все видимые имена переменных по всей цепочке окружений.
    ///
    /// Только read-only вспомогательная функция для построения подсказок в
    /// сообщениях об ошибках. Не изменяет семантику окружения (scoping).
    pub fn all_names(env: &Rc<RefCell<Environment>>) -> Vec<String> {
        let mut names = Vec::new();
        let mut current = Some(env.clone());
        while let Some(c) = current {
            let this = c.borrow();
            names.extend(this.bindings.keys().cloned());
            current = this.parent.clone();
        }
        names
    }

    /// Устанавливает новое значение переменной через `set!`.
    ///
    /// Ищет первую встретившуюся переменную с заданным именем по цепочке
    /// окружений. Переменная должна быть изменяемой (`define-mutable`),
    /// иначе — ошибка.
    pub fn set(env: &Rc<RefCell<Environment>>, name: &str, value: Value) -> Result<(), Error> {
        let mut this = env.borrow_mut();

        if let Some(binding) = this.bindings.get_mut(name) {
            if !binding.1 {
                return Err(Error::runtime(
                    "immutable-assignment",
                    format!("Переменная {} не изменяемая", name),
                ));
            }
            binding.0 = StoredValue::for_env(env, value);
            return Ok(());
        }

        if let Some(parent) = &this.parent {
            let parent = parent.clone();
            drop(this);
            return Self::set(&parent, name, value);
        }

        Err(Error::unknown_identifier(format!(
            "Переменная не определена: {}",
            name
        )))
    }
}
