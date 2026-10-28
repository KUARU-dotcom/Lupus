//! Прогон файлов из `examples/` (V0.6 этап E).
//!
//! `LUPUS_EXAMPLE_PORT` подставляет порт в `examples/v06_net.lupus`; без
//! переменной выбирается свободный локальный порт.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn run_file(path: &Path, timeout: Duration) -> (i32, String, String, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lupus"))
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn lupus");
    let started = Instant::now();
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
    (
        status.and_then(|status| status.code()).unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        status.is_none(),
    )
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

#[test]
fn async_example_runs() {
    let (code, stdout, stderr, timed_out) =
        run_file(&example("v06_async.lupus"), Duration::from_secs(10));
    assert!(!timed_out, "stderr: {}", stderr);
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert_eq!(stdout, "1\ntwo\n(3 4)\n");
}

#[test]
fn net_example_runs() {
    let port = std::env::var("LUPUS_EXAMPLE_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or_else(|| {
            TcpListener::bind("127.0.0.1:0")
                .expect("free port")
                .local_addr()
                .expect("addr")
                .port()
        });

    let source = std::fs::read_to_string(example("v06_net.lupus")).expect("read example");
    let source = source.replace("47711", &port.to_string());
    let path = std::env::temp_dir().join(format!("lupus_v06_net_{}.lupus", std::process::id()));
    std::fs::write(&path, source).expect("write example");

    let (code, stdout, stderr, timed_out) = run_file(&path, Duration::from_secs(10));
    let _ = std::fs::remove_file(&path);
    assert!(!timed_out, "stderr: {}", stderr);
    assert_eq!(code, 0, "stderr: {}", stderr);
    assert_eq!(stdout, "4\nping\n");
}
