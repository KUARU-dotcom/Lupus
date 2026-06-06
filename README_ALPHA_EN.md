# Lupus Alpha v0.1 - Documentation

## Overview

**Lupus** is an experimental programming language based on Lisp syntax (S-expressions).  
**Lupus Alpha v0.1** is a minimal working prototype supporting core operations.

The implementation includes:
- **Lexer** — tokenizes source code
- **Parser** — builds an Abstract Syntax Tree (AST)
- **Interpreter** — tree-walk AST execution with lexical scoping

## Installation & Running

### Requirements
- Python 3.10 or higher
- Standard library only — no external dependencies

### Run a Lupus file

```bash
python lupus_proto.py filename.lupus
```

### Interactive REPL

```bash
python lupus_proto.py
```

Expression results are printed automatically:

```
lupus> (define x 10)
lupus> (+ x 5)
15
lupus> (string-append "x = " (int->str x))
x = 10
```

## Syntax Reference

### 1. Comments

```lisp
;; This is a comment
```

### 2. Variables

```lisp
;; Immutable (define)
(define x 10)
(define name "Lupus")

;; Mutable (define-mutable + set!)
(define-mutable y 20)
(set! y 25)
```

### 3. Functions

```lisp
;; Single-line body
(define (add a b) (+ a b))

;; Multi-expression body — all expressions execute, last value returned
(define (log-double n)
  (print (string-append "n = " (int->str n)))
  (* n 2))

;; No parameters
(define (greet) (print "Hello!"))

;; Recursion
(define (factorial n)
  (if (= n 0)
    1
    (* n (factorial (- n 1)))))

;; Calls
(factorial 5)   ;; → 120
(greet)         ;; → "Hello!"
```

### 4. Arithmetic

```lisp
(+ 1 2 3)   ;; 6    — addition, any number of args
(- 10 3)    ;; 7    — subtraction
(- 5)       ;; -5   — unary minus
(* 3 4)     ;; 12   — multiplication
(/ 10 3)    ;; 3    — integer division
(% 10 3)    ;; 1    — modulo
```

### 5. Comparison

```lisp
(= 5 5)     ;; true
(!= 3 5)    ;; true
(< 3 5)     ;; true
(> 5 3)     ;; true
(<= 5 5)    ;; true
(>= 5 5)    ;; true
```

### 6. Logic

```lisp
(and (> 5 3) (< 1 2))   ;; true  — short-circuit: if first is false, second is not evaluated
(or  (= 1 2) (= 2 2))   ;; true  — short-circuit: if first is true, second is not evaluated
(not (= 1 2))            ;; true
```

### 7. Strings

```lisp
(string-append "Hello" ", " "World!")   ;; "Hello, World!"
(int->str 42)                            ;; "42"
```

### 8. Conditionals

```lisp
(if (> x 0)
  (print "positive")
  (print "non-positive"))
```

Truthiness: `false` and `0` are falsy; everything else (non-empty strings, non-zero numbers, lists) is truthy.

### 9. Loops

```lisp
(define-mutable i 0)
(while (< i 5)
  (print (int->str i))
  (set! i (+ i 1)))
;; Prints: 0 1 2 3 4
```

### 10. Begin — sequential execution

```lisp
(begin
  (print "step 1")
  (print "step 2"))
;; Returns value of last expression
```

Use `begin` where the syntax expects a single expression but you need multiple:

```lisp
(if (> x 0)
  (begin
    (print "positive")
    (set! x (- x 1)))
  (print "non-positive"))
```

### 11. Lists

```lisp
(define lst (list 1 2 3 4 5))
(nth lst 0)      ;; 1 — element by 0-based index
(length lst)     ;; 5 — length
```

Lists can hold values of any type (integers, strings, other lists).

### 12. Output

```lisp
(print "Hello, World!")   ;; prints string with newline
(print (int->str 42))     ;; prints "42"
```

## Built-in Functions — Full Reference

| Function | Signature | Description | Example |
|----------|-----------|-------------|---------|
| `+` | `(+ a b ...)` | Addition | `(+ 1 2 3)` → 6 |
| `-` | `(- a b ...)` / `(- a)` | Subtraction / unary minus | `(- 10 3)` → 7, `(- 5)` → -5 |
| `*` | `(* a b ...)` | Multiplication | `(* 2 3)` → 6 |
| `/` | `(/ a b)` | Integer division | `(/ 10 3)` → 3 |
| `%` | `(% a b)` | Modulo | `(% 7 2)` → 1 |
| `=` | `(= a b)` | Equal | `(= 5 5)` → true |
| `!=` | `(!= a b)` | Not equal | `(!= 3 5)` → true |
| `<` | `(< a b)` | Less than | `(< 3 5)` → true |
| `>` | `(> a b)` | Greater than | `(> 5 3)` → true |
| `<=` | `(<= a b)` | Less or equal | `(<= 5 5)` → true |
| `>=` | `(>= a b)` | Greater or equal | `(>= 5 5)` → true |
| `and` | `(and ...)` | Logical AND (short-circuit) | `(and true false)` → false |
| `or` | `(or ...)` | Logical OR (short-circuit) | `(or false true)` → true |
| `not` | `(not x)` | Logical NOT | `(not false)` → true |
| `string-append` | `(string-append ...)` | String concatenation | `(string-append "a" "b")` → "ab" |
| `int->str` | `(int->str n)` | Integer to string | `(int->str 42)` → "42" |
| `list` | `(list ...)` | Create a list | `(list 1 2 3)` |
| `nth` | `(nth lst i)` | Element by index | `(nth lst 0)` → first element |
| `length` | `(length lst)` | List length | `(length lst)` → 5 |
| `print` | `(print str)` | Print string | `(print "Hello")` |

## Data Types

| Type | Examples | Notes |
|------|---------|-------|
| `int` | `42`, `-10`, `0` | Integers (Python int) |
| `str` | `"Hello"`, `""` | Strings in double quotes |
| `list` | `(list 1 2 3)` | Heterogeneous lists |
| `bool` | result of comparisons | Python bool (true/false) |

## Error Handling

On error the interpreter prints a message and exits with code 1:

```
Runtime error: Variable not defined: x
Runtime error: Division by zero
Runtime error: Index out of range: 10
Parser error at line 3: Unexpected end of file
```

## v0.1 Limitations

1. **Integers only** — no float support
2. **No modules** — everything runs in one global namespace
3. **No exceptions** — errors terminate the program
4. **No objects or structs** — primitive types only
5. **No TCO** — deep recursion (1000+) will hit Python's recursion limit

## Roadmap

- Float support
- String operations (split, length, contains)
- Hash maps / dictionaries
- Exception handling
- Higher-order functions (map, filter, fold)
- Module system and imports
- Tail call optimization (TCO)

---

**Version**: Lupus Alpha v0.1 | **Python**: 3.10+ | **Date**: 2026
