//! Маршалинг значений Lupus между потоками.
//!
//! `Value` содержит `Rc`/`RefCell` и не является `Send`. Для фоновых задач
//! строится зеркало [`SendValue`] на `Arc`/`Mutex`, которое можно передать в
//! `std::thread` или через `std::sync::mpsc`.
//!
//! Снимок окружения делается в момент `spawn`: последующие `set!` в исходном
//! окружении на задачу не влияют. Socket/Listener/Task не маршалируются; при
//! явной передаче в канал или возврате из задачи это даёт `type-mismatch`.
//! При снимке окружения такие связи пропускаются, чтобы наличие неиспользуемого
//! сокета/задачи в глобальном окружении не мешало запускать другие задачи.

use crate::ast::Expr;
use crate::builtins::amiya::{ReceiverHandle, SenderHandle};
use crate::environment::Environment;
use crate::error::Error;
use crate::gc;
use crate::value::{Arity, Closure, NativeImpl, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

/// Потокобезопасное зеркало [`Value`].
#[derive(Clone, Debug)]
pub enum SendValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    List(Vec<SendValue>),
    Tuple(Vec<SendValue>),
    Map(Vec<(SendValue, SendValue)>),
    Struct {
        name: String,
        fields: Vec<SendValue>,
    },
    Sender(Sender<SendValue>),
    Receiver(Arc<Mutex<Receiver<SendValue>>>),
    None,
    Some(Box<SendValue>),
    Success(Box<SendValue>),
    Failure(Box<SendValue>),
    Closure(SendClosure),
    Native {
        name: String,
        arity: Arity,
        func: NativeImpl,
    },
}

/// Потокобезопасное замыкание: AST плюс снимок захваченного окружения.
#[derive(Clone, Debug)]
pub struct SendClosure {
    pub params: Vec<String>,
    pub body: Expr,
    pub env: Arc<Mutex<SendEnvironment>>,
}

/// Снимок одного окружения.
#[derive(Clone, Debug, Default)]
pub struct SendEnvironment {
    pub parent: Option<Arc<Mutex<SendEnvironment>>>,
    pub bindings: Vec<(String, SendValue, bool)>,
    pub net_malice: Option<f64>,
    pub net_delay: Option<f64>,
}

/// Импортированный модуль в потокобезопасном виде.
#[derive(Clone, Debug)]
pub enum SendImport {
    Native { package: String },
    File(Vec<(String, SendValue)>),
}

/// Конвертер `Value` → `SendValue` с сохранением идентичности окружений.
#[derive(Default)]
pub struct Marshaler {
    envs: HashMap<usize, Arc<Mutex<SendEnvironment>>>,
}

impl Marshaler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Строгая конвертация: Socket/Listener/Task дают `type-mismatch`.
    pub fn to_send(&mut self, value: &Value) -> Result<SendValue, Error> {
        match value {
            Value::Int(value) => Ok(SendValue::Int(*value)),
            Value::Float(value) => Ok(SendValue::Float(*value)),
            Value::Bool(value) => Ok(SendValue::Bool(*value)),
            Value::Str(value) => Ok(SendValue::Str(value.clone())),
            Value::List(items) => Ok(SendValue::List(
                items
                    .iter()
                    .map(|item| self.to_send(item))
                    .collect::<Result<_, _>>()?,
            )),
            Value::Tuple(items) => Ok(SendValue::Tuple(
                items
                    .iter()
                    .map(|item| self.to_send(item))
                    .collect::<Result<_, _>>()?,
            )),
            Value::Map(pairs) => Ok(SendValue::Map(
                pairs
                    .iter()
                    .map(|(key, value)| Ok((self.to_send(key)?, self.to_send(value)?)))
                    .collect::<Result<_, Error>>()?,
            )),
            Value::Struct { name, fields } => Ok(SendValue::Struct {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|field| self.to_send(field))
                    .collect::<Result<_, _>>()?,
            }),
            Value::Socket(_) => Err(opaque_error("socket")),
            Value::Listener(_) => Err(opaque_error("listener")),
            Value::Task(_) => Err(opaque_error("task")),
            Value::Sender(sender) => Ok(SendValue::Sender(sender.tx.clone())),
            Value::Receiver(receiver) => Ok(SendValue::Receiver(receiver.rx.clone())),
            Value::None => Ok(SendValue::None),
            Value::Some(value) => Ok(SendValue::Some(Box::new(self.to_send(value)?))),
            Value::Success(value) => Ok(SendValue::Success(Box::new(self.to_send(value)?))),
            Value::Failure(value) => Ok(SendValue::Failure(Box::new(self.to_send(value)?))),
            Value::Closure(closure) => {
                let env = self.env_to_send(&closure.env)?;
                Ok(SendValue::Closure(SendClosure {
                    params: closure.params.clone(),
                    body: closure.body.clone(),
                    env,
                }))
            }
            Value::Native { name, arity, func } => Ok(SendValue::Native {
                name: name.clone(),
                arity: *arity,
                func: func.clone(),
            }),
        }
    }

    /// Снимок окружения. Немаршалируемые связи пропускаются.
    pub fn env_to_send(
        &mut self,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Arc<Mutex<SendEnvironment>>, Error> {
        let key = Rc::as_ptr(env) as usize;
        if let Some(existing) = self.envs.get(&key) {
            return Ok(existing.clone());
        }

        let placeholder = Arc::new(Mutex::new(SendEnvironment::default()));
        self.envs.insert(key, placeholder.clone());

        let parent = match Environment::parent(env) {
            Some(parent) => Some(self.env_to_send(&parent)?),
            None => None,
        };

        let mut bindings = Vec::new();
        for (name, value, mutable) in Environment::snapshot_entries(env)? {
            if !is_marshalable(&value) {
                continue;
            }
            bindings.push((name, self.to_send(&value)?, mutable));
        }

        let (net_malice, net_delay) = Environment::net_settings(env);
        let mut guard = placeholder.lock().map_err(|_| {
            Error::runtime("async-marshal-failed", "Маршалинг: окружение повреждено")
        })?;
        *guard = SendEnvironment {
            parent,
            bindings,
            net_malice,
            net_delay,
        };
        drop(guard);

        Ok(placeholder)
    }
}

/// Конвертер `SendValue` → `Value`.
#[derive(Default)]
pub struct Unmarshaler {
    envs: HashMap<usize, Rc<RefCell<Environment>>>,
    sent_envs: Vec<Arc<Mutex<SendEnvironment>>>,
}

impl Unmarshaler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn to_value(&mut self, value: &SendValue) -> Result<Value, Error> {
        match value {
            SendValue::Int(value) => Ok(Value::Int(*value)),
            SendValue::Float(value) => Ok(Value::Float(*value)),
            SendValue::Bool(value) => Ok(Value::Bool(*value)),
            SendValue::Str(value) => Ok(Value::Str(value.clone())),
            SendValue::List(items) => Ok(Value::List(
                items
                    .iter()
                    .map(|item| self.to_value(item))
                    .collect::<Result<_, _>>()?,
            )),
            SendValue::Tuple(items) => Ok(Value::Tuple(
                items
                    .iter()
                    .map(|item| self.to_value(item))
                    .collect::<Result<_, _>>()?,
            )),
            SendValue::Map(pairs) => Ok(Value::Map(
                pairs
                    .iter()
                    .map(|(key, value)| Ok((self.to_value(key)?, self.to_value(value)?)))
                    .collect::<Result<_, Error>>()?,
            )),
            SendValue::Struct { name, fields } => Ok(Value::Struct {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|field| self.to_value(field))
                    .collect::<Result<_, _>>()?,
            }),
            SendValue::Sender(sender) => {
                Ok(Value::Sender(Rc::new(SenderHandle { tx: sender.clone() })))
            }
            SendValue::Receiver(receiver) => Ok(Value::Receiver(Rc::new(ReceiverHandle {
                rx: receiver.clone(),
            }))),
            SendValue::None => Ok(Value::None),
            SendValue::Some(value) => Ok(Value::Some(Box::new(self.to_value(value)?))),
            SendValue::Success(value) => Ok(Value::Success(Box::new(self.to_value(value)?))),
            SendValue::Failure(value) => Ok(Value::Failure(Box::new(self.to_value(value)?))),
            SendValue::Closure(closure) => {
                let env = self.env_from_send(&closure.env)?;
                Ok(Value::Closure(Closure {
                    params: closure.params.clone(),
                    body: closure.body.clone(),
                    env,
                }))
            }
            SendValue::Native { name, arity, func } => Ok(Value::Native {
                name: name.clone(),
                arity: *arity,
                func: func.clone(),
            }),
        }
    }

    fn env_from_send(
        &mut self,
        send: &Arc<Mutex<SendEnvironment>>,
    ) -> Result<Rc<RefCell<Environment>>, Error> {
        let key = Arc::as_ptr(send) as usize;
        if let Some(existing) = self.envs.get(&key) {
            return Ok(existing.clone());
        }

        let env = Environment::new_unregistered();
        self.envs.insert(key, env.clone());
        self.sent_envs.push(send.clone());

        let snapshot = send
            .lock()
            .map_err(|_| {
                Error::runtime("async-marshal-failed", "Демаршалинг: окружение повреждено")
            })?
            .clone();

        if let Some(parent) = &snapshot.parent {
            let parent = self.env_from_send(parent)?;
            Environment::set_parent_unregistered(&env, Some(parent));
        }

        for (name, value, mutable) in &snapshot.bindings {
            let value = self.to_value(value)?;
            Environment::define(&env, name, value, *mutable);
        }
        if let Some(malice) = snapshot.net_malice {
            Environment::set_net_malice(&env, malice);
        }
        if let Some(delay) = snapshot.net_delay {
            Environment::set_net_delay(&env, delay);
        }

        gc::register(&env);
        Ok(env)
    }

    /// Разрывает `Arc`-циклы снимка после успешного демаршалинга.
    pub fn break_cycles(&mut self) {
        for env in self.sent_envs.drain(..) {
            if let Ok(mut env) = env.lock() {
                env.parent = None;
                env.bindings.clear();
            }
        }
    }
}

fn opaque_error(kind: &str) -> Error {
    Error::type_mismatch(format!(
        "Значение {} нельзя передать через границу потока",
        kind
    ))
}

/// Проверяет, можно ли включить значение в снимок окружения.
pub(crate) fn is_marshalable(value: &Value) -> bool {
    match value {
        Value::Socket(_) | Value::Listener(_) | Value::Task(_) => false,
        Value::List(items) | Value::Tuple(items) => items.iter().all(is_marshalable),
        Value::Map(pairs) => pairs
            .iter()
            .all(|(key, value)| is_marshalable(key) && is_marshalable(value)),
        Value::Struct { fields, .. } => fields.iter().all(is_marshalable),
        Value::Some(value) | Value::Success(value) | Value::Failure(value) => is_marshalable(value),
        Value::Int(_)
        | Value::Float(_)
        | Value::Bool(_)
        | Value::Str(_)
        | Value::None
        | Value::Sender(_)
        | Value::Receiver(_)
        | Value::Native { .. }
        | Value::Closure(_) => true,
    }
}

/// Удобная строгая конвертация для канала/результата задачи.
pub fn value_to_send(value: &Value) -> Result<SendValue, Error> {
    Marshaler::new().to_send(value)
}

/// Удобная обратная конвертация.
pub fn send_to_value(value: &SendValue) -> Result<Value, Error> {
    Unmarshaler::new().to_value(value)
}
