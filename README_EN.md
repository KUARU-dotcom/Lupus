# Lupus Programming Language - Alpha v0.1 Prototype

A minimal working prototype of the **Lupus** programming language, implemented in Python.

## Description

**Lupus** is an experimental programming language based on Lisp syntax (S-expressions).  
This prototype (Alpha v0.1) is a fully functional **tree-walk interpreter** for a core subset of the language.

### Features

- ✅ **Variables** — immutable and mutable bindings
- ✅ **Functions** — definition, calls, recursion, multi-expression bodies
- ✅ **Closures** — lexical scoping with closure support
- ✅ **Operators** — arithmetic, comparison, logical
- ✅ **Strings** — concatenation via `string-append`
- ✅ **Control flow** — conditionals (`if`) and loops (`while`, `begin`)
- ✅ **Data structures** — lists
- ✅ **I/O** — print and type conversion
- ✅ **Two modes** — file execution and interactive REPL

## Quick Start

### Requirements

```bash
# No dependencies — Python 3.10+ only
cd "Lupus/Prototype/Alpha v0.1"
```

### Run

```bash
# Run a file
python lupus_proto.py examples/calc.lupus

# Interactive REPL
python lupus_proto.py
```

### Example program

```lisp
;; hello.lupus
(define (factorial n)
  (if (= n 0)
    1
    (* n (factorial (- n 1)))))

(print (int->str (factorial 5)))  ;; → 120
```

## Documentation

- **[QUICKSTART_EN.md](QUICKSTART_EN.md)** — Quick start and examples
- **[README_ALPHA_EN.md](README_ALPHA_EN.md)** — Full specification and documentation

## Project Structure

```
Lupus/Prototype/Alpha v0.1/
├── lupus_proto.py        # Interpreter (860 lines)
├── run_tests.py          # Test runner
├── README_EN.md          # This file
├── QUICKSTART_EN.md      # Quick start
├── README_ALPHA_EN.md    # Full documentation
└── examples/
    ├── calc.lupus        # Factorial computation
    ├── list.lupus        # List operations
    ├── fib.lupus         # Fibonacci sequence
    └── test_repl.lupus   # Basic operations
```

## Syntax at a Glance

```lisp
;; Variables
(define x 10)                     ;; immutable
(define-mutable y 20)             ;; mutable
(set! y 25)                       ;; assignment

;; Functions
(define (add a b) (+ a b))        ;; single-line
(define (log-add a b)             ;; multi-line body
  (print (string-append "sum of "
    (int->str a) " and " (int->str b)))
  (+ a b))

;; Calls
(add 5 3)                         ;; → 8

;; Conditionals
(if (> x 0) (print "pos") (print "non-pos"))

;; Loops
(while (< i 10) (set! i (+ i 1)))
```

## Built-in Functions

```lisp
;; Arithmetic
(+ 1 2)    (- 5 3)    (* 4 2)    (/ 10 2)    (% 7 2)

;; Comparison
(= a b)    (!= a b)    (< a b)    (> a b)    (<= a b)    (>= a b)

;; Logic
(and expr1 expr2)    (or expr1 expr2)    (not expr)

;; Strings
(string-append "Hello" ", " "World!")    (int->str 42)

;; Lists
(list 1 2 3)    (nth lst 0)    (length lst)

;; I/O
(print "text")
```

## Architecture

```
Source code
    │
    ▼
┌─────────┐     ┌─────────┐     ┌─────────────┐
│  Lexer  │────▶│ Parser  │────▶│ Interpreter │
│ tokens  │     │   AST   │     │  tree-walk  │
└─────────┘     └─────────┘     └─────────────┘
```

## Known Limitations

- **Integers only** — no float support
- **No modules** — single global namespace
- **No exceptions** — errors terminate the program
- **No TCO** — deep recursion hits Python's stack limit

## Roadmap

- Float numbers
- String operations (split, length)
- Hash maps
- Exception handling
- Higher-order functions (map, filter, fold)
- Module system
- Tail call optimization

## Testing

```bash
python run_tests.py                        # run all examples
python lupus_proto.py examples/calc.lupus  # run one example
python lupus_proto.py                      # REPL
```

## Requirements

- Python 3.10+
- Standard library only

---

**Version**: Alpha v0.1 | **Status**: Working prototype | **Date**: 2026
