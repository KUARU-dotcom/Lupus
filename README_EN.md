# Lupus Alpha v0.1

**Status:** Working prototype for hypothesis validation  
**Goal:** Verify that a small LLM (1.5B–7B) writes Lupus more correctly than Python  
**Scope:** Minimal language subset (no types, no async, no FFI)

## How to Run

```bash
# Run a file
python lupus_proto.py examples/calc.lupus

# Interactive REPL
python lupus_proto.py
```

**Requirements:** Python 3.10+, no dependencies.

## What Works

- [x] Variables — `(define x 10)`
- [x] Mutable variables — `(define-mutable x 0)`, `(set! x 1)`
- [x] Functions — `(define (fn a b) body)`
- [x] Multi-expression function body — `(define (f x) expr1 expr2 expr3)`
- [x] Closures and lexical scoping
- [x] Recursion
- [x] Arithmetic — `+`, `-`, `*`, `/`, `%` (integers only)
- [x] Unary minus — `(- 5)` → `-5`
- [x] Comparison — `=`, `!=`, `<`, `>`, `<=`, `>=`
- [x] Logic — `(and ...)`, `(or ...)`, `(not x)` (short-circuit)
- [x] Conditional — `(if cond then else)`
- [x] Loop — `(while cond body...)`
- [x] Sequencing — `(begin expr1 expr2 ...)`
- [x] Strings — `(string-append "a" "b")`, `(int->str 42)`
- [x] Lists — `(list ...)`, `(nth lst i)`, `(length lst)`
- [x] Output — `(print "text")`
- [x] Interactive REPL with result printing

## What Doesn't Work Yet

- ❌ Static typing (Hindley-Milner)
- ❌ Modules — `senko`, `texas`, `kaltsit`, `amiya`
- ❌ Async — `async/spawn`, channels
- ❌ `match`, `if-let`, `cond`
- ❌ `Option`, `Result`, `defstruct`
- ❌ FFI (calling Python from Lupus)
- ❌ Built-in tests — `(test ...)` form
- ❌ `float` (integers only)

## Examples

```lisp
;; Factorial
(define (factorial n)
  (if (= n 0) 1 (* n (factorial (- n 1)))))
(print (int->str (factorial 10)))

;; Strings
(print (string-append "Result: " (int->str (factorial 5))))

;; Logic
(if (and (> 5 3) (not (= 1 2)))
  (print "all correct")
  (print "something is wrong"))

;; Loop with mutable variable
(define-mutable i 0)
(while (< i 5)
  (print (int->str i))
  (set! i (+ i 1)))
```
