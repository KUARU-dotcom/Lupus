//! Интеграционные тесты V0.6 этапа D: сетевой пакет `texas` и инъекция
//! отказов `lappland`.

use std::net::{TcpListener, UdpSocket};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct RunResult {
    stdout: String,
    stderr: String,
    code: i32,
    elapsed: Duration,
    timed_out: bool,
}

fn temp_program(source: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lupus_net_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("main.lupus");
    std::fs::write(&path, source).expect("write program");
    path
}

fn run_with_env(source: &str, envs: &[(&str, &str)], timeout: Duration) -> RunResult {
    let path = temp_program(source);
    let mut command = Command::new(env!("CARGO_BIN_EXE_lupus"));
    command
        .arg(&path)
        .env_remove("LUPUS_MALICE_SEED")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }

    let started = Instant::now();
    let mut child = command.spawn().expect("spawn lupus");
    let status = loop {
        match child.try_wait().expect("wait lupus") {
            Some(status) => break Some(status),
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    };

    let elapsed = started.elapsed();
    let output = child.wait_with_output().expect("collect output");
    let _ = std::fs::remove_file(&path);
    RunResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        code: status.and_then(|status| status.code()).unwrap_or(-1),
        elapsed,
        timed_out: status.is_none(),
    }
}

fn run_ok(source: &str) -> String {
    let result = run_with_env(source, &[], Duration::from_secs(10));
    assert!(!result.timed_out, "программа зависла: {}", result.stderr);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    result.stdout
}

fn free_tcp_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("free tcp port");
    listener.local_addr().expect("tcp addr").port()
}

fn free_udp_port() -> u16 {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("free udp port");
    socket.local_addr().expect("udp addr").port()
}

fn error_code(stderr: &str) -> Option<String> {
    let marker = "\"code\":\"";
    let start = stderr.find(marker)? + marker.len();
    let rest = &stderr[start..];
    Some(rest[..rest.find('"')?].to_string())
}

#[test]
fn tcp_echo_loopback() {
    let port = free_tcp_port();
    let out = run_ok(&format!(
        r#"
(import (texas net) :as net)
(define listener
  (match (net/tcp-listen {port})
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(define client
  (match (net/tcp-connect "127.0.0.1" {port})
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(print (match (net/send client "hello")
  ((success sent) sent)
  ((failure message) (print message) (exit 1))))
(define accepted
  (match (net/accept listener)
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(define received
  (match (net/recv accepted 64)
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(print received)
(print (match (net/send accepted received)
  ((success sent) sent)
  ((failure message) (print message) (exit 1))))
(print (match (net/recv client 64)
  ((success value) value)
  ((failure message) message)))
(net/close accepted)
(net/close client)
(net/close listener)
"#
    ));
    assert_eq!(out, "5\nhello\n5\nhello\n");
}

#[test]
fn udp_round_trip() {
    let port = free_udp_port();
    let out = run_ok(&format!(
        r#"
(import (texas net) :as net)
(define socket
  (match (net/udp-bind {port})
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(print (match (net/udp-send-to socket "127.0.0.1" {port} "pong")
  ((success sent) sent)
  ((failure message) (print message) (exit 1))))
(print (match (net/udp-recv-from socket 64)
  ((success (tuple host source-port data))
    (string-append host ":" (int->str source-port) ":" data))
  ((failure message) message)))
(net/close socket)
"#
    ));
    assert_eq!(out, format!("4\n127.0.0.1:{port}:pong\n"));
}

#[test]
fn tcp_connect_closed_port_is_failure_with_exit_zero() {
    let port = free_tcp_port();
    let result = run_with_env(
        &format!(
            r#"
(import (texas net) :as net)
(print (match (net/tcp-connect "127.0.0.1" {port})
  ((success _) "unexpected-success")
  ((failure message) message)))
"#
        ),
        &[],
        Duration::from_secs(10),
    );
    assert!(!result.timed_out);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert!(
        result.stdout.starts_with("net: "),
        "ожидался net-failure, получено: {}",
        result.stdout
    );
}

#[test]
fn malice_one_injects_every_net_call() {
    let port = free_tcp_port();
    let out = run_ok(&format!(
        r#"
(import lappland)
(import (texas net) :as net)
(lappland/malice 1.0)
(print (match (net/tcp-listen {port}) ((success _) "success") ((failure message) message)))
(print (match (net/tcp-connect "127.0.0.1" {port}) ((success _) "success") ((failure message) message)))
(print (match (net/udp-bind {port}) ((success _) "success") ((failure message) message)))
"#
    ));
    assert_eq!(
        out,
        "net: injected malice\nnet: injected malice\nnet: injected malice\n"
    );
}

#[test]
fn malice_zero_never_injects() {
    let port = free_tcp_port();
    let out = run_ok(&format!(
        r#"
(import lappland)
(import (texas net) :as net)
(lappland/malice 0.0)
(print (match (net/tcp-connect "127.0.0.1" {port})
  ((success _) "unexpected-success")
  ((failure message) (if (= message "net: injected malice") "injected" "clean"))))
"#
    ));
    assert_eq!(out, "clean\n");
}

#[test]
fn tcp_recv_timeout_is_five_seconds() {
    let port = free_tcp_port();
    let result = run_with_env(
        &format!(
            r#"
(import (texas net) :as net)
(define listener
  (match (net/tcp-listen {port})
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(define client
  (match (net/tcp-connect "127.0.0.1" {port})
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(define accepted
  (match (net/accept listener)
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(print (match (net/recv accepted 16)
  ((success value) value)
  ((failure message) message)))
(net/close client)
"#
        ),
        &[],
        Duration::from_secs(10),
    );
    assert!(!result.timed_out);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert_eq!(result.stdout, "net: timeout\n");
    assert!(
        result.elapsed >= Duration::from_secs(4) && result.elapsed <= Duration::from_secs(6),
        "recv timeout занял {:?}, ожидалось около 5 с",
        result.elapsed
    );
}

#[test]
fn udp_recv_timeout_is_five_seconds() {
    let port = free_udp_port();
    let result = run_with_env(
        &format!(
            r#"
(import (texas net) :as net)
(define socket
  (match (net/udp-bind {port})
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(print (match (net/udp-recv-from socket 16)
  ((success value) value)
  ((failure message) message)))
"#
        ),
        &[],
        Duration::from_secs(10),
    );
    assert!(!result.timed_out);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert_eq!(result.stdout, "net: timeout\n");
    assert!(
        result.elapsed >= Duration::from_secs(4) && result.elapsed <= Duration::from_secs(6),
        "udp timeout занял {:?}, ожидалось около 5 с",
        result.elapsed
    );
}

#[test]
fn malice_out_of_range_is_type_mismatch() {
    let result = run_with_env(
        "(import lappland)\n(lappland/malice 2.0)\n",
        &[],
        Duration::from_secs(5),
    );
    assert_eq!(result.code, 1);
    assert_eq!(error_code(&result.stderr).as_deref(), Some("type-mismatch"));
}

#[test]
fn delay_below_one_is_type_mismatch() {
    let result = run_with_env(
        "(import lappland)\n(lappland/delay 0.5)\n",
        &[],
        Duration::from_secs(5),
    );
    assert_eq!(result.code, 1);
    assert_eq!(error_code(&result.stderr).as_deref(), Some("type-mismatch"));
}

#[test]
fn malice_seed_42_is_deterministic() {
    let source = r#"
(import lappland)
(import (texas net) :as net)
(define (probe)
  (match (net/tcp-connect "127.0.0.1" 0)
    ((success _) (print "open"))
    ((failure message)
      (print (if (= message "net: injected malice") "I" "N")))))
(lappland/malice 0.5)
(probe)
(probe)
(probe)
(probe)
(probe)
(probe)
(probe)
(probe)
"#;
    let first = run_with_env(
        source,
        &[("LUPUS_MALICE_SEED", "42")],
        Duration::from_secs(10),
    );
    let second = run_with_env(
        source,
        &[("LUPUS_MALICE_SEED", "42")],
        Duration::from_secs(10),
    );
    assert_eq!(first.code, 0, "stderr: {}", first.stderr);
    assert_eq!(second.code, 0, "stderr: {}", second.stderr);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(first.stdout, "I\nN\nN\nN\nN\nN\nI\nI\n");
}

#[test]
fn short_texas_aliases_work() {
    let port = free_tcp_port();
    let out = run_ok(&format!(
        r#"
(import (texas net) :as net)
(define listener
  (match (net/tcp-listen {port})
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(define client
  (match (net/tcp-connect "127.0.0.1" {port})
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(net/tcp-send client "alias")
(define accepted
  (match (net/tcp-accept listener)
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(define received
  (match (net/tcp-recv accepted 64)
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(print received)
(net/tcp-send accepted "alias\n")
(print (match (net/recv-line client)
  ((success value) value)
  ((failure failure-message) failure-message)))
(net/close accepted)
(net/close client)
(net/close listener)
"#
    ));
    assert_eq!(out, "alias\nalias\n");
}
