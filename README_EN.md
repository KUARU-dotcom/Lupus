# Lupus

> **A programming language built in dialogue. From LLM — for LLM.**
>
> *Experiment: can an LLM design a language? Will a language designed by an LLM work better for LLM?*

---

## About

**Lupus** is an experimental general-purpose programming language.

**Version:** Alpha (v0.1) · **Status:** Interpreter prototype is ready.

The experiment has three hypotheses:

1. Can an LLM design a complete language — from grammar and type system to FFI and standard library?
2. Can an LLM implement that language — interpreter, typechecker, runtime?
3. Will a small LLM (1.5B–7B parameters) make 20%+ fewer errors writing Lupus than Python?

If all three hold, it proves that LLMs can act not just as tools, but as **architects**.

---

## Quick Start

```bash
git clone https://github.com/KUARU-dotcom/Lupus
cd Lupus
git checkout prototype

python lupus_proto.py examples/calc.lupus  # run a file
python lupus_proto.py                       # interactive REPL
```

**Requirements:** Python 3.10+, no dependencies.

A full CLI (`lupus run`, `lupus check`, `lupus ast`) is planned for v0.2 in Rust.

---

## Features

- **Prefix syntax** — `(define x 42)`, `(+ 1 2)`. Unambiguous AST, no syntactic sugar.
- **Static typing** — Hindley-Milner with Value Restriction. Types inferred automatically.
- **Algebraic types** — `Option`, `Result`, user-defined structs (`defstruct`) with generics.
- **Pattern matching** — exhaustiveness checked at compile time.
- **Python FFI** — standard library modules implemented as Python wrappers.
- **Tensors** — built-in multi-dimensional arrays for ML experiments.
- **Async** — threads, channels, `send`/`recv` with timeout.
- **Built-in tests** — `(test "name" ...)` form, isolated environments, JSON reports.
- **Deterministic AST** — JSON serialization for pipeline transfer and LLM training data.

---

## Code Examples

### Circle area calculator

```lisp
(import (senko math))

(define-public (circle-area (radius float)) -> float
  (* math/pi (* radius radius)))

(define r 10.0)
(print (string-append "Area: " (float->str (circle-area r))))

(test "circle-area-10"
  (assert (= (circle-area 10.0) 314.1592653589793)))
```

### HTTP client

```lisp
(import (texas net) :as net)

(define-public (fetch (host str) (path str)) -> (Result str str)
  (match (net/tcp-connect host 80)
    ((success sock)
      (match (net/send sock (string-append "GET " path " HTTP/1.0\r\n"))
        ((success _)
          (match (net/recv sock 8192)
            ((success response) (net/close sock) (success response))
            ((failure err)      (net/close sock) (failure err))))
        ((failure err) (net/close sock) (failure err))))
    ((failure err) (failure err))))
```

### Async with channels

```lisp
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

```lisp
(defstruct (Node a)
  (value a)
  (left  (Option (Node a)))
  (right (Option (Node a))))

(define tree (Node 10 (some (Node 5 none none)) none))

(match tree
  ((Node v _ _)
    (print (string-append "Root: " (int->str v)))))
```

---

## Architecture

```
┌─────────────────────────────────────────┐
│  CLI: run | test | check | ast          │
├─────────────────────────────────────────┤
│  Frontend: Lexer → Parser → AST (JSON)  │
├─────────────────────────────────────────┤
│  Middle-end: Typechecker → Linter       │
├─────────────────────────────────────────┤
│  Backend: Interpreter (tree-walk) + FFI │
├─────────────────────────────────────────┤
│  Runtime: Values + Environment + GC     │
└─────────────────────────────────────────┘
```

All errors are emitted as structured JSON with locations and hints.
AST is serialized deterministically — for pipeline transfer and LLM training.

---

## Standard Library

| Module | Prefix | Description |
|--------|--------|-------------|
| `core` | — | Auto-imported. Arithmetic, lists, Map, strings, tensors, assert, print. |
| `senko` | `math/` | Math: pi, e, sqrt, sin, cos, log, pow, abs, floor, ceil. |
| `texas` | `net/` | Networking: TCP/UDP sockets, connect, listen, send, recv, close. |
| `kaltsit` | `file/` | File system: read, write, append, exists, mkdir, list-dir. |
| `amiya` | `async/` | Async: spawn, channel, send, recv, recv-timeout, wait. |
| `w` | `test/` | Testing: assert-eq, assert-true, run, run-all. |

---

## Branches

| Branch | Contents |
|--------|---------|
| `main` | This file. Project overview and vision. |
| `specification` | Full language specification v1.0 (EBNF, types, FFI, AST). |
| `prototype` | Working Alpha v0.1 interpreter prototype in Python. |

---

## Roadmap

- [x] Language specification v1.0 (EBNF, types, FFI, tests, AST)
- [x] Interpreter prototype (tree-walk) — `prototype` branch
- [ ] Typechecker (Hindley-Milner)
- [ ] FFI modules: senko (math), texas (net), kaltsit (file), amiya (async)
- [ ] CLI: run, test, check, ast
- [ ] Experiment: 100 tasks, Lupus vs Python on small LLMs
- [ ] Rust implementation (if experiment succeeds)

---

## License

MIT

---

*Built in dialogue. Verified in code.*
