//! V0.6 этап F: host-API kennel, профили, fuel и `eval --import`.

use lupus::kennel::{self, Config, Host, JsonError, Profile};
use lupus::value::{Arity, Value};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn run(source: &str, config: Config) -> Result<String, JsonError> {
    kennel::run_program(source, &config, &Host::new())
}

/// Выполняет программу в отдельном потоке: если лимит не сработает,
/// тест упадёт по таймауту, а не зависнет навсегда.
fn run_with_timeout(
    source: &str,
    config: Config,
    host: Host,
    timeout: Duration,
) -> Result<String, JsonError> {
    let source = source.to_string();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(kennel::run_program(&source, &config, &host));
    });
    rx.recv_timeout(timeout)
        .expect("программа не завершилась за отведённое время")
}

fn temp_program(source: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lupus_kennel_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("main.lupus");
    std::fs::write(&path, source).expect("write program");
    path
}

struct CliResult {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run_cli(args: &[&str], envs: &[(&str, &str)], timeout: Duration) -> CliResult {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lupus"));
    command
        .args(args)
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
    let output = child.wait_with_output().expect("collect output");
    CliResult {
        code: status.and_then(|status| status.code()).unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

fn extract_code(stderr: &str) -> Option<String> {
    let marker = "\"code\":\"";
    let start = stderr.find(marker)? + marker.len();
    let rest = &stderr[start..];
    Some(rest[..rest.find('"')?].to_string())
}

#[test]
fn kennel_pure_allows_core() {
    let config = Config {
        profile: Profile::Pure,
        ..Config::default()
    };
    assert_eq!(run("(print (+ 2 3))", config).expect("core"), "5\n");
}

#[test]
fn kennel_pure_denies_kaltsit() {
    let config = Config {
        profile: Profile::Pure,
        ..Config::default()
    };
    let error = run("(import kaltsit)", config).expect_err("kaltsit запрещён");
    assert_eq!(error.code(), "profile-denied");
    assert!(error.message().contains("kaltsit"), "{}", error.message);
}

#[test]
fn kennel_core_allows_kaltsit() {
    let config = Config {
        profile: Profile::Core,
        ..Config::default()
    };
    let output = run(
        "(import kaltsit)\n(print (kaltsit/exists? \"README.md\"))",
        config,
    )
    .expect("kaltsit разрешён в core");
    assert_eq!(output, "true\n");
}

#[test]
fn kennel_core_denies_texas() {
    let config = Config {
        profile: Profile::Core,
        ..Config::default()
    };
    let error = run("(import texas)", config).expect_err("texas запрещён");
    assert_eq!(error.code(), "profile-denied");
    assert!(error.message().contains("texas"), "{}", error.message);
}

#[test]
fn kennel_full_host_native_double() {
    let mut host = Host::new();
    host.register_native("double", Arity::Exact(1), |args| match &args[0] {
        Value::Int(value) => Ok(Value::Int(value * 2)),
        _ => Err("host/double ожидает int".to_string()),
    });
    let config = Config {
        profile: Profile::Full,
        ..Config::default()
    };
    let output = kennel::run_program("(print (host/double 21))", &config, &host).expect("host");
    assert_eq!(output, "42\n");
}

#[test]
fn kennel_full_host_without_registration() {
    let config = Config {
        profile: Profile::Full,
        ..Config::default()
    };
    let error = run("(host/missing 1)", config).expect_err("host не зарегистрирован");
    assert_eq!(error.code(), "unknown-module-symbol");
}

#[test]
fn kennel_host_denied_outside_full() {
    let mut host = Host::new();
    host.register_native("double", Arity::Exact(1), |args| match &args[0] {
        Value::Int(value) => Ok(Value::Int(value * 2)),
        _ => Err("host/double ожидает int".to_string()),
    });
    let config = Config {
        profile: Profile::Core,
        ..Config::default()
    };
    let error = kennel::run_program("(host/double 21)", &config, &host).expect_err("host запрещён");
    assert_eq!(error.code(), "profile-denied");
    assert!(error.message().contains("host"), "{}", error.message);
}

#[test]
fn kennel_max_steps_exhausts_loop() {
    let config = Config {
        max_steps: Some(1000),
        ..Config::default()
    };
    let error = run_with_timeout(
        "(define-mutable i 0)\n(while true (set! i (+ i 1)))",
        config,
        Host::new(),
        Duration::from_secs(5),
    )
    .expect_err("ожидался exhausted");
    assert_eq!(error.code(), "exhausted");
}

#[test]
fn kennel_max_steps_exhausts_recursion() {
    let config = Config {
        max_steps: Some(50),
        ..Config::default()
    };
    let error = run_with_timeout(
        "(define (f n) (+ 1 (f n)))\n(f 0)",
        config,
        Host::new(),
        Duration::from_secs(5),
    )
    .expect_err("ожидался exhausted");
    assert_eq!(error.code(), "exhausted");
}

#[test]
fn kennel_set_fuel_equal_limit_allowed() {
    let config = Config {
        max_steps: Some(1000),
        ..Config::default()
    };
    let output = run("(set-fuel! 1000)\n(print 1)", config).expect("равный потолок разрешён");
    assert_eq!(output, "1\n");
}

#[test]
fn kennel_set_fuel_can_decrease() {
    let config = Config {
        max_steps: Some(1000),
        ..Config::default()
    };
    let error = run_with_timeout(
        "(set-fuel! 10)\n(define-mutable i 0)\n(while true (set! i (+ i 1)))",
        config,
        Host::new(),
        Duration::from_secs(5),
    )
    .expect_err("ожидался exhausted");
    assert_eq!(error.code(), "exhausted");
}

#[test]
fn kennel_set_fuel_raise_denied() {
    let config = Config {
        max_steps: Some(1000),
        ..Config::default()
    };
    let error = run("(set-fuel! 99999999)", config).expect_err("поднять fuel нельзя");
    assert_eq!(error.code(), "fuel-raise-denied");
}

#[test]
fn kennel_max_depth_exceeded() {
    let config = Config {
        max_depth: Some(100),
        ..Config::default()
    };
    let error = run(
        "(define (f n) (if (= n 0) 0 (f (- n 1))))\n(f 1000)",
        config,
    )
    .expect_err("ожидался stack-depth-exceeded");
    assert_eq!(error.code(), "stack-depth-exceeded");
    assert!(error.message().contains("100"), "{}", error.message);
}

#[test]
fn kennel_max_memory_values_exhausts() {
    let config = Config {
        max_memory_values: Some(500),
        ..Config::default()
    };
    let error = run_with_timeout(
        "(define-mutable acc (list))\n(while true (set! acc (cons 1 acc)))",
        config,
        Host::new(),
        Duration::from_secs(5),
    )
    .expect_err("ожидался exhausted");
    assert_eq!(error.code(), "exhausted");
}

#[test]
fn kennel_no_limits_preserve_output() {
    let output = run(
        "(define (fib n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))\n(print (fib 10))",
        Config::default(),
    )
    .expect("без лимитов");
    assert_eq!(output, "55\n");
}

#[test]
fn kennel_host_error_code() {
    let mut host = Host::new();
    host.register_native("fail", Arity::Exact(0), |_args| {
        Err("намеренный отказ".to_string())
    });
    let config = Config {
        profile: Profile::Full,
        ..Config::default()
    };
    let error = kennel::run_program("(host/fail)", &config, &host).expect_err("host-error");
    assert_eq!(error.code(), "host-error");
    assert!(error.message().contains("намеренный отказ"));
}

#[test]
fn kennel_eval_import_senko() {
    let result = run_cli(
        &["eval", "--import", "w,senko", "(senko/sqrt 16.0)"],
        &[],
        Duration::from_secs(5),
    );
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert_eq!(result.stdout, "4.0\n");
}

#[test]
fn kennel_eval_without_import_fails() {
    let result = run_cli(&["eval", "(senko/sqrt 16.0)"], &[], Duration::from_secs(5));
    assert_eq!(result.code, 1);
    assert_eq!(
        extract_code(&result.stderr).as_deref(),
        Some("unknown-identifier")
    );
}

#[test]
fn cli_fuel_env_exhausts_loop() {
    let path = temp_program("(define-mutable i 0)\n(while true (set! i (+ i 1)))\n");
    let result = run_cli(
        &[path.to_str().expect("path")],
        &[("LUPUS_FUEL", "1000")],
        Duration::from_secs(5),
    );
    let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    assert_eq!(result.code, 1, "stderr: {}", result.stderr);
    assert_eq!(extract_code(&result.stderr).as_deref(), Some("exhausted"));
}

#[test]
fn cli_fuel_env_invalid_is_ignored() {
    let path = temp_program("(print 1)\n");
    let result = run_cli(
        &[path.to_str().expect("path")],
        &[("LUPUS_FUEL", "not-a-number")],
        Duration::from_secs(5),
    );
    let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    assert_eq!(result.code, 0, "stderr: {}", result.stderr);
    assert_eq!(result.stdout, "1\n");
}
