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
