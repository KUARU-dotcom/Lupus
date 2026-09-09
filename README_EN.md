# Lupus

> **A programming language built in dialogue. From LLM — for LLM.**
>
> *Experiment: can an LLM design a language? Will a language designed by an LLM work better for LLM?*

---

## About

**Lupus** is an experimental general-purpose programming language designed for optimized interaction with LLMs (large language models).

**Version:** v0.2 (Rust Rewrite) · **Status:** Interpreter in active development.

The experiment has three hypotheses:

1. Can an LLM design a complete language — from grammar and type system to FFI and standard library?
2. Can an LLM implement that language — interpreter, parser, typechecker, runtime?
3. Will a small LLM (1.5B–7B parameters) make 20%+ fewer errors writing Lupus than Python?

If all three hold, it proves that LLMs can act not just as tools, but as **language architects**.

---

## Quick Start

### From Source (Rust)

```bash
git clone https://github.com/KUARU-dotcom/Lupus
cd Lupus

# Build
cargo build --release

# Run a program
./target/release/lupus examples/calc.lupus

# Interactive REPL (planned for v0.3)
# ./target/release/lupus --repl
```

**Requirements:** Rust 1.70+. No external dependencies.

### From Binaries

Download pre-built binaries for Linux/Windows from the [Releases](https://github.com/KUARU-dotcom/Lupus/releases) page.

```bash
# Linux/macOS
./lupus examples/calc.lupus

# Windows
lupus.exe examples/calc.lupus
```

### Legacy Python Prototype (v0.1)

To use the original Python prototype, switch to the `prototype` branch:

```bash
git checkout prototype
python lupus_proto.py examples/calc.lupus
```

---

## Features

- **Prefix syntax** — `(define x 42)`, `(+ 1 2)`. Unambiguous AST, no syntactic sugar.
- **Static typing** — type inference algorithm (Hindley-Milner planned). Types inferred automatically.
- **Pattern matching** — `match` with exhaustiveness checking.
- **Functions and lambdas** — `define`, `lambda`, closures.
- **Control flow** — `if`, `while`, `define-mutable`, `set!`.
- **Built-in types** — integers, floats, strings, lists, booleans.
- **Built-in functions** — arithmetic, string operations, list operations, printing.
- **Deterministic AST** — designed for training LLMs on clean syntax.

---

## Code Examples

### Basic Arithmetic and Definitions

```lisp
(define pi 3.14159)
(define radius 10.0)
(define area (* pi (* radius radius)))
(print area)  ; => 314.159
```

### Functions

```lisp
(define (square x)
  (* x x))

(define (circle-area r)
  (* 3.14159 (* r r)))

(print (square 5))           ; => 25
(print (circle-area 10.0))   ; => 314.159
```

### Conditionals and Loops

```lisp
(define (abs x)
  (if (< x 0) (- x) x))

(define (factorial n)
  (define-mutable result 1)
  (define-mutable i 1)
  (while (<= i n)
    (set! result (* result i))
    (set! i (+ i 1)))
  result)

(print (factorial 5))  ; => 120
```

### Lists and Data Processing

```lisp
(define numbers (list 1 2 3 4 5))
(print (car numbers))      ; => 1
(print (length numbers))   ; => 5
(print (reverse numbers))  ; => (5 4 3 2 1)
```

### Lambdas and Closures

```lisp
(define (make-adder x)
  (lambda (y) (+ x y)))

(define add5 (make-adder 5))
(print (add5 3))  ; => 8
```

---

## Interpreter Architecture

```
┌──────────────────────────────────────────────────────┐
│  CLI: lupus [--lenient-parens] file.lupus            │
├──────────────────────────────────────────────────────┤
│  Lexer (src/lexer.rs)                                │
│  Transforms text into tokens                         │
├──────────────────────────────────────────────────────┤
│  Parser (src/parser.rs)                              │
│  Transforms tokens into AST (Abstract Syntax Tree)   │
├──────────────────────────────────────────────────────┤
│  Interpreter (src/interpreter.rs)                    │
│  Executes AST in an environment (tree-walk)          │
├──────────────────────────────────────────────────────┤
│  Runtime (src/value.rs, src/environment.rs)          │
│  Stores values, variables, closures                  │
└──────────────────────────────────────────────────────┘
```

### Project Structure

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Public library API
├── lexer.rs             # Lexical analyzer
├── parser.rs            # Syntax analyzer
├── ast.rs               # AST definitions
├── interpreter.rs       # Interpreter
├── value.rs             # Value representation
├── environment.rs       # Variable environment
├── error.rs             # Error handling
└── builtins/            # Built-in functions
    ├── arithmetic.rs    # Planned
    ├── io.rs            # Planned
    └── list.rs          # Planned
```

---

## Supported Syntax (v0.2)

### Definitions and Variables

| Form | Example | Description |
|------|---------|-------------|
| `define` | `(define x 42)` | Define a variable |
| `define-mutable` | `(define-mutable x 0)` | Define a mutable variable |
| `set!` | `(set! x 10)` | Mutate a variable |

### Functions

| Form | Example | Description |
|------|---------|-------------|
| `define` with function | `(define (f x) (+ x 1))` | Define a function |
| `lambda` | `(lambda (x) (+ x 1))` | Anonymous function |

### Control Flow

| Form | Example | Description |
|------|---------|-------------|
| `if` | `(if (> x 0) "pos" "neg")` | Conditional branching |
| `while` | `(while (< i 10) (set! i (+ i 1)))` | While loop |

### Built-in Functions

| Function | Examples |
|----------|----------|
| Arithmetic | `(+ 1 2)`, `(- 5 3)`, `(* 2 3)`, `(/ 10 2)` |
| Comparison | `(= a b)`, `(< x y)`, `(> x y)`, `(<= x y)`, `(>= x y)` |
| Logic | `(and true false)`, `(or true false)`, `(not true)` |
| Strings | `(string-append "a" "b")`, `(string-length "hello")` |
| Lists | `(list 1 2 3)`, `(car lst)`, `(cdr lst)`, `(length lst)`, `(reverse lst)` |
| Output | `(print x)`, `(println x)` |

---

## Historical Information

### Branches

| Branch | Contents | Status |
|--------|----------|--------|
| `main` | Current documentation. v0.2 release. | Active |
| `prototype` | Original Python prototype (v0.1/v0.1e). | Archive |
| `specification` | Full language specification v1.0 (EBNF). | Archive |

### Versioning

- **v0.1 / v0.1e** — Python prototype (`prototype` branch). Deprecated.
- **v0.2** — Complete Rust rewrite (`main` branch). Active development.
- **v1.0** — Planned stable release with full feature set.

---

## Roadmap

- [x] Language specification (EBNF, types, AST)
- [x] Python prototype interpreter (v0.1)
- [x] Rust rewrite of interpreter (v0.2)
- [x] Basic lexing, parsing, evaluation
- [ ] Built-in functions (modules in `src/builtins/`)
- [ ] Typechecker (Hindley-Milner)
- [ ] Interactive REPL
- [ ] FFI modules (math, net, file, async)
- [ ] Standard library
- [ ] Performance optimization
- [ ] API documentation

---

## License

MIT

---

*Built in dialogue. Verified in code.*
