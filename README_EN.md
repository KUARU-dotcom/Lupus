# Lupus

> **A programming language born from dialogue. By LLM — for LLM.**
>
> *Experiment: can an architect (LLM) design a language? Can a language created by an LLM actually work? Let's find out.*

---

## About

**Lupus** is an experimental general-purpose programming language.

**Version:** Alpha (v0.1)
**Status:** Specification is ready. Interpreter is in development.

The experiment's hypothesis has three parts:
1. Can an LLM design a full-fledged language — from grammar and type system to FFI and standard library?
2. Can an LLM implement that language — interpreter, type checker, runtime?
3. Can the resulting language solve real tasks — networking, files, async, ML?

If all three hold, it proves that an LLM can be not just a tool, but an **architect**.

---

## Features

- **Prefix syntax** — `(define x 42)`, `(+ 1 2)`. Clean, unambiguous AST without syntactic sugar.
- **Static typing** — Hindley-Milner algorithm with Value Restriction. Types are inferred automatically, but can be annotated explicitly.
- **Algebraic data types** — `Option`, `Result`, user-defined structs (`defstruct`) with generics support.
- **Pattern matching** — full-featured, with exhaustiveness checking at compile time.
- **Python FFI** — standard library modules (`math`, `net`, `file`, `async`) implemented as Python wrappers via FFI.
- **Tensors** — built-in multidimensional arrays for ML experiments.
- **Async** — threads, channels (`channel`), blocking and timeout `send`/`recv` operations.
- **Built-in tests** — special form `(test "name" ...)`, isolated environments, JSON reports.
- **Deterministic AST** — JSON serialization for passing between compiler layers and for training other models.

---

## Quick Start

```bash
# Installation (available after v0.1 release)
pip install lupus-lang

# Run a program
lupus run example.lupus

# Run tests
lupus test example.lupus

# Type-check without execution
lupus check example.lupus

# Output AST as JSON
lupus ast example.lupus
```

---

## Code Examples

### Circle area calculator
```lupus
(import (senko math))

(define-public (circle-area (radius float)) -> float
  (* math/pi (* radius radius)))

(define r 10.0)
(print (string-append "Area: " (float->str (circle-area r))))

(test "circle-area-10"
  (assert (= (circle-area 10.0) 314.1592653589793)))
```

### HTTP client
```lupus
(import (texas net) :as net)

(define-public (fetch (host str) (path str)) -> (Result str str)
  (match (net/tcp-connect host 80)
    ((success sock)
      (match (net/send sock (string-append "GET " path " HTTP/1.0\r\n"))
        ((success _)
          (match (net/recv sock 8192)
            ((success response) (net/close sock) (success response))
            ((failure err) (net/close sock) (failure err))))
        ((failure err) (net/close sock) (failure err))))
    ((failure err) (failure err))))
```

### Async with channels
```lupus
(import (amiya async) :as async)

(define-public (ticker (id str) (count int) (ch (channel str)))
  (define-mutable i 0)
  (while (< i count)
    (async/sleep 500)
    (async/send ch (string-append "Tick from " id " #" (int->str i)))
    (set! i (+ i 1))))

(define ch (async/channel))
(async/spawn (lambda () (ticker "A" 3 ch)))
(async/spawn (lambda () (ticker "B" 3 ch)))

(define-mutable total 6)
(while (> total 0)
  (print (async/recv ch))
  (set! total (- total 1)))
```

### Generics and data structures
```lupus
(defstruct (Node a)
  (value a)
  (left (Option (Node a)))
  (right (Option (Node a))))

(define tree (Node 10 (some (Node 5 none none)) none))

(match tree
  ((Node v left right)
    (print (string-append "Root: " (int->str v)))))
```

---

## Architecture

```
┌─────────────────────────────────────────┐
│  CLI: run | test | check | ast          │
├─────────────────────────────────────────┤
│  Frontend: Lexer -> Parser -> AST(JSON) │
├─────────────────────────────────────────┤
│  Middle-end: Typechecker -> Linter      │
├─────────────────────────────────────────┤
│  Backend: Interpreter (tree-walk) + FFI │
├─────────────────────────────────────────┤
│  Runtime: Values + Environment + GC     │
└─────────────────────────────────────────┘
```

All errors (lexical, syntax, type, runtime, FFI) are emitted in strict JSON format with locations, hints, and context.

All AST nodes serialize to deterministic JSON, enabling:
- passing AST between compiler layers;
- using code as a dataset for LLM training;
- reading AST from other implementations (e.g., in Rust).

---

## Standard Library

| Module | Prefix | Description |
|--------|--------|-------------|
| `core` | — | Auto-import. Arithmetic, lists, tuples, Map, strings, tensors, assert, print. |
| `senko` | `math/` | Math: pi, e, sqrt, sin, cos, log, pow, abs, floor, ceil, round. |
| `texas` | `net/` | Networking: TCP/UDP sockets, connect, listen, send, recv, close. |
| `kaltsit` | `file/` | File system: read, write, append, exists, mkdir, list-dir. |
| `amiya` | `async/` | Async: spawn, channel, send, recv, recv-timeout, wait. |
| `w` | `test/` | Testing: assert-eq, assert-true, assert-false, run, run-all. |

---

## Roadmap

- [x] Language specification v1.2 (EBNF, types, FFI, tests, AST)
- [ ] Lexer and parser with AST construction
- [ ] Type checker (Hindley-Milner)
- [ ] Interpreter (tree-walk)
- [ ] FFI modules: senko (math), texas (net), kaltsit (file), amiya (async)
- [ ] CLI: run, test, check, ast
- [ ] Integration tests (all examples from the specification)
- [ ] Test coverage >= 80% for core files
- [ ] Documentation: tutorial, API reference

---

## License

MIT

---

*Created in dialogue. Verified in code.*
