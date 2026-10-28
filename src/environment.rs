//! Окружения переменных с поддержкой лексических областей видимости.
//!
//! Окружение — это иерархическая цепочка связей «имя → значение».
//! Каждое окружение хранит ссылку на родителя; поиск переменной идёт вверх
//! по цепочке. Замыкания захватывают окружение как `Rc<RefCell<Environment>>`,
//! что позволяет `set!` изменять переменные во внешних областях видимости.

use crate::ast::Expr;
use crate::error::Error;
use crate::value::{Closure, Value};
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
}

/// Окружение переменных.
#[derive(Debug, Default)]
pub struct Environment {
    /// Родительское окружение (для лексического scoping).
    parent: Option<Rc<RefCell<Environment>>>,
    /// Связи имя → (значение, изменяемость).
    bindings: HashMap<String, (StoredValue, bool)>,
}

impl Environment {
    /// Создаёт корневое (глобальное) окружение без родителя.
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Environment {
            parent: None,
            bindings: HashMap::new(),
        }))
    }

    /// Создаёт дочернее окружение с заданным родителем.
    pub fn new_child(parent: Rc<RefCell<Environment>>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Environment {
            parent: Some(parent),
            bindings: HashMap::new(),
        }))
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
