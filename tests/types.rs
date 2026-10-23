//! Интеграционные тесты Map и дженерик-структур V0.5 (этап C).

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("lupus_types_{}_{}_{}", std::process::id(), tag, n));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run_source(source: &str) -> (String, String, i32) {
    let dir = temp_dir("run");
    let file = dir.join("main.lupus");
    std::fs::write(&file, source).expect("write");
    let output = Command::new(env!("CARGO_BIN_EXE_lupus"))
        .arg(&file)
        .output()
        .expect("run lupus");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

fn run_ok(source: &str) -> String {
    let (stdout, stderr, code) = run_source(source);
    assert_eq!(code, 0, "stderr: {}\nstdout: {}", stderr, stdout);
    stdout
}

fn error_code(stderr: &str) -> Option<String> {
    let marker = "\"code\":\"";
    let start = stderr.find(marker)? + marker.len();
    let rest = &stderr[start..];
    Some(rest[..rest.find('"')?].to_string())
}

#[test]
fn map_constructor_get_keys_values() {
    let out = run_ok(
        "(define m (map (tuple \"a\" 1) (tuple \"b\" 2)))\n\
         (print m)\n\
         (print (map-get m \"a\"))\n\
         (print (map-get m \"missing\"))\n\
         (print (map-keys m))\n\
         (print (map-values m))\n\
         (print (map-has? m \"b\"))",
    );
    assert_eq!(
        out,
        "(map (tuple \"a\" 1) (tuple \"b\" 2))\n(some 1)\nnone\n(\"a\" \"b\")\n(1 2)\ntrue\n"
    );
}

#[test]
fn map_set_is_immutable() {
    let out = run_ok(
        "(define m (map (tuple \"a\" 1)))\n\
         (define m2 (map-set m \"b\" 2))\n\
         (print (map-has? m \"b\"))\n\
         (print (map-has? m2 \"b\"))\n\
         (print (map-get m \"b\"))\n\
         (print (map-get m2 \"b\"))",
    );
    assert_eq!(out, "false\ntrue\nnone\n(some 2)\n");
}

#[test]
fn map_set_replaces_existing_key() {
    let out = run_ok(
        "(define m (map (tuple \"a\" 1)))\n\
         (define m2 (map-set m \"a\" 9))\n\
         (print (map-get m \"a\"))\n\
         (print (map-get m2 \"a\"))",
    );
    assert_eq!(out, "(some 1)\n(some 9)\n");
}

#[test]
fn map_equality_is_pairwise_and_order_independent() {
    let out = run_ok(
        "(print (= (map (tuple \"a\" 1) (tuple \"b\" 2))\n\
                  (map (tuple \"b\" 2) (tuple \"a\" 1))))\n\
         (print (= (map (tuple \"a\" 1)) (map (tuple \"a\" 2))))",
    );
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn generic_struct_tree_with_options() {
    let out = run_ok(
        "(defstruct (Node a)\n\
            (value a)\n\
            (left (Option (Node a)))\n\
            (right (Option (Node a))))\n\
         (define tree (Node 10 (some (Node 5 none none)) none))\n\
         (print tree)\n\
         (print (Node-value tree))\n\
         (print (node-value tree))",
    );
    assert_eq!(out, "(Node 10 (some (Node 5 none none)) none)\n10\n10\n");
}

#[test]
fn generic_struct_match_binds_fields() {
    let out = run_ok(
        "(defstruct (Node a) (value a) (left (Option (Node a))) (right (Option (Node a))))\n\
         (define tree (Node 7 none none))\n\
         (print (match tree\n\
            ((Node v _ _) v)\n\
            (_ 0)))\n\
         (print (match tree\n\
            ((Node v (some left) _) (+ v left))\n\
            ((Node v none _) v)\n\
            (_ 0)))",
    );
    assert_eq!(out, "7\n7\n");
}

#[test]
fn struct_equality_is_by_fields() {
    let out = run_ok(
        "(defstruct (Pair a b) (first a) (second b))\n\
         (print (= (Pair 1 \"x\") (Pair 1 \"x\")))\n\
         (print (= (Pair 1 \"x\") (Pair 2 \"x\")))",
    );
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn map_nested_in_struct_and_match() {
    let out = run_ok(
        "(defstruct box (contents (Map str int)))\n\
         (define b (box (map (tuple \"n\" 5))))\n\
         (print (map-get (box-contents b) \"n\"))",
    );
    assert_eq!(out, "(some 5)\n");
}

#[test]
fn negative_constructor_arity() {
    let (_, stderr, code) = run_source(
        "(defstruct (Node a) (value a) (left (Option (Node a))) (right (Option (Node a))))\n\
         (Node 1)",
    );
    assert_ne!(code, 0);
    assert_eq!(error_code(&stderr).as_deref(), Some("arity-mismatch"));
}

#[test]
fn negative_accessor_foreign_struct() {
    let (_, stderr, code) = run_source(
        "(defstruct point (x int) (y int))\n\
         (defstruct circle (r int))\n\
         (point-x (circle 5))",
    );
    assert_ne!(code, 0);
    assert_eq!(error_code(&stderr).as_deref(), Some("type-mismatch"));
}

#[test]
fn negative_map_nan_key() {
    let (_, stderr, code) = run_source(
        "(define bad (/ 0.0 0.0))\n\
         (map (tuple bad 1))",
    );
    assert_ne!(code, 0);
    assert_eq!(error_code(&stderr).as_deref(), Some("type-mismatch"));
}

#[test]
fn nested_defstruct_in_constructor_argument_does_not_panic() {
    let out = run_ok(
        "(defstruct Outer (x int))\n\
         (print (Outer-x (Outer (begin (defstruct Inner (y int)) 5))))",
    );
    assert_eq!(out, "5\n");
}
