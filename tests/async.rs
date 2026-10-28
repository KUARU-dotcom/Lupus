//! Интеграционные тесты V0.6 этапа E: пакет `amiya`, маршалинг и каналы.

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
    let dir = std::env::temp_dir().join(format!("lupus_async_{}_{}", std::process::id(), n));
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

fn error_code(stderr: &str) -> Option<String> {
    let marker = "\"code\":\"";
    let start = stderr.find(marker)? + marker.len();
    let rest = &stderr[start..];
    Some(rest[..rest.find('"')?].to_string())
}

#[test]
fn channel_round_trip_values() {
    let out = run_ok(
        r#"
(import (amiya async) :as async)
(define ch (async/channel))
(define worker
  (async/spawn (lambda ()
    (async/send ch 42)
    (async/send ch "text")
    (async/send ch (list 1 2 3))
    (async/send ch (map (tuple "a" 1) (tuple "b" 2))))))
(print (async/recv ch))
(print (async/recv ch))
(print (async/recv ch))
(print (async/recv ch))
(async/wait worker)
"#,
    );
    assert_eq!(
        out,
        "42\ntext\n(1 2 3)\n(map (tuple \"a\" 1) (tuple \"b\" 2))\n"
    );
}

#[test]
fn parallel_sleep_is_faster_than_sequential() {
    let result = run_with_env(
        r#"
(import (amiya async) :as async)
(define first (async/spawn (lambda () (async/sleep 100))))
(define second (async/spawn (lambda () (async/sleep 100))))
(async/wait-all (list first second))
"#,
        &[],
        Duration::from_secs(5),
    );
    assert!(!result.timed_out, "stderr: {}", result.stderr);
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert!(
        result.elapsed < Duration::from_millis(300),
        "две задачи по 100 мс заняли {:?}",
        result.elapsed
    );
}

#[test]
fn spawn_sees_snapshot_not_later_mutation() {
    let out = run_ok(
        r#"
(import (amiya async) :as async)
(define-mutable x 1)
(define ch (async/channel))
(define worker (async/spawn (lambda () (async/send ch x))))
(set! x 2)
(print (async/recv ch))
(async/wait worker)
"#,
    );
    assert_eq!(out, "1\n");
}

#[test]
fn send_from_main_then_recv_works() {
    let out = run_ok(
        r#"
(import (amiya async) :as async)
(define ch (async/channel))
(async/send ch 7)
(print (async/recv ch))
"#,
    );
    assert_eq!(out, "7\n");
}

#[test]
fn recv_from_closed_channel_returns_failure() {
    let out = run_ok(
        r#"
(import (amiya async) :as async)
(define receiver
  (match (async/channel)
    ((tuple sender rest) rest)))
(print (async/recv receiver))
"#,
    );
    assert_eq!(out, "(failure \"async: closed\")\n");
}

#[test]
fn recv_timeout_returns_none_then_some() {
    let out = run_ok(
        r#"
(import (amiya async) :as async)
(define ch (async/channel))
(print (async/recv-timeout ch 20))
(async/send ch 5)
(print (async/recv-timeout ch 100))
"#,
    );
    assert_eq!(out, "none\n(some 5)\n");
}

#[test]
fn socket_cannot_be_sent_to_channel() {
    let result = run_with_env(
        r#"
(import (texas net) :as net)
(import (amiya async) :as async)
(define ch (async/channel))
(define listener
  (match (net/tcp-listen 0)
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(print (async/send ch listener))
"#,
        &[],
        Duration::from_secs(5),
    );
    assert!(!result.timed_out, "stderr: {}", result.stderr);
    assert_eq!(result.code, 1);
    assert_eq!(error_code(&result.stderr).as_deref(), Some("type-mismatch"));
}

#[test]
fn task_returning_socket_raises_type_mismatch() {
    let result = run_with_env(
        r#"
(import (texas net) :as net)
(import (amiya async) :as async)
(define worker
  (async/spawn (lambda ()
    (match (net/tcp-listen 0)
      ((success listener) listener)
      ((failure message) (failure message))))))
(async/wait worker)
"#,
        &[],
        Duration::from_secs(5),
    );
    assert!(!result.timed_out, "stderr: {}", result.stderr);
    assert_eq!(result.code, 1);
    assert_eq!(error_code(&result.stderr).as_deref(), Some("type-mismatch"));
}

#[test]
fn task_cannot_be_sent_to_channel() {
    let result = run_with_env(
        r#"
(import (amiya async) :as async)
(define ch (async/channel))
(define worker (async/spawn (lambda () none)))
(print (async/send ch worker))
"#,
        &[],
        Duration::from_secs(5),
    );
    assert!(!result.timed_out, "stderr: {}", result.stderr);
    assert_eq!(result.code, 1);
    assert_eq!(error_code(&result.stderr).as_deref(), Some("type-mismatch"));
}

#[test]
fn task_must_return_unit() {
    let result = run_with_env(
        r#"
(import (amiya async) :as async)
(define worker (async/spawn (lambda () 42)))
(async/wait worker)
"#,
        &[],
        Duration::from_secs(5),
    );
    assert!(!result.timed_out, "stderr: {}", result.stderr);
    assert_eq!(result.code, 1);
    assert_eq!(error_code(&result.stderr).as_deref(), Some("type-mismatch"));
}
