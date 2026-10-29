//! Grammar-based генератор программ Lupus для fuzz-тестирования.
//!
//! Генератор строит только синтаксически корректные программы по БНФ спеки
//! §1.1–1.2 и использует исключительно реально существующий API ядра.
//! Глубина вложенности ограничена: по умолчанию 4, максимум 8.
//!
//! Принципы:
//! - скобки всегда сбалансированы;
//! - имена определяются до использования (в рамках одной сцены);
//! - типы выражений не смешиваются (`int`/`float`, `length` только для списков);
//! - `head`/`tail` используются через форму `(some ...)`/`none`, а не через
//!   конкретное значение;
//! - сетевые (`texas/*`) и асинхронные (`amiya/*`) вызовы не генерируются.
//!
//! В каждой программе выбирается несколько независимых «сцен»: арифметика,
//! списки, Map, Tuple, Option/Result, рекурсия, замыкания, модули, структуры,
//! циклы и локальные определения. Это даёт покрытие разных ветвей семантики.

/// Глубина вложенности по умолчанию.
pub const DEFAULT_DEPTH: usize = 4;
/// Максимально допустимая глубина вложенности.
pub const MAX_DEPTH: usize = 8;

/// Детерминированный xorshift64* — без внешних зависимостей.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next() % bound as u64) as usize
        }
    }

    fn choose<'a, T>(&mut self, values: &'a [T]) -> &'a T {
        &values[self.below(values.len())]
    }
}

const STRINGS: &[&str] = &[
    "\"\"",
    "\"abc\"",
    "\"привет\"",
    "\"🐺\"",
    "\"a b c\"",
    "\"line\\nbreak\"",
    "\"tab\\there\"",
];

/// Сцена генерации. `required_depth` — минимальная глубина, при которой
/// максимальная скобочная вложенность сцены гарантированно помещается в лимит.
#[derive(Clone, Copy)]
enum Scenario {
    SimplePrint,
    Directive,
    Arithmetic,
    Floats,
    Strings,
    Booleans,
    Cond,
    Lists,
    Maps,
    TupleMatch,
    OptionMatch,
    Recursion,
    MutualRecursion,
    Closure,
    ClosureCounter,
    LoopCapture,
    Modules,
    ModuleAlias,
    Struct,
    StructMatch,
    Results,
    Loops,
    LetBinding,
    LocalDefine,
    PublicFunction,
    ConstBinding,
    TestForm,
}

impl Scenario {
    const ALL: &'static [Scenario] = &[
        Scenario::SimplePrint,
        Scenario::Directive,
        Scenario::Arithmetic,
        Scenario::Floats,
        Scenario::Strings,
        Scenario::Booleans,
        Scenario::Cond,
        Scenario::Lists,
        Scenario::Maps,
        Scenario::TupleMatch,
        Scenario::OptionMatch,
        Scenario::Recursion,
        Scenario::MutualRecursion,
        Scenario::Closure,
        Scenario::ClosureCounter,
        Scenario::LoopCapture,
        Scenario::Modules,
        Scenario::ModuleAlias,
        Scenario::Struct,
        Scenario::StructMatch,
        Scenario::Results,
        Scenario::Loops,
        Scenario::LetBinding,
        Scenario::LocalDefine,
        Scenario::PublicFunction,
        Scenario::ConstBinding,
        Scenario::TestForm,
    ];

    fn required_depth(self) -> usize {
        match self {
            Scenario::SimplePrint | Scenario::Directive => 1,
            Scenario::Floats
            | Scenario::Strings
            | Scenario::Modules
            | Scenario::ModuleAlias
            | Scenario::Struct
            | Scenario::ConstBinding => 2,
            Scenario::Arithmetic
            | Scenario::PublicFunction
            | Scenario::Booleans
            | Scenario::Lists
            | Scenario::Maps
            | Scenario::TupleMatch
            | Scenario::Closure
            | Scenario::ClosureCounter
            | Scenario::Results
            | Scenario::Loops
            | Scenario::LocalDefine => 3,
            Scenario::Cond
            | Scenario::OptionMatch
            | Scenario::Recursion
            | Scenario::MutualRecursion
            | Scenario::LoopCapture
            | Scenario::StructMatch
            | Scenario::LetBinding
            | Scenario::TestForm => 4,
        }
    }
}

/// Генератор программ с контролируемой глубиной вложенности.
pub struct Generator {
    rng: Rng,
    max_depth: usize,
    next_id: u64,
}

impl Generator {
    /// Создаёт генератор с глубиной по умолчанию.
    pub fn new(seed: u64) -> Self {
        Self::with_depth(seed, DEFAULT_DEPTH)
    }

    /// Создаёт генератор с заданной глубиной (зажимается в `1..=MAX_DEPTH`).
    pub fn with_depth(seed: u64, max_depth: usize) -> Self {
        Generator {
            rng: Rng::new(seed),
            max_depth: max_depth.clamp(1, MAX_DEPTH),
            next_id: 0,
        }
    }

    /// Текущий лимит глубины.
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Генерирует одну программу.
    pub fn generate(&mut self) -> String {
        let mut candidates: Vec<Scenario> = Scenario::ALL
            .iter()
            .copied()
            .filter(|scenario| scenario.required_depth() <= self.max_depth)
            .collect();
        if candidates.is_empty() {
            candidates.push(Scenario::SimplePrint);
        }

        let upper = 4.min(candidates.len());
        let count = 1 + self.rng.below(upper);
        let mut chosen = Vec::with_capacity(count);
        for _ in 0..count {
            let index = self.rng.below(candidates.len());
            chosen.push(candidates.swap_remove(index));
        }

        let mut forms = Vec::new();
        for scenario in chosen {
            forms.extend(self.emit(scenario));
        }
        let source = format!("{}\n", forms.join("\n"));
        let actual_depth = nesting_depth(&source);
        debug_assert!(
            actual_depth.is_some_and(|depth| depth <= self.max_depth),
            "сгенерирована программа с глубиной {actual_depth:?} > {}: {source}",
            self.max_depth
        );
        source
    }

    fn next_name(&mut self, base: &str) -> String {
        self.next_id += 1;
        format!("{base}{}", self.next_id)
    }

    fn int_lit(&mut self) -> String {
        self.rng.below(20).to_string()
    }

    fn small_int(&mut self) -> String {
        self.rng.below(10).to_string()
    }

    fn nonzero_int(&mut self) -> String {
        (1 + self.rng.below(9)).to_string()
    }

    fn float_lit(&mut self) -> String {
        format!("{}.{}", self.rng.below(10), self.rng.below(10))
    }

    fn nonzero_float(&mut self) -> String {
        format!("{}.{}", 1 + self.rng.below(9), self.rng.below(10))
    }

    fn string_lit(&mut self) -> &'static str {
        self.rng.choose(STRINGS)
    }

    fn bool_lit(&mut self) -> &'static str {
        self.rng.choose(&["true", "false"])
    }

    fn emit(&mut self, scenario: Scenario) -> Vec<String> {
        match scenario {
            Scenario::SimplePrint => self.scenario_simple_print(),
            Scenario::Directive => vec!["(#lupus enable-check types)".to_string()],
            Scenario::Arithmetic => self.scenario_arithmetic(),
            Scenario::Floats => self.scenario_floats(),
            Scenario::Strings => self.scenario_strings(),
            Scenario::Booleans => self.scenario_booleans(),
            Scenario::Cond => self.scenario_cond(),
            Scenario::Lists => self.scenario_lists(),
            Scenario::Maps => self.scenario_maps(),
            Scenario::TupleMatch => self.scenario_tuple_match(),
            Scenario::OptionMatch => self.scenario_option_match(),
            Scenario::Recursion => self.scenario_recursion(),
            Scenario::MutualRecursion => self.scenario_mutual_recursion(),
            Scenario::Closure => self.scenario_closure(),
            Scenario::ClosureCounter => self.scenario_closure_counter(),
            Scenario::LoopCapture => self.scenario_loop_capture(),
            Scenario::Modules => self.scenario_modules(),
            Scenario::ModuleAlias => self.scenario_module_alias(),
            Scenario::Struct => self.scenario_struct(),
            Scenario::StructMatch => self.scenario_struct_match(),
            Scenario::Results => self.scenario_results(),
            Scenario::Loops => self.scenario_loops(),
            Scenario::LetBinding => self.scenario_let_binding(),
            Scenario::LocalDefine => self.scenario_local_define(),
            Scenario::PublicFunction => self.scenario_public_function(),
            Scenario::ConstBinding => self.scenario_const_binding(),
            Scenario::TestForm => self.scenario_test_form(),
        }
    }

    fn scenario_simple_print(&mut self) -> Vec<String> {
        match self.rng.below(4) {
            0 => vec![format!("(print {})", self.int_lit())],
            1 => vec![format!("(print {})", self.float_lit())],
            2 => vec![format!("(print {})", self.string_lit())],
            _ => vec![format!("(print {})", self.bool_lit())],
        }
    }

    fn scenario_arithmetic(&mut self) -> Vec<String> {
        let a = self.int_lit();
        let b = self.int_lit();
        let divisor = self.nonzero_int();
        let base = self.small_int();
        let exponent = self.rng.below(4).to_string();
        vec![
            format!("(print (+ {a} {b}))"),
            format!("(print (- {a} {b}))"),
            format!("(print (* {a} {b}))"),
            format!("(print (/ {a} {divisor}))"),
            format!("(print (% {a} {divisor}))"),
            format!("(print (pow {base} {exponent}))"),
            format!("(print (min {a} {b}))"),
            format!("(print (max {a} {b}))"),
            format!("(print (abs (- 0 {a})))"),
        ]
    }

    fn scenario_floats(&mut self) -> Vec<String> {
        let a = self.float_lit();
        let b = self.nonzero_float();
        let integer = self.int_lit();
        vec![
            format!("(print (+ {a} {b}))"),
            format!("(print (- {a} {b}))"),
            format!("(print (* {a} {b}))"),
            format!("(print (/ {a} {b}))"),
            format!("(print (int->float {integer}))"),
            format!("(print (float->int {a}))"),
        ]
    }

    fn scenario_strings(&mut self) -> Vec<String> {
        let a = self.string_lit();
        let b = self.string_lit();
        let integer = self.int_lit();
        let float = self.float_lit();
        vec![
            format!("(print (string-append {a} {b}))"),
            format!("(print (string-reverse {a}))"),
            format!("(print (string-length {a}))"),
            format!("(print (int->str {integer}))"),
            format!("(print (float->str {float}))"),
            format!("(print (string-split {a} \" \"))"),
        ]
    }

    fn scenario_booleans(&mut self) -> Vec<String> {
        let a = self.int_lit();
        let b = self.int_lit();
        vec![
            format!("(print (if (< {a} {b}) {a} {b}))"),
            format!("(print (and (< {a} {b}) (> {b} 0)))"),
            format!("(print (or (= {a} {b}) (= {a} {a})))"),
            format!("(print (not (= {a} {b})))"),
        ]
    }

    fn scenario_cond(&mut self) -> Vec<String> {
        let a = self.int_lit();
        let b = self.int_lit();
        let text = self.string_lit();
        vec![
            format!("(print (cond ((< {a} {b}) {a}) (else {b})))"),
            format!(
                "(print (cond ((= {a} {b}) (string-append {text} \"!\")) (else (int->str {a}))))"
            ),
        ]
    }

    fn scenario_lists(&mut self) -> Vec<String> {
        let xs = self.next_name("xs");
        let a = self.small_int();
        let b = self.small_int();
        let c = self.small_int();
        let extra = self.small_int();
        let index = self.rng.below(3);
        vec![
            format!("(define {xs} (list {a} {b} {c}))"),
            format!("(print {xs})"),
            format!("(print (length {xs}))"),
            format!("(print (head {xs}))"),
            format!("(print (tail {xs}))"),
            format!("(print (nth {xs} {index}))"),
            format!("(print (cons {extra} {xs}))"),
            format!("(print (append {xs} (list {extra})))"),
            format!("(print (contains? {xs} {b}))"),
            format!("(print (empty? {xs}))"),
            "(print (empty? (list)))".to_string(),
            format!("(print (tail (list {a})))"),
        ]
    }

    fn scenario_maps(&mut self) -> Vec<String> {
        let table = self.next_name("m");
        let a = self.small_int();
        let b = self.small_int();
        let c = self.small_int();
        vec![
            format!("(define {table} (map (tuple \"k\" {a}) (tuple \"j\" {b})))"),
            format!("(print {table})"),
            format!("(print (map-get {table} \"k\"))"),
            format!("(print (map-has? {table} \"j\"))"),
            format!("(print (map-keys {table}))"),
            format!("(print (map-values {table}))"),
            format!("(print (map-set {table} \"z\" {c}))"),
        ]
    }

    fn scenario_tuple_match(&mut self) -> Vec<String> {
        let value = self.next_name("t");
        let a = self.small_int();
        let text = self.string_lit();
        let flag = self.bool_lit();
        vec![
            format!("(define {value} (tuple {a} {text} {flag}))"),
            format!("(print {value})"),
            format!("(match {value} ((tuple x y z) (print x)))"),
        ]
    }

    fn scenario_option_match(&mut self) -> Vec<String> {
        let xs = self.next_name("xs");
        let a = self.small_int();
        let b = self.small_int();
        let c = self.small_int();
        vec![
            format!("(define {xs} (list {a} {b} {c}))"),
            format!("(match (head {xs}) ((some v) (print v)) (none (print 0)))"),
            format!("(if-let (v (head {xs})) (print v) (print 0))"),
            format!("(match (tail {xs}) ((some rest) (print (length rest))) (none (print 0)))"),
            format!(
                "(match (tail (list {a})) ((some rest) (print (length rest))) (none (print 0)))"
            ),
        ]
    }

    fn scenario_recursion(&mut self) -> Vec<String> {
        if self.rng.below(2) == 0 {
            let function = self.next_name("count");
            let n = (3 + self.rng.below(5)).to_string();
            vec![
                format!("(define ({function} n) (if (= n 0) 0 ({function} (- n 1))))"),
                format!("(print ({function} {n}))"),
            ]
        } else {
            let function = self.next_name("sum");
            let n = (3 + self.rng.below(5)).to_string();
            vec![
                format!(
                    "(define ({function} n acc) (if (= n 0) acc ({function} (- n 1) (+ acc n))))"
                ),
                format!("(print ({function} {n} 0))"),
            ]
        }
    }

    fn scenario_mutual_recursion(&mut self) -> Vec<String> {
        let even = self.next_name("even");
        let odd = self.next_name("odd");
        let n = (2 * self.rng.below(4)).to_string();
        vec![
            format!("(define ({even} n) (if (= n 0) true ({odd} (- n 1))))"),
            format!("(define ({odd} n) (if (= n 0) false ({even} (- n 1))))"),
            format!("(print ({even} {n}))"),
        ]
    }

    fn scenario_closure(&mut self) -> Vec<String> {
        let make = self.next_name("make-adder");
        let add = self.next_name("add");
        let a = self.small_int();
        let b = self.small_int();
        vec![
            format!("(define ({make} x) (lambda (y) (+ x y)))"),
            format!("(define {add} ({make} {a}))"),
            format!("(print ({add} {b}))"),
        ]
    }

    fn scenario_closure_counter(&mut self) -> Vec<String> {
        let counter = self.next_name("counter");
        let increment = self.next_name("inc");
        vec![
            format!("(define-mutable {counter} 0)"),
            format!("(define ({increment}) (set! {counter} (+ {counter} 1)) {counter})"),
            format!("(print ({increment}))"),
            format!("(print ({increment}))"),
        ]
    }

    fn scenario_loop_capture(&mut self) -> Vec<String> {
        let make = self.next_name("make");
        let closures = self.next_name("fs");
        let item = self.next_name("x");
        vec![
            format!("(define ({make} x) (lambda () x))"),
            format!("(define-mutable {closures} (list))"),
            format!(
                "(for {item} in (range 0 3) (set! {closures} (cons ({make} {item}) {closures})))"
            ),
            format!("(print ((nth {closures} 0)))"),
            format!("(print ((nth {closures} 2)))"),
        ]
    }

    fn scenario_modules(&mut self) -> Vec<String> {
        let value = self.rng.choose(&["0.0", "1.0", "4.0", "9.0", "16.0"]);
        vec![
            "(import senko)".to_string(),
            format!("(print (senko/sqrt {value}))"),
            "(print (senko/pow 2.0 3.0))".to_string(),
            "(print (senko/sin 0.0))".to_string(),
            "(print (senko/cos 0.0))".to_string(),
            "(print (senko/log 1.0))".to_string(),
            "(print (senko/floor 3.7))".to_string(),
            "(print senko/pi)".to_string(),
        ]
    }

    fn scenario_module_alias(&mut self) -> Vec<String> {
        vec![
            "(import senko :as s)".to_string(),
            "(print (s/sqrt 9.0))".to_string(),
            "(print (s/min 1.0 2.0))".to_string(),
            "(print s/pi)".to_string(),
        ]
    }

    fn scenario_struct(&mut self) -> Vec<String> {
        let type_name = self.next_name("Point");
        let value = self.next_name("p");
        let a = self.small_int();
        let b = self.small_int();
        let text = self.string_lit();
        if self.rng.below(2) == 0 {
            vec![
                format!("(defstruct {type_name} (x int) (y int))"),
                format!("(define {value} ({type_name} {a} {b}))"),
                format!("(print ({type_name}-x {value}))"),
                format!("(print ({type_name}-y {value}))"),
                format!("(print {value})"),
            ]
        } else {
            vec![
                format!("(defstruct {type_name} (name str) (age int))"),
                format!("(define {value} ({type_name} {text} {a}))"),
                format!("(print ({type_name}-name {value}))"),
                format!("(print ({type_name}-age {value}))"),
                format!("(print {value})"),
            ]
        }
    }

    fn scenario_struct_match(&mut self) -> Vec<String> {
        let type_name = self.next_name("Point");
        let value = self.next_name("p");
        let a = self.small_int();
        let b = self.small_int();
        vec![
            format!("(defstruct {type_name} (x int) (y int))"),
            format!("(define {value} ({type_name} {a} {b}))"),
            format!("(match {value} (({type_name} x y) (print (+ x y))))"),
        ]
    }

    fn scenario_results(&mut self) -> Vec<String> {
        let a = self.small_int();
        let text = self.string_lit();
        vec![
            format!("(print (some {a}))"),
            format!("(print (success {a}))"),
            format!("(print (failure {text}))"),
            format!("(match (some {a}) ((some v) (print v)) (none (print 0)))"),
            format!("(match (success {a}) ((success v) (print v)) ((failure e) (print 0)))"),
        ]
    }

    fn scenario_loops(&mut self) -> Vec<String> {
        let counter = self.next_name("i");
        let sum = self.next_name("sum");
        let item = self.next_name("x");
        vec![
            format!("(define-mutable {counter} 0)"),
            format!("(while (< {counter} 3) (print {counter}) (set! {counter} (+ {counter} 1)))"),
            format!("(define-mutable {sum} 0)"),
            format!("(for {item} in (range 0 4) (set! {sum} (+ {sum} {item})))"),
            format!("(print {sum})"),
        ]
    }

    fn scenario_let_binding(&mut self) -> Vec<String> {
        let x = self.next_name("x");
        let y = self.next_name("y");
        if self.rng.below(2) == 0 {
            let a = self.small_int();
            let b = self.small_int();
            vec![format!("(print (let (({x} {a}) ({y} {b})) (+ {x} {y})))")]
        } else {
            let a = self.string_lit();
            let b = self.string_lit();
            vec![format!(
                "(print (let (({x} {a}) ({y} {b})) (string-append {x} {y})))"
            )]
        }
    }

    fn scenario_local_define(&mut self) -> Vec<String> {
        let function = self.next_name("local");
        let a = self.small_int();
        vec![
            format!("(define ({function} x) (define y (+ x 1)) (+ x y))"),
            format!("(print ({function} {a}))"),
        ]
    }

    fn scenario_public_function(&mut self) -> Vec<String> {
        let function = self.next_name("pub");
        let a = self.small_int();
        vec![
            format!("(define-public ({function} (x int)) -> int (+ x 1))"),
            format!("(print ({function} {a}))"),
        ]
    }

    fn scenario_const_binding(&mut self) -> Vec<String> {
        let constant = self.next_name("c");
        let a = self.small_int();
        vec![
            format!("(define-const {constant} 42)"),
            format!("(print (+ {constant} {a}))"),
        ]
    }

    fn scenario_test_form(&mut self) -> Vec<String> {
        let name = self.next_name("gen");
        let a = self.small_int();
        let b = self.small_int();
        vec![format!(
            "(test \"{name}\" (assert (= (+ {a} {b}) (+ {b} {a}))))"
        )]
    }
}

/// Максимальная скобочная вложенность программы.
///
/// Возвращает `None`, если скобки не сбалансированы. Строки, строчные (`;`)
/// и блочные (`#| ... |#`) комментарии не учитываются.
pub fn nesting_depth(source: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut maximum = 0usize;
    let mut index = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;

    while index < bytes.len() {
        let byte = bytes[index];

        if line_comment {
            if byte == b'\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }

        if block_comment {
            if byte == b'|' && bytes.get(index + 1) == Some(&b'#') {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b';' => line_comment = true,
            b'#' if bytes.get(index + 1) == Some(&b'|') => {
                block_comment = true;
                index += 2;
                continue;
            }
            b'(' => {
                depth += 1;
                maximum = maximum.max(depth);
            }
            b')' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            _ => {}
        }

        index += 1;
    }

    if in_string || block_comment || depth != 0 {
        None
    } else {
        Some(maximum)
    }
}

/// Проверяет, сбалансированы ли скобки в программе.
pub fn is_balanced(source: &str) -> bool {
    nesting_depth(source).is_some()
}

/// Максимальная скобочная вложенность или `usize::MAX` при дисбалансе.
pub fn max_nesting_depth(source: &str) -> usize {
    nesting_depth(source).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_programs_are_balanced_and_within_depth() {
        for depth in 1..=MAX_DEPTH {
            let mut generator = Generator::with_depth(0x5EED, depth);
            for _ in 0..200 {
                let source = generator.generate();
                assert!(is_balanced(&source), "несбалансированные скобки:\n{source}");
                assert!(
                    max_nesting_depth(&source) <= depth,
                    "глубина {} превышает лимит {depth}:\n{source}",
                    max_nesting_depth(&source)
                );
            }
        }
    }

    #[test]
    fn generated_corpus_covers_semantic_branches() {
        let mut generator = Generator::new(0xC0FF_EE00_u64);
        let mut corpus = String::new();
        for _ in 0..500 {
            corpus.push_str(&generator.generate());
        }

        for feature in [
            "define (",
            "define-mutable",
            "lambda",
            "import senko",
            "defstruct",
            "(map ",
            "(tuple ",
            "(list ",
            "match",
            "if-let",
            "for ",
            "while ",
            "cond",
            "string-reverse",
            "some",
            "success",
        ] {
            assert!(
                corpus.contains(feature),
                "в сгенерированном корпусе нет ветки семантики: {feature}"
            );
        }
    }

    #[test]
    fn nesting_depth_ignores_strings_and_comments() {
        assert_eq!(nesting_depth("(print \"(((\")"), Some(1));
        assert_eq!(nesting_depth("(print 1) ; )))\n"), Some(1));
        assert_eq!(nesting_depth("(print 1) #| ))) |#"), Some(1));
        assert_eq!(nesting_depth(")("), None);
        assert_eq!(nesting_depth("(print 1"), None);
    }
}
