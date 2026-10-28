//! V0.6: builtins как значения первого класса и таблица арности.

use lupus::error::Error;
use lupus::interpreter::Interpreter;
use lupus::value::Value;

fn eval(source: &str) -> Result<Value, Error> {
    Interpreter::new().eval_source(source)
}

fn ok(source: &str) -> Value {
    eval(source).unwrap_or_else(|error| panic!("{source}: {} ({})", error.message, error.code))
}

fn int(value: i64) -> Value {
    Value::Int(value)
}

fn list(values: Vec<Value>) -> Value {
    Value::List(values)
}

#[test]
fn first_class_builtin_head() {
    assert_eq!(
        ok("(define f head) (f (list 1 2))"),
        Value::Some(Box::new(int(1)))
    );
}

#[test]
fn list_map_accepts_abs_builtin() {
    assert_eq!(ok("(list-map (list 1 -2) abs)"), list(vec![int(1), int(2)]));
}

#[test]
fn list_map_composition_via_define() {
    assert_eq!(
        ok("(define g (list-map (list -3 4) abs)) g"),
        list(vec![int(3), int(4)])
    );
}

#[test]
fn arity_mismatch_comes_from_table_with_name() {
    let error = eval("(abs)").expect_err("ожидалась ошибка арности");
    assert_eq!(error.code, "arity-mismatch");
    assert!(error.message.contains("abs"), "{}", error.message);
    assert!(error.message.contains("получено 0"), "{}", error.message);
}

#[test]
fn qualified_names_still_work_and_are_first_class() {
    assert_eq!(
        ok("(import senko) (define root senko/sqrt) (root 81.0)"),
        Value::Float(9.0)
    );
}

#[test]
fn module_native_arity_is_centralized() {
    let error = eval("(import senko) (senko/sqrt 1.0 2.0)").expect_err("ожидалась ошибка арности");
    assert_eq!(error.code, "arity-mismatch");
    assert!(error.message.contains("senko/sqrt"), "{}", error.message);
    assert!(error.message.contains("получено 2"), "{}", error.message);
}

#[test]
fn variadic_builtins_keep_old_behavior() {
    assert_eq!(
        ok("(list (+ 1 2 3) (string-append) (string-append \"a\" \"b\"))"),
        list(vec![
            int(6),
            Value::Str(String::new()),
            Value::Str("ab".into())
        ])
    );
}

#[test]
fn higher_order_builtins_accept_builtins() {
    assert_eq!(
        ok("(list (list-filter (list 0 -1 2) abs) (list-fold (list 1 2 3) 0 +))"),
        list(vec![list(vec![int(0), int(-1), int(2)]), int(6)])
    );
}
