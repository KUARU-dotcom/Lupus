//! Перфоманс-гейт V0.6: автоматический GC не должен давать регресс >5%.
//!
//! Сравнивается один и тот же `fib(25)` в двух интерпретаторах: с отключённым
//! автоматическим сборщиком (базовая линия) и с включённым (порог 1000
//! аллокаций). Ручной `gc-collect` и `Drop` в замер не входят.

use lupus::gc;
use lupus::value::Value;
use lupus::Interpreter;
use std::time::{Duration, Instant};

const FIB: &str = "(define (fib n)
  (if (< n 2)
      n
      (+ (fib (- n 1)) (fib (- n 2)))))
(fib 25)";

fn run_fib(auto_gc: bool) -> Duration {
    gc::reset_for_tests();
    gc::set_enabled(auto_gc);
    let mut interpreter = Interpreter::new();
    let start = Instant::now();
    let value = interpreter.eval_source(FIB).expect("fib(25)");
    let elapsed = start.elapsed();
    assert_eq!(value, Value::Int(75025));
    elapsed
}

#[test]
fn gc_performance() {
    // Два замера каждого режима и минимум: так тест устойчив к фоновой
    // нагрузке CI, но по-прежнему проверяет регресс на одном и том же коде.
    let baseline_time = run_fib(false).min(run_fib(false));
    let gc_time = run_fib(true).min(run_fib(true));

    let limit = baseline_time.mul_f64(1.05);
    assert!(
        gc_time <= limit,
        "регресс GC больше 5%: без GC {baseline_time:?}, с GC {gc_time:?}"
    );
    eprintln!("GC perf: без GC {baseline_time:?}, с GC {gc_time:?}");
}
