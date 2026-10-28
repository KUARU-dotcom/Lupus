//! Регрессия #45: кэш файловых модулей не должен удерживать окружение модуля.
//!
//! Раньше `define-public` складывал в экспорты `Value::Closure` с сильной
//! ссылкой на `module_env`, а `Environment::define` складывал то же замыкание
//! в сам `module_env`. Возникал `Rc`-цикл
//! `module_env → bindings → Closure.env → module_env`, поэтому после выхода
//! из интерпретатора окружение модуля не освобождалось.
//!
//! Тест проверяет именно цикл (а не RSS): после `drop(interpreter)` слабая
//! ссылка на окружение экспортированного замыкания обязана перестать
//! «подниматься». На коде без фикса проверка краснеет.

use lupus::value::Value;
use lupus::Interpreter;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("lupus_memory_{}_{}_{}", std::process::id(), tag, n));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn module_function_does_not_leak_module_environment() {
    let dir = temp_dir("module");
    std::fs::write(
        dir.join("fnmod.lupus"),
        "(define-public inc (lambda (x) (+ x 1)))",
    )
    .expect("write module");

    let mut interpreter = Interpreter::new();
    interpreter.set_program_dir(dir.clone());
    let value = interpreter
        .eval_source("(import fnmod) fnmod/inc")
        .expect("module program runs");

    let Value::Closure(closure) = value else {
        panic!("экспорт модуля должен быть замыканием");
    };
    let module_env = Rc::downgrade(&closure.env);
    drop(closure);
    drop(interpreter);

    assert!(
        module_env.upgrade().is_none(),
        "окружение модуля осталось живым: Rc-цикл env → closure → env не разорван"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Регрессия #47: цикл `env → список → замыкание → env` должен разрываться
/// сборщиком после `drop(interpreter)`.
///
/// Локальное окружение `make-cycles` хранит список из 20 000 замыканий, а
/// каждое замыкание захватывает это же окружение. После уничтожения
/// интерпретатора без GC остаётся большой связный цикл; `list-map` строит
/// список за один проход, поэтому тест не квадратичный.
#[test]
fn memory_leak_containers() {
    let mut interpreter = Interpreter::new();
    let value = interpreter
        .eval_source(
            "(define (make-cycles n)
               (define-mutable local (list))
               (set! local (list-map (range 0 n) (lambda (i) (lambda () local))))
               local)
             (define c (make-cycles 20000))
             c",
        )
        .expect("цикл с контейнерами выполняется");

    let Value::List(items) = &value else {
        panic!("ожидался список замыканий");
    };
    let Value::Closure(closure) = &items[0] else {
        panic!("элемент списка должен быть замыканием");
    };
    let env = Rc::downgrade(&closure.env);
    let pause = interpreter.gc_collect();
    // В debug-сборке накладные расходы трассировки заметно выше; требование
    // «пауза < 10 мс» проверяется в release, где оно и актуально.
    if !cfg!(debug_assertions) {
        assert!(
            pause.duration_micros < 10_000,
            "пауза GC больше 10 мс: {} us",
            pause.duration_micros
        );
    }
    drop(value);
    drop(interpreter);

    assert!(
        env.upgrade().is_none(),
        "окружение с 20 000 замыканий осталось живым: цикл через контейнер не разорван"
    );
}

/// Проверяет, что `(gc-collect)` очищает недостижимый локальный цикл, пока
/// интерпретатор ещё жив.
#[test]
fn gc_cycle_breaking() {
    let mut interpreter = Interpreter::new();
    let value = interpreter
        .eval_source(
            "(define (make-cycle)
               (define-mutable box (list))
               (set! box (cons (lambda () box) box))
               box)
             (define-mutable c (make-cycle))
             c",
        )
        .expect("локальный цикл создан");

    let Value::List(items) = &value else {
        panic!("ожидался список");
    };
    let Value::Closure(closure) = &items[0] else {
        panic!("элемент списка должен быть замыканием");
    };
    let local_env = Rc::downgrade(&closure.env);
    drop(value);

    // Убираем последнюю внешнюю ссылку (глобальную c) и запускаем GC вручную.
    interpreter
        .eval_source("(set! c none) (gc-collect)")
        .expect("gc-collect выполняется");

    assert!(
        local_env.upgrade().is_none(),
        "gc-collect не разорвал цикл env → list → closure → env"
    );
}

/// Автоматический GC по порогу печатает `GC: collected N objects` в stderr.
#[test]
fn gc_logging() {
    let dir = temp_dir("gc_log");
    let program = dir.join("gc.lupus");
    std::fs::write(
        &program,
        "(define (make-cycle)
           (define-mutable box (list))
           (set! box (cons (lambda () box) box))
           box)
         (define-mutable i 0)
         (while (< i 1200)
           (make-cycle)
           (set! i (+ i 1)))
         (print \"ok\")",
    )
    .expect("write program");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_lupus"))
        .arg(&program)
        .output()
        .expect("run lupus");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stderr.contains("GC: collected") && stderr.contains("objects"),
        "нет строки логирования GC: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
