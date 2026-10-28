//! Нативный пакет `amiya` (async).
//!
//! Реализация использует `std::thread`, `std::sync::mpsc` и маршалинг
//! [`crate::marshal::SendValue`]. API строго соответствует §3.5:
//! `spawn`, `sleep`, `channel`, `send`, `recv`, `recv-timeout`, `wait`,
//! `wait-all`. Функций `join`/`await` в спецификации нет, поэтому их нет и
//! здесь.
//!
//! `channel` возвращает пару `(tuple sender receiver)`. Для совместимости с
//! примерами §3.5, где один `ch` используется и для `send`, и для `recv`,
//! обе функции принимают как отдельный endpoint, так и всю пару.

use crate::environment::Environment;
use crate::error::Error;
use crate::interpreter::Interpreter;
use crate::marshal::{send_to_value, value_to_send, SendValue};
use crate::value::{value_to_string, Arity, Value};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Разделяемый отправитель канала.
pub struct SenderHandle {
    pub(crate) tx: Sender<SendValue>,
}

impl std::fmt::Debug for SenderHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SenderHandle")
    }
}

/// Разделяемый получатель канала. `Receiver` не является `Sync`, поэтому
/// он защищён мьютексом; это также сериализует конкурентные `recv`.
pub struct ReceiverHandle {
    pub(crate) rx: Arc<Mutex<Receiver<SendValue>>>,
}

impl std::fmt::Debug for ReceiverHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReceiverHandle")
    }
}

/// Результат фоновой задачи.
struct TaskState {
    join: Option<JoinHandle<Result<(), Error>>>,
    result: Option<Result<(), Error>>,
}

/// Opaque-дескриптор задачи. Не маршалится через потоки.
pub struct TaskHandle {
    state: Mutex<TaskState>,
}

impl std::fmt::Debug for TaskHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TaskHandle")
    }
}

impl TaskHandle {
    pub(crate) fn new(join: JoinHandle<Result<(), Error>>) -> Self {
        TaskHandle {
            state: Mutex::new(TaskState {
                join: Some(join),
                result: None,
            }),
        }
    }

    /// Дожидается завершения задачи и пробрасывает её ошибку (включая
    /// `type-mismatch` при возврате немаршалируемого значения).
    pub(crate) fn wait(&self) -> Result<(), Error> {
        let mut state = self.state.lock().map_err(|_| {
            Error::runtime("async-task-failed", "amiya: состояние задачи повреждено")
        })?;

        if let Some(join) = state.join.take() {
            let result = match join.join() {
                Ok(result) => result,
                Err(_) => Err(Error::runtime(
                    "async-task-panicked",
                    "amiya: фоновая задача завершилась паникой",
                )),
            };
            state.result = Some(result.clone());
            return result;
        }

        state.result.clone().unwrap_or(Ok(()))
    }
}

/// Имена пакета.
pub fn names() -> &'static [&'static str] {
    &[
        "channel",
        "recv",
        "recv-timeout",
        "send",
        "sleep",
        "spawn",
        "wait",
        "wait-all",
    ]
}

/// Арность функций пакета.
pub fn arity(name: &str) -> Option<Arity> {
    match name {
        "spawn" | "sleep" | "recv" | "wait" | "wait-all" => Some(Arity::Exact(1)),
        "send" | "recv-timeout" => Some(Arity::Exact(2)),
        "channel" => Some(Arity::Exact(0)),
        _ => None,
    }
}

/// Вызывает функцию пакета.
pub fn call(
    interpreter: &Interpreter,
    _env: &Rc<RefCell<Environment>>,
    name: &str,
    args: &[Value],
) -> Result<Value, Error> {
    match name {
        "spawn" => spawn(interpreter, args),
        "sleep" => sleep(args),
        "channel" => Ok(new_channel()),
        "send" => send(args),
        "recv" => recv(args),
        "recv-timeout" => recv_timeout(args),
        "wait" => wait(args),
        "wait-all" => wait_all(args),
        other => Err(Error::runtime(
            "unknown-module-symbol",
            format!("amiya/{}: неизвестный символ модуля", other),
        )),
    }
}

fn new_channel() -> Value {
    let (tx, rx) = mpsc::channel();
    Value::Tuple(vec![
        Value::Sender(Rc::new(SenderHandle { tx })),
        Value::Receiver(Rc::new(ReceiverHandle {
            rx: Arc::new(Mutex::new(rx)),
        })),
    ])
}

fn sender_from(value: &Value, function: &str) -> Result<Sender<SendValue>, Error> {
    match value {
        Value::Sender(sender) => Ok(sender.tx.clone()),
        Value::Tuple(items) => items
            .iter()
            .find_map(|item| match item {
                Value::Sender(sender) => Some(sender.tx.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                Error::type_mismatch(format!(
                    "amiya/{} ожидает channel/sender, получено {}",
                    function,
                    value_to_string(value)
                ))
            }),
        other => Err(Error::type_mismatch(format!(
            "amiya/{} ожидает channel/sender, получено {}",
            function,
            value_to_string(other)
        ))),
    }
}

fn receiver_from(value: &Value, function: &str) -> Result<Arc<Mutex<Receiver<SendValue>>>, Error> {
    match value {
        Value::Receiver(receiver) => Ok(receiver.rx.clone()),
        Value::Tuple(items) => items
            .iter()
            .find_map(|item| match item {
                Value::Receiver(receiver) => Some(receiver.rx.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                Error::type_mismatch(format!(
                    "amiya/{} ожидает channel/receiver, получено {}",
                    function,
                    value_to_string(value)
                ))
            }),
        other => Err(Error::type_mismatch(format!(
            "amiya/{} ожидает channel/receiver, получено {}",
            function,
            value_to_string(other)
        ))),
    }
}

fn closed_failure() -> Value {
    Value::Failure(Box::new(Value::Str("async: closed".to_string())))
}

fn int_arg(name: &str, args: &[Value], index: usize) -> Result<i64, Error> {
    match args.get(index) {
        Some(Value::Int(value)) => Ok(*value),
        Some(value) => Err(Error::type_mismatch(format!(
            "amiya/{} ожидает int, получено {}",
            name,
            value_to_string(value)
        ))),
        None => Err(Error::arity(format!(
            "amiya/{}: недостаточно аргументов",
            name
        ))),
    }
}

fn spawn(interpreter: &Interpreter, args: &[Value]) -> Result<Value, Error> {
    let Value::Closure(closure) = &args[0] else {
        return Err(Error::type_mismatch(format!(
            "amiya/spawn ожидает функцию, получено {}",
            value_to_string(&args[0])
        )));
    };
    interpreter.spawn_task(closure)
}

fn sleep(args: &[Value]) -> Result<Value, Error> {
    let milliseconds = int_arg("sleep", args, 0)?;
    if milliseconds < 0 {
        return Err(Error::type_mismatch(
            "amiya/sleep: длительность не может быть отрицательной",
        ));
    }
    std::thread::sleep(Duration::from_millis(milliseconds as u64));
    Ok(Value::None)
}

fn send(args: &[Value]) -> Result<Value, Error> {
    let sender = sender_from(&args[0], "send")?;
    let message = value_to_send(&args[1])?;
    match sender.send(message) {
        Ok(()) => Ok(Value::None),
        Err(_) => Ok(closed_failure()),
    }
}

fn recv(args: &[Value]) -> Result<Value, Error> {
    let receiver = receiver_from(&args[0], "recv")?;
    let received = {
        let guard = receiver
            .lock()
            .map_err(|_| Error::runtime("async-channel-failed", "amiya/recv: канал повреждён"))?;
        guard.recv()
    };
    match received {
        Ok(value) => send_to_value(&value),
        Err(_) => Ok(closed_failure()),
    }
}

fn recv_timeout(args: &[Value]) -> Result<Value, Error> {
    let receiver = receiver_from(&args[0], "recv-timeout")?;
    let milliseconds = int_arg("recv-timeout", args, 1)?;
    if milliseconds < 0 {
        return Err(Error::type_mismatch(
            "amiya/recv-timeout: таймаут не может быть отрицательным",
        ));
    }

    let received = {
        let guard = receiver.lock().map_err(|_| {
            Error::runtime(
                "async-channel-failed",
                "amiya/recv-timeout: канал повреждён",
            )
        })?;
        guard.recv_timeout(Duration::from_millis(milliseconds as u64))
    };
    match received {
        Ok(value) => Ok(Value::Some(Box::new(send_to_value(&value)?))),
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(Value::None),
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(closed_failure()),
    }
}

fn wait(args: &[Value]) -> Result<Value, Error> {
    let Value::Task(task) = &args[0] else {
        return Err(Error::type_mismatch(format!(
            "amiya/wait ожидает task, получено {}",
            value_to_string(&args[0])
        )));
    };
    task.wait()?;
    Ok(Value::None)
}

fn wait_all(args: &[Value]) -> Result<Value, Error> {
    let Value::List(tasks) = &args[0] else {
        return Err(Error::type_mismatch(format!(
            "amiya/wait-all ожидает список task, получено {}",
            value_to_string(&args[0])
        )));
    };

    let mut first_error = None;
    for task in tasks {
        let Value::Task(task) = task else {
            return Err(Error::type_mismatch(format!(
                "amiya/wait-all ожидает список task, получено {}",
                value_to_string(task)
            )));
        };
        if let Err(error) = task.wait() {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(Value::None),
    }
}
