//! Интеграционные тесты интерпретатора Lupus.
//!
//! Покрывают: арифметику, условия, циклы, функции и рекурсию, списки,
//! строки/вывод, замыкания и обработку ошибок. Часть тестов проверяет
//! значения через библиотечный API, часть — вывод через запуск бинарника.

use lupus::interpreter::Interpreter;
use lupus::value::Value;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Выполняет исходный код и возвращает значение последней формы.
fn run(src: &str) -> Value {
    let mut interp = Interpreter::new();
    interp.eval_source(src).expect("ожидалась успешная оценка")
}

/// Выполняет Lupus-программу через бинарник и возвращает stdout.
fn run_bin(src: &str) -> String {
    let n = FILE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let file = std::env::temp_dir().join(format!("lupus_test_{}_{}.lupus", std::process::id(), n));
    std::fs::write(&file, src).expect("не удалось записать тестовый файл");

    let out = Command::new(env!("CARGO_BIN_EXE_lupus"))
        .arg(&file)
        .output()
        .expect("не удалось запустить интерпретатор");

    let _ = std::fs::remove_file(&file);
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn int(v: i64) -> Value {
    Value::Int(v)
}

// ─── Арифметика ─────────────────────────────────────────────────────────────

#[test]
fn arithmetic_add() {
    assert_eq!(run("(+ 2 3)"), int(5));
    assert_eq!(run("(+ 1 2 3 4)"), int(10));
}

#[test]
fn arithmetic_sub() {
    assert_eq!(run("(- 10 4)"), int(6));
    assert_eq!(run("(- 5)"), int(-5)); // унарный минус
}

#[test]
fn arithmetic_mul() {
    assert_eq!(run("(* 6 7)"), int(42));
    assert_eq!(run("(* 2 3 4)"), int(24));
}

#[test]
fn arithmetic_div() {
    assert_eq!(run("(/ 17 4)"), int(4)); // целочисленное деление
    assert_eq!(run("(/ -7 2)"), int(-3));
}

#[test]
fn arithmetic_rem() {
    assert_eq!(run("(% 17 5)"), int(2));
}

// ─── Условия ────────────────────────────────────────────────────────────────

#[test]
fn condition_if() {
    assert_eq!(run("(if (< 2 3) 10 20)"), int(10));
    assert_eq!(run("(if (> 2 3) 10 20)"), int(20));
    assert_eq!(run("(if (= 2 2) 1 0)"), int(1));
}

#[test]
fn condition_if_number_zero_is_falsy() {
    assert_eq!(run("(if 0 1 2)"), int(2));
    assert_eq!(run("(if 5 1 2)"), int(1));
}

// ─── Логика ─────────────────────────────────────────────────────────────────

#[test]
fn logic_and_or_not() {
    assert_eq!(run("(and (> 5 3) (< 10 20))"), Value::Bool(true));
    assert_eq!(run("(and (> 5 3) (> 10 20))"), Value::Bool(false));
    assert_eq!(run("(or (> 5 10) (< 3 7))"), Value::Bool(true));
    assert_eq!(run("(not (= 5 3))"), Value::Bool(true));
}

// ─── Циклы ──────────────────────────────────────────────────────────────────

#[test]
fn loop_while_sum() {
    // Сумма 1..10 через mutable-аккумулятор.
    let src = "
        (define-mutable s 0)
        (define-mutable i 1)
        (while (<= i 10)
            (set! s (+ s i))
            (set! i (+ i 1)))
        s";
    assert_eq!(run(src), int(55));
}

// ─── Функции и рекурсия ─────────────────────────────────────────────────────

#[test]
fn recursion_factorial() {
    let src = "
        (define (factorial n)
            (if (= n 0) 1 (* n (factorial (- n 1)))))
        (factorial 7)";
    assert_eq!(run(src), int(5040));
}

#[test]
fn recursion_fibonacci() {
    let src = "
        (define (fib n)
            (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))
        (fib 10)";
    assert_eq!(run(src), int(55));
}

#[test]
fn function_multiple_body_forms_uses_begin() {
    let src = "
        (define (sq x)
            (define y 2)
            (* x y))
        (sq 21)";
    assert_eq!(run(src), int(42));
}

// ─── Списки ─────────────────────────────────────────────────────────────────

#[test]
fn lists_list_nth_length() {
    assert_eq!(run("(length (list 1 2 3 4 5))"), int(5));
    assert_eq!(run("(nth (list 10 20 30) 0)"), int(10));
    assert_eq!(run("(nth (list 10 20 30) 2)"), int(30));
}

#[test]
fn lists_recursive_sum() {
    let src = "
        (define (sum-list lst i acc)
            (if (= i (length lst))
                acc
                (sum-list lst (+ i 1) (+ acc (nth lst i)))))
        (sum-list (list 3 1 4 1 5 9) 0 0)";
    assert_eq!(run(src), int(23));
}
#[test]
fn lists_contains() {
    assert_eq!(run("(contains? (list 3 1 4 1 5 9) 7)"), Value::Bool(false));
    assert_eq!(run("(contains? (list 3 1 4 1 5 9) 5)"), Value::Bool(true));
}

#[test]
fn lists_len_alias() {
    assert_eq!(run("(len (list 1 2 3 4 5))"), int(5));
    assert_eq!(run("(len (list))"), int(0));
}

#[test]
fn lenient_parens_ignore_extra_rparen() {
    // Пример регрессии из модели: лишняя `)` после тела цикла.
    let src = "
        (define-mutable i 1)
        (while (<= i 3)
            (print (int->str i))
            (set! i (+ i 1)))
        )";
    // В обычном режиме — ошибка парсера.
    let mut strict = Interpreter::new();
    assert!(strict.eval_source(src).is_err());

    // В lenient-режиме — выполняется с предупреждением в stderr.
    let mut interp = Interpreter::with_lenient_parens(true);
    let value = interp
        .eval_source(src)
        .expect("lenient-режим должен выполниться");
    assert_eq!(value, Value::None);
}

// ─── Строки и вывод ─────────────────────────────────────────────────────────

#[test]
fn strings_int_to_str() {
    assert_eq!(run("(int->str 42)"), Value::Str("42".to_string()));
}

#[test]
fn strings_string_append() {
    assert_eq!(
        run("(string-append \"Hello\" \" \" \"World\")"),
        Value::Str("Hello World".to_string())
    );
}

#[test]
fn strings_print_output() {
    let out = run_bin("(print (string-append \"Result: \" (int->str 99)))");
    assert_eq!(out, "Result: 99\n");
}

#[test]
fn strings_print_multiline_loop() {
    let src = "
        (define-mutable i 1)
        (while (<= i 5)
            (print (int->str i))
            (set! i (+ i 1)))";
    assert_eq!(run_bin(src), "1\n2\n3\n4\n5\n");
}

// ─── Замыкания ──────────────────────────────────────────────────────────────

#[test]
fn closure_counter() {
    let src = "
        (define (make-counter)
            (define-mutable n 0)
            (lambda () (set! n (+ n 1)) n))
        (define c (make-counter))
        (c)
        (c)
        (c)";
    assert_eq!(run(src), int(3));
}

#[test]
fn lambda_inline_call() {
    assert_eq!(run("((lambda (x) (* x x)) 5)"), int(25));
}

// ─── Обработка ошибок ───────────────────────────────────────────────────────

#[test]
fn error_division_by_zero() {
    let mut interp = Interpreter::new();
    let err = interp.eval_source("(/ 10 0)").unwrap_err();
    assert!(err.message.contains("Деление на ноль"));
}

#[test]
fn error_undefined_variable() {
    let mut interp = Interpreter::new();
    let err = interp.eval_source("missing").unwrap_err();
    assert!(err.message.contains("не определена"));
}

#[test]
fn error_undefined_variable_suggests_similar() {
    // Переменная `summ` не определена, но рядом есть `sum` — подсказка.
    let mut interp = Interpreter::new();
    let err = interp
        .eval_source("(define-mutable sum 0) (set! summ 5)")
        .unwrap_err();
    assert!(err.message.contains("Возможно, вы имели в виду: 'sum'"));
}

#[test]
fn error_arity_reports_received_count() {
    let mut interp = Interpreter::new();
    let err = interp.eval_source("(+ 1)").unwrap_err();
    assert!(err.message.contains("получено 1"));
}

#[test]
fn error_nth_out_of_bounds() {
    let mut interp = Interpreter::new();
    let err = interp.eval_source("(nth (list 1 2) 5)").unwrap_err();
    assert!(err.message.contains("Индекс вне диапазона"));
}

#[test]
fn error_set_immutable() {
    let mut interp = Interpreter::new();
    let err = interp.eval_source("(define x 1) (set! x 2)").unwrap_err();
    assert!(err.message.contains("не изменяемая"));
}

#[test]
fn error_binary_prints_error_and_exits_1() {
    let file = std::env::temp_dir().join("lupus_err.lupus");
    std::fs::write(&file, "(/ 1 0)").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_lupus"))
        .arg(&file)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Деление на ноль"));
}

// ─── V0.3: Option/Result (ЭТАП 1) ─────────────────────────────────────────────

/// Вспомогательная функция: выполнить код и вернуть сообщение ошибки.
fn run_err(src: &str) -> String {
    let mut interp = Interpreter::new();
    let err = interp.eval_source(src).unwrap_err();
    err.message
}

#[test]
fn v03_constructors_some_success_failure() {
    assert_eq!(run("(some 5)"), Value::Some(Box::new(int(5))));
    assert_eq!(run("(success 7)"), Value::Success(Box::new(int(7))));
    assert_eq!(
        run("(failure \"msg\")"),
        Value::Failure(Box::new(Value::Str("msg".to_string())))
    );
    assert_eq!(run("none"), Value::None);
}

#[test]
fn v03_some_value_to_string() {
    let mut interp = Interpreter::new();
    interp.eval_source("(define a (some 5))").unwrap();
    let v = interp.eval_source("a").unwrap();
    assert_eq!(lupus::value::value_to_string(&v), "(some 5)");
}

#[test]
fn v03_success_failure_value_to_string() {
    // Вариант B: строки внутри success/failure печатаются в кавычках.
    assert_eq!(
        lupus::value::value_to_string(&run("(success 5)")),
        "(success 5)"
    );
    assert_eq!(
        lupus::value::value_to_string(&run("(success \"ok\")")),
        "(success \"ok\")"
    );
    assert_eq!(
        lupus::value::value_to_string(&run("(failure \"err\")")),
        "(failure \"err\")"
    );
    // Верхнеуровневая строка — без кавычек.
    assert_eq!(lupus::value::value_to_string(&run("\"plain\"")), "plain");
}

#[test]
fn v03_print_number() {
    assert_eq!(run_bin("(print 42)"), "42\n");
}

#[test]
fn v03_print_list() {
    assert_eq!(run_bin("(print (list 1 2 3))"), "(1 2 3)\n");
}

#[test]
fn v03_print_some() {
    assert_eq!(run_bin("(print (some 5))"), "(some 5)\n");
}

#[test]
fn v03_equality_some() {
    assert_eq!(run("(= (some 1) (some 1))"), Value::Bool(true));
    assert_eq!(run("(= (some 1) (some 2))"), Value::Bool(false));
    assert_eq!(run("(= (success 1) (success 1))"), Value::Bool(true));
    assert_eq!(
        run("(= (failure \"a\") (failure \"a\"))"),
        Value::Bool(true)
    );
}

#[test]
fn v03_truthy_none_failure_false() {
    assert_eq!(run("(if none 1 2)"), int(2));
    assert_eq!(run("(if (failure \"x\") 1 2)"), int(2));
    assert_eq!(run("(if (some 0) 1 2)"), int(1)); // Some — истинно
}

// ─── V0.3: Функции списков (ЭТАП 2) ──────────────────────────────────────────

#[test]
fn v03_lists_cons_head_tail_empty() {
    assert_eq!(run("(cons 1 (list 2 3))"), run("(list 1 2 3)"));
    assert_eq!(run("(head (list 1 2))"), Value::Some(Box::new(int(1))));
    assert_eq!(run("(head (list))"), Value::None);
    assert_eq!(
        run("(tail (list 1 2 3))"),
        Value::Some(Box::new(run("(list 2 3)")))
    );
    assert_eq!(run("(tail (list 1))"), Value::None);
    assert_eq!(run("(empty? (list))"), Value::Bool(true));
    assert_eq!(run("(empty? (list 1))"), Value::Bool(false));
}

#[test]
fn v03_lists_append() {
    assert_eq!(run("(append (list 1 2) (list 3 4))"), run("(list 1 2 3 4)"));
    assert_eq!(run("(append (list) (list 1))"), run("(list 1)"));
}

#[test]
fn v03_lists_map_filter_fold() {
    assert_eq!(
        run("(list-map (list 1 2 3) (lambda (x) (* x x)))"),
        run("(list 1 4 9)")
    );
    assert_eq!(
        run("(list-filter (list 1 2 3 4) (lambda (x) (> x 2)))"),
        run("(list 3 4)")
    );
    assert_eq!(
        run("(list-fold (list 1 2 3 4) 0 (lambda (acc x) (+ acc x)))"),
        int(10)
    );
}

#[test]
fn v03_lists_map_not_list_error() {
    let msg = run_err("(list-map 5 (lambda (x) x))");
    assert!(msg.contains("списком"));
}

#[test]
fn v03_nth_out_of_range_is_error() {
    // Осознанное отступление от спеки V1.0: nth по-прежнему кидает ошибку.
    let msg = run_err("(nth (list 1 2) 5)");
    assert!(msg.contains("Индекс вне диапазона"));
}

// ─── V0.3: Функции строк (ЭТАП 2) ────────────────────────────────────────────

#[test]
fn v03_strings_length() {
    // Кириллица и Unicode code points (ЭТАП H: лексер корректно декодирует UTF-8).
    assert_eq!(run("(string-length \"привет\")"), int(6));
    assert_eq!(run("(string-length \"\")"), int(0));
    assert_eq!(run("(string-length \"a b\")"), int(3));
}

#[test]
fn v03_strings_split() {
    assert_eq!(
        run("(string-split \"a,b,c\" \",\")"),
        run("(list \"a\" \"b\" \"c\")")
    );
    assert_eq!(run("(string-split \"abc\" \",\")"), run("(list \"abc\")"));
}

#[test]
fn v03_strings_reverse() {
    assert_eq!(
        run("(string-reverse \"abc\")"),
        Value::Str("cba".to_string())
    );
    assert_eq!(
        run("(string-reverse \"привет\")"),
        Value::Str("тевирп".to_string())
    );
}

#[test]
fn v03_str_to_int_some_and_none() {
    assert_eq!(run("(str->int \"42\")"), Value::Some(Box::new(int(42))));
    assert_eq!(run("(str->int \"abc\")"), Value::None);
}

// ─── V0.3: Арифметика (ЭТАП 2) ───────────────────────────────────────────────

#[test]
fn v03_arithmetic_pow_min_max_abs() {
    assert_eq!(run("(pow 2 10)"), int(1024));
    assert_eq!(run("(pow 5 0)"), int(1));
    assert_eq!(run("(min 3 7)"), int(3));
    assert_eq!(run("(max 3 7)"), int(7));
    assert_eq!(run("(abs -9)"), int(9));
}

#[test]
fn v03_arithmetic_mod_alias() {
    assert_eq!(run("(mod 17 5)"), int(2));
    assert_eq!(run("(% 17 5)"), int(2));
}

#[test]
fn v03_pow_negative_exp_error() {
    let msg = run_err("(pow 2 -1)");
    assert!(msg.contains("отрицательным"));
}

// ─── V0.3: Специальные формы (ЭТАП 3) ────────────────────────────────────────

#[test]
fn v03_cond_with_else() {
    assert_eq!(run("(cond ((< 2 3) 10) (else 20))"), int(10));
    assert_eq!(run("(cond ((> 2 3) 10) (else 20))"), int(20));
}

#[test]
fn v03_cond_without_else_none() {
    assert_eq!(run("(cond ((> 2 3) 10))"), Value::None);
    // Многоформное тело (неявный begin).
    assert_eq!(
        run("(cond ((< 1 2) (define-mutable s 0) (set! s 7) s))"),
        int(7)
    );
}

#[test]
fn v03_for_over_range() {
    let src = "
        (define-mutable total 0)
        (for x in (range 1 6)
            (set! total (+ total x)))
        total";
    assert_eq!(run(src), int(15));
}

#[test]
fn v03_for_not_list_error() {
    let msg = run_err("(for x in 5 (print x))");
    assert!(msg.contains("список"));
}

#[test]
fn v03_range() {
    assert_eq!(run("(range 0 3)"), run("(list 0 1 2)"));
    assert_eq!(run("(range 5 2)"), run("(list)"));
}

#[test]
fn v03_let_parallel_bindings() {
    // Параллельные связи: y вычисляется во внешнем окружении (x ещё не связан).
    let src = "
        (define x 100)
        (let ((x 5) (y x)) y)";
    assert_eq!(run(src), int(100));
}

#[test]
fn v03_let_multiple_bindings_and_body() {
    let src = "
        (let ((a 2) (b 3))
            (define-mutable s 0)
            (set! s (+ a b))
            s)";
    assert_eq!(run(src), int(5));
}

#[test]
fn v03_match_some_variable_binding() {
    let src = "
        (match (some 42)
            (none 0)
            ((some v) v)
            (_ -1))";
    assert_eq!(run(src), int(42));
}

#[test]
fn v03_match_none_and_failure() {
    assert_eq!(run("(match none (none 1) (_ 2))"), int(1));
    assert_eq!(
        run("(match (failure \"e\") ((failure m) m) (_ \"x\"))"),
        Value::Str("e".to_string())
    );
    assert_eq!(run("(match (success 7) ((success n) n) (_ 0))"), int(7));
}

#[test]
fn v03_match_no_match_error() {
    let msg = run_err("(match (some 1) (2 3))");
    assert!(msg.contains("match: ни одна ветка не совпала"));
}

#[test]
fn v03_match_literal_and_wildcard() {
    assert_eq!(
        run("(match 5 (3 \"three\") (5 \"five\") (_ \"other\"))"),
        Value::Str("five".to_string())
    );
}

#[test]
fn v03_if_let_some_binds() {
    assert_eq!(run("(if-let (v (some 10)) (* v 2) 0)"), int(20));
    assert_eq!(run("(if-let (v (some 10)) v 0)"), int(10));
}

#[test]
fn v03_if_let_none_else() {
    assert_eq!(run("(if-let (v none) v 99)"), int(99));
}

#[test]
fn v03_if_let_non_option_error() {
    let msg = run_err("(if-let (v 5) 1 2)");
    assert!(msg.contains("(some v) или none"));
}

#[test]
fn v03_assert_passes_to_none() {
    assert_eq!(run("(assert (= 1 1))"), Value::None);
}

#[test]
fn v03_assert_fail_error() {
    let msg = run_err("(assert (> 2 3))");
    assert!(msg.contains("Assert не выполнен"));
}

// ─── V0.3 ЭТАП H: UTF-8 в строковых литералах (лексер) ───────────────────────

#[test]
fn h_utf8_print_cyrillic() {
    assert_eq!(run_bin("(print \"привет\")"), "привет\n");
}

#[test]
fn h_utf8_string_length() {
    assert_eq!(run("(string-length \"привет\")"), int(6));
}

#[test]
fn h_utf8_string_reverse() {
    assert_eq!(
        run("(string-reverse \"привет\")"),
        Value::Str("тевирп".to_string())
    );
}

#[test]
fn h_utf8_string_append() {
    assert_eq!(
        run("(string-append \"при\" \"вет\")"),
        Value::Str("привет".to_string())
    );
}

#[test]
fn h_utf8_escape_and_cyrillic_length() {
    // \n — один символ (escape), плюс кириллица «пт» (2 code points) = 3.
    assert_eq!(run("(string-length \"п\\nт\")"), int(3));
}

#[test]
fn h_utf8_emoji_surrogate_pair() {
    // 🐺 — один Unicode code point (4 байта UTF-8 / суррогатная пара).
    assert_eq!(run("(string-length \"🐺\")"), int(1));
}
