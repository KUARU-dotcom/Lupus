# Lupus Alpha v0.1 - Quick Start

## Installation

Make sure you have **Python 3.10 or higher**. No additional packages required.

## First Run

### Option 1: Run a Lupus file

```bash
python lupus_proto.py examples/calc.lupus
```

### Option 2: Interactive REPL

```bash
python lupus_proto.py
```

Expression results are printed automatically:

```
lupus> (define x 10)
lupus> (+ x 5)
15
lupus> (string-append "Hello" ", " "World!")
Hello, World!
lupus> exit
```

## Minimal Example

Create `hello.lupus`:

```lisp
;; Comment
(print "Hello, Lupus!")

(define x 10)
(define y 20)

(define (add a b) (+ a b))

(print (int->str (add x y)))
```

Run:

```bash
python lupus_proto.py hello.lupus
```

Output:
```
Hello, Lupus!
30
```

## Example Programs

The `examples/` folder contains ready-to-run programs:

1. **calc.lupus** — factorial computation
2. **list.lupus** — list operations
3. **fib.lupus** — Fibonacci sequence

Run all at once:

```bash
python run_tests.py
```

## Syntax Overview

### Variables

```lisp
;; Immutable
(define name "Alice")
(define age 30)

;; Mutable
(define-mutable counter 0)
(set! counter 1)
```

### Functions

```lisp
;; Single-line body
(define (add a b) (+ a b))

;; Multi-expression body — all expressions execute, last value returned
(define (factorial n)
  (if (= n 0)
    1
    (* n (factorial (- n 1)))))

;; No-parameter function
(define (greet) (print "Hello!"))
(greet)
```

### Conditionals

```lisp
(if (> x 10)
  (print "big number")
  (print "small number"))
```

### Logic

```lisp
(and (> 5 3) (< 1 2))   ;; true  — short-circuit
(or  (= 1 2) (= 2 2))   ;; true  — short-circuit
(not (= 1 2))            ;; true
```

### Strings

```lisp
(string-append "Hello" ", " "World!")   ;; "Hello, World!"
(int->str 42)                            ;; "42"
```

### Loops

```lisp
(define-mutable i 0)
(while (< i 5)
  (print (int->str i))
  (set! i (+ i 1)))
```

### Sequential execution (begin)

```lisp
(begin
  (print "step 1")
  (print "step 2"))
```

### Lists

```lisp
(define numbers (list 1 2 3 4 5))
(nth numbers 0)    ;; 1
(length numbers)   ;; 5
```

### Arithmetic

```lisp
(+ 1 2 3)   ;; 6
(- 10 3)    ;; 7
(* 4 5)     ;; 20
(/ 20 4)    ;; 5
(% 7 2)     ;; 1  (modulo)
```

## Built-in Functions Reference

| Function | Purpose |
|----------|---------|
| `(print str)` | Print string to stdout |
| `(int->str n)` | Convert integer to string |
| `(string-append ...)` | Concatenate strings |
| `(list ...)` | Create a list |
| `(nth list i)` | Get element by index |
| `(length list)` | Get list length |
| `(if cond then else)` | Conditional |
| `(while cond body...)` | Loop |
| `(begin forms...)` | Sequential execution |
| `(and ...)` | Logical AND (short-circuit) |
| `(or ...)` | Logical OR (short-circuit) |
| `(not x)` | Logical NOT |
| `(% a b)` | Modulo |

## FAQ

### Q: How do I embed variables in strings?

```lisp
(define age 25)
(print (string-append "Age: " (int->str age)))
```

### Q: How do I define a function with no parameters?

```lisp
(define (say-hello) (print "Hello!"))
(say-hello)
```

### Q: How do I check if a number is even?

```lisp
(define (even? n) (= (% n 2) 0))
(if (even? 4) (print "even") (print "odd"))
```

### Q: Can I use floating-point numbers?

No, only integers in this version. Float support is planned for a future release.

---

**Version**: Lupus Alpha v0.1 | **Python**: 3.10+ | **Date**: 2026
