//! Гейт #47: после 20 000 циклов `env → list → closure → env` живая
//! память должна освобождаться сборщиком (утечка < 1 МБ).
//!
//! В отличие от `tests/memory.rs`, здесь считается не RSS (аллокатор может
//! держать арены), а реально занятые байты через обёртку над системным
//! аллокатором. Именно это позволяет проверить требование «< 1 МБ».

use lupus::value::Value;
use lupus::Interpreter;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountingAllocator;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE_BYTES.fetch_add(layout.size(), Ordering::SeqCst);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::SeqCst);
        System.dealloc(ptr, layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LIVE_BYTES.fetch_add(layout.size(), Ordering::SeqCst);
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let result = System.realloc(ptr, layout, new_size);
        if !result.is_null() {
            LIVE_BYTES.fetch_add(new_size, Ordering::SeqCst);
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::SeqCst);
        }
        result
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const PROGRAM: &str = "(define (make-cycles n)
  (define-mutable local (list))
  (set! local (list-map (range 0 n) (lambda (i) (lambda () local))))
  local)
(define c (make-cycles 20000))
c";

#[test]
fn gc_leak_under_1mb() {
    let before = LIVE_BYTES.load(Ordering::SeqCst);
    {
        let mut interpreter = Interpreter::new();
        let value = interpreter.eval_source(PROGRAM).expect("20 000 циклов");
        let Value::List(items) = &value else {
            panic!("ожидался список");
        };
        let Value::Closure(closure) = &items[0] else {
            panic!("элемент списка должен быть замыканием");
        };
        let cycle_env = std::rc::Rc::downgrade(&closure.env);
        drop(value);
        drop(interpreter);

        assert!(
            cycle_env.upgrade().is_none(),
            "окружение цикла осталось живым"
        );
    }

    let after = LIVE_BYTES.load(Ordering::SeqCst);
    let delta = after.saturating_sub(before);
    assert!(
        delta < 1024 * 1024,
        "утечка {} байт (порог 1 МБ): before={before}, after={after}",
        delta as i64
    );
}
