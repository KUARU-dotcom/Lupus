//! Нативный пакет `texas` (сеть).
//!
//! Реализация использует только `std::net`, без внешних крейтов. Сетевые
//! ошибки не поднимаются как ошибки интерпретатора: все функции, кроме
//! `close`, возвращают Result-значения `(success ...)` / `(failure "net: ...")`.
//!
//! Канонические имена и сигнатуры взяты из §3.3 спецификации
//! (`tcp-connect`, `tcp-listen`, `accept`, `send`, `recv`, `recv-line`,
//! `close`, `udp-bind`, `udp-send-to`, `udp-recv-from`). Дополнительно
//! поддерживаются короткие алиасы `tcp-accept`, `tcp-send`, `tcp-recv`,
//! `udp-socket`, `udp-send`, `udp-recv` с теми же арностями и семантикой.

use crate::builtins::lappland;
use crate::environment::Environment;
use crate::error::Error;
use crate::interpreter::Interpreter;
use crate::value::{value_to_string, Arity, Value};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::rc::Rc;
use std::time::Duration;

/// Таймаут чтения для `recv`/`recv-line`/`udp-recv-from`.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Opaque TCP/UDP-сокет. `Value` клонируется, поэтому дескриптор общий.
#[derive(Debug)]
pub struct Socket {
    inner: RefCell<Option<SocketKind>>,
}

#[derive(Debug)]
enum SocketKind {
    Tcp(TcpStream),
    Udp(UdpSocket),
}

/// Opaque TCP-слушатель.
#[derive(Debug)]
pub struct Listener {
    inner: RefCell<Option<TcpListener>>,
}

impl Socket {
    fn tcp(stream: TcpStream) -> Self {
        Socket {
            inner: RefCell::new(Some(SocketKind::Tcp(stream))),
        }
    }

    fn udp(socket: UdpSocket) -> Self {
        Socket {
            inner: RefCell::new(Some(SocketKind::Udp(socket))),
        }
    }

    fn close(&self) {
        self.inner.borrow_mut().take();
    }
}

impl Listener {
    fn new(listener: TcpListener) -> Self {
        Listener {
            inner: RefCell::new(Some(listener)),
        }
    }

    fn close(&self) {
        self.inner.borrow_mut().take();
    }
}

/// Экспортируемые имена пакета (канонические и короткие алиасы).
pub fn names() -> &'static [&'static str] {
    &[
        "accept",
        "close",
        "recv",
        "recv-line",
        "send",
        "tcp-accept",
        "tcp-connect",
        "tcp-listen",
        "tcp-recv",
        "tcp-send",
        "udp-bind",
        "udp-recv",
        "udp-recv-from",
        "udp-send",
        "udp-send-to",
        "udp-socket",
    ]
}

/// Арность сетевой функции.
pub fn arity(name: &str) -> Option<Arity> {
    match name {
        "tcp-connect" => Some(Arity::Exact(2)),
        "tcp-listen" | "accept" | "tcp-accept" | "recv-line" | "close" | "udp-bind"
        | "udp-socket" => Some(Arity::Exact(1)),
        "send" | "tcp-send" | "recv" | "tcp-recv" | "udp-recv" | "udp-recv-from" => {
            Some(Arity::Exact(2))
        }
        "udp-send" | "udp-send-to" => Some(Arity::Exact(4)),
        _ => None,
    }
}

/// Вызывает функцию пакета.
pub fn call(
    interpreter: &Interpreter,
    env: &Rc<RefCell<Environment>>,
    name: &str,
    args: &[Value],
) -> Result<Value, Error> {
    // `close` не возвращает Result, поэтому инъекция отказа к нему не
    // применяется. Задержка lappland действует на все net-вызовы.
    let returns_result = name != "close";
    if let Some(failure) = lappland::before_net_call(interpreter, env, returns_result) {
        return Ok(failure);
    }

    match name {
        "tcp-connect" => tcp_connect(name, args),
        "tcp-listen" => tcp_listen(name, args),
        "accept" | "tcp-accept" => accept(name, args),
        "send" | "tcp-send" => send(name, args),
        "recv" | "tcp-recv" => recv(name, args),
        "recv-line" => recv_line(name, args),
        "close" => close(args),
        "udp-bind" | "udp-socket" => udp_bind(name, args),
        "udp-send-to" | "udp-send" => udp_send(name, args),
        "udp-recv-from" | "udp-recv" => udp_recv(name, args),
        other => Err(Error::runtime(
            "unknown-module-symbol",
            format!("texas/{}: неизвестный символ модуля", other),
        )),
    }
}

fn string_arg(name: &str, args: &[Value], index: usize) -> Result<String, Error> {
    match args.get(index) {
        Some(Value::Str(value)) => Ok(value.clone()),
        Some(value) => Err(Error::type_mismatch(format!(
            "texas/{} ожидает str, получено {}",
            name,
            value_to_string(value)
        ))),
        None => Err(Error::arity(format!(
            "texas/{}: недостаточно аргументов",
            name
        ))),
    }
}

fn int_arg(name: &str, args: &[Value], index: usize) -> Result<i64, Error> {
    match args.get(index) {
        Some(Value::Int(value)) => Ok(*value),
        Some(value) => Err(Error::type_mismatch(format!(
            "texas/{} ожидает int, получено {}",
            name,
            value_to_string(value)
        ))),
        None => Err(Error::arity(format!(
            "texas/{}: недостаточно аргументов",
            name
        ))),
    }
}

fn port_arg(name: &str, args: &[Value], index: usize) -> Result<u16, Error> {
    let port = int_arg(name, args, index)?;
    u16::try_from(port).map_err(|_| {
        Error::type_mismatch(format!(
            "texas/{}: порт должен быть в диапазоне 0..=65535, получено {}",
            name, port
        ))
    })
}

fn socket_arg<'a>(name: &str, value: &'a Value) -> Result<&'a Rc<Socket>, Error> {
    match value {
        Value::Socket(socket) => Ok(socket),
        other => Err(Error::type_mismatch(format!(
            "texas/{} ожидает socket, получено {}",
            name,
            value_to_string(other)
        ))),
    }
}

fn listener_arg<'a>(name: &str, value: &'a Value) -> Result<&'a Rc<Listener>, Error> {
    match value {
        Value::Listener(listener) => Ok(listener),
        other => Err(Error::type_mismatch(format!(
            "texas/{} ожидает listener (результат tcp-listen), получено {}",
            name,
            value_to_string(other)
        ))),
    }
}

fn success(value: Value) -> Value {
    Value::Success(Box::new(value))
}

fn net_failure(message: impl std::fmt::Display) -> Value {
    Value::Failure(Box::new(Value::Str(format!("net: {}", message))))
}

fn io_failure(error: std::io::Error) -> Value {
    if matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ) {
        net_failure("timeout")
    } else {
        net_failure(error)
    }
}

fn with_read_timeout(stream: &TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))
}

fn tcp_connect(name: &str, args: &[Value]) -> Result<Value, Error> {
    let host = string_arg(name, args, 0)?;
    let port = port_arg(name, args, 1)?;
    Ok(match TcpStream::connect((host.as_str(), port)) {
        Ok(stream) => match with_read_timeout(&stream) {
            Ok(()) => success(Value::Socket(Rc::new(Socket::tcp(stream)))),
            Err(error) => io_failure(error),
        },
        Err(error) => io_failure(error),
    })
}

fn tcp_listen(name: &str, args: &[Value]) -> Result<Value, Error> {
    let port = port_arg(name, args, 0)?;
    Ok(match TcpListener::bind(("0.0.0.0", port)) {
        Ok(listener) => success(Value::Listener(Rc::new(Listener::new(listener)))),
        Err(error) => io_failure(error),
    })
}

fn accept(name: &str, args: &[Value]) -> Result<Value, Error> {
    let listener = listener_arg(name, &args[0])?;
    let borrowed = listener.inner.borrow();
    let Some(listener) = borrowed.as_ref() else {
        return Ok(net_failure("listener is closed"));
    };
    Ok(match listener.accept() {
        Ok((stream, _address)) => match with_read_timeout(&stream) {
            Ok(()) => success(Value::Socket(Rc::new(Socket::tcp(stream)))),
            Err(error) => io_failure(error),
        },
        Err(error) => io_failure(error),
    })
}

fn send(name: &str, args: &[Value]) -> Result<Value, Error> {
    let socket = socket_arg(name, &args[0])?;
    let data = string_arg(name, args, 1)?;
    let mut inner = socket.inner.borrow_mut();
    let Some(kind) = inner.as_mut() else {
        return Ok(net_failure("socket is closed"));
    };
    match kind {
        SocketKind::Tcp(stream) => Ok(match stream.write_all(data.as_bytes()) {
            Ok(()) => success(Value::Int(data.len() as i64)),
            Err(error) => io_failure(error),
        }),
        SocketKind::Udp(_) => Err(Error::type_mismatch(
            "texas/send ожидает TCP-socket; для UDP используйте udp-send-to",
        )),
    }
}

fn recv(name: &str, args: &[Value]) -> Result<Value, Error> {
    let socket = socket_arg(name, &args[0])?;
    let limit = int_arg(name, args, 1)?;
    if limit < 0 {
        return Err(Error::type_mismatch(
            "texas/recv: размер буфера не может быть отрицательным",
        ));
    }
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let mut inner = socket.inner.borrow_mut();
    let Some(kind) = inner.as_mut() else {
        return Ok(net_failure("socket is closed"));
    };
    let SocketKind::Tcp(stream) = kind else {
        return Err(Error::type_mismatch(
            "texas/recv ожидает TCP-socket; для UDP используйте udp-recv-from",
        ));
    };
    if let Err(error) = with_read_timeout(stream) {
        return Ok(io_failure(error));
    }
    let mut buffer = vec![0u8; limit];
    Ok(match stream.read(&mut buffer) {
        Ok(read) => success(Value::Str(
            String::from_utf8_lossy(&buffer[..read]).into_owned(),
        )),
        Err(error) => io_failure(error),
    })
}

fn recv_line(name: &str, args: &[Value]) -> Result<Value, Error> {
    let socket = socket_arg(name, &args[0])?;
    let mut inner = socket.inner.borrow_mut();
    let Some(kind) = inner.as_mut() else {
        return Ok(net_failure("socket is closed"));
    };
    let SocketKind::Tcp(stream) = kind else {
        return Err(Error::type_mismatch("texas/recv-line ожидает TCP-socket"));
    };
    if let Err(error) = with_read_timeout(stream) {
        return Ok(io_failure(error));
    }

    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }
                line.push(byte[0]);
            }
            Err(error) => return Ok(io_failure(error)),
        }
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    Ok(success(Value::Str(
        String::from_utf8_lossy(&line).into_owned(),
    )))
}

fn close(args: &[Value]) -> Result<Value, Error> {
    match &args[0] {
        Value::Socket(socket) => socket.close(),
        Value::Listener(listener) => listener.close(),
        other => {
            return Err(Error::type_mismatch(format!(
                "texas/close ожидает socket, получено {}",
                value_to_string(other)
            )))
        }
    }
    Ok(Value::None)
}

fn udp_bind(name: &str, args: &[Value]) -> Result<Value, Error> {
    let port = port_arg(name, args, 0)?;
    Ok(match UdpSocket::bind(("0.0.0.0", port)) {
        Ok(socket) => match socket.set_read_timeout(Some(READ_TIMEOUT)) {
            Ok(()) => success(Value::Socket(Rc::new(Socket::udp(socket)))),
            Err(error) => io_failure(error),
        },
        Err(error) => io_failure(error),
    })
}

fn udp_send(name: &str, args: &[Value]) -> Result<Value, Error> {
    let socket = socket_arg(name, &args[0])?;
    let host = string_arg(name, args, 1)?;
    let port = port_arg(name, args, 2)?;
    let data = string_arg(name, args, 3)?;
    let inner = socket.inner.borrow();
    let Some(kind) = inner.as_ref() else {
        return Ok(net_failure("socket is closed"));
    };
    let SocketKind::Udp(socket) = kind else {
        return Err(Error::type_mismatch("texas/udp-send-to ожидает UDP-socket"));
    };
    Ok(
        match socket.send_to(data.as_bytes(), (host.as_str(), port)) {
            Ok(sent) => success(Value::Int(sent as i64)),
            Err(error) => io_failure(error),
        },
    )
}

fn udp_recv(name: &str, args: &[Value]) -> Result<Value, Error> {
    let socket = socket_arg(name, &args[0])?;
    let limit = int_arg(name, args, 1)?;
    if limit < 0 {
        return Err(Error::type_mismatch(
            "texas/udp-recv-from: размер буфера не может быть отрицательным",
        ));
    }
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let inner = socket.inner.borrow();
    let Some(kind) = inner.as_ref() else {
        return Ok(net_failure("socket is closed"));
    };
    let SocketKind::Udp(socket) = kind else {
        return Err(Error::type_mismatch(
            "texas/udp-recv-from ожидает UDP-socket",
        ));
    };
    if let Err(error) = socket.set_read_timeout(Some(READ_TIMEOUT)) {
        return Ok(io_failure(error));
    }
    let mut buffer = vec![0u8; limit];
    Ok(match socket.recv_from(&mut buffer) {
        Ok((read, address)) => success(Value::Tuple(vec![
            Value::Str(address.ip().to_string()),
            Value::Int(address.port() as i64),
            Value::Str(String::from_utf8_lossy(&buffer[..read]).into_owned()),
        ])),
        Err(error) => io_failure(error),
    })
}
