//! Окружения переменных с поддержкой лексических областей видимости.
//!
//! Окружение — это иерархическая цепочка связей «имя → значение».
//! Каждое окружение хранит ссылку на родителя; поиск переменной идёт вверх
//! по цепочке. Замыкания захватывают окружение как `Rc<RefCell<Environment>>`,
//! что позволяет `set!` изменять переменные во внешних областях видимости.

use crate::error::Error;
use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Окружение переменных.
#[derive(Debug, Default)]
pub struct Environment {
    /// Родительское окружение (для лексического scoping).
    parent: Option<Rc<RefCell<Environment>>>,
    /// Связи имя → (значение, изменяемость).
    bindings: HashMap<String, (Value, bool)>,
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
        env.borrow_mut()
            .bindings
            .insert(name.to_string(), (value, mutable));
    }

    /// Возвращает значение переменной, ища её в текущем и родительских окружениях.
    pub fn get(env: &Rc<RefCell<Environment>>, name: &str) -> Result<Value, Error> {
        let this = env.borrow();
        if let Some(binding) = this.bindings.get(name) {
            return Ok(binding.0.clone());
        }

        if let Some(parent) = &this.parent {
            return Self::get(parent, name);
        }

        Err(Error::new(format!("Переменная не определена: {}", name)))
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
                return Err(Error::new(format!("Переменная {} не изменяемая", name)));
            }
            binding.0 = value;
            return Ok(());
        }

        if let Some(parent) = &this.parent {
            let parent = parent.clone();
            drop(this);
            return Self::set(&parent, name, value);
        }

        Err(Error::new(format!("Переменная не определена: {}", name)))
    }
}
