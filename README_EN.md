# Lupus V0.3

Prototype interpreter repository for [Lupus](https://github.com/KUARU-dotcom/Lupus) (V0.3 phase).
V0.3 release artifacts are placed under `dist/` (see "Build" below).

## What's new in V0.3

V0.3 is a major language extension on top of V0.2: the `cond` conditional, `for`/`range` loops,
`let` local bindings, `match` pattern matching, `if-let`, `assert`, higher-order functions for
lists, type-safe `Option` and `Result`, string facilities, and additional arithmetic functions.

| Feature | Example |
|---------|---------|
| `cond` (multi-branch conditional) | `(cond ((< n 0) "neg") (else "pos"))` |
| `for … in (range …)` | `(for i in (range 1 11) (print i))` |
| `let` (local bindings) | `(let ((a 3) (b 4)) (+ a b))` |
| `match` (pattern matching) | `(match (some 42) (none 0) ((some v) v))` |
| `if-let` (conditional binding) | `(if-let (v (some 7)) (* v 2) 0)` |
| `assert` (assertions) | `(assert (> x 0) "x must be > 0")` |
| Lists: `cons`/`head`/`tail`/`empty?`/`append` | `(cons 1 (list 2 3))` |
| `list-map`/`list-filter`/`list-fold` | `(list-map (list 1 2 3) (lambda (x) (* x 2)))` |
| Strings: `string-length`/`split`/`reverse` | `(string-reverse "abc")` |
| `str->int` (Option) | `(str->int "42")` → `(some 42)` |
| Arithmetic: `pow`/`min`/`max`/`abs`/`mod` | `(pow 2 10)` |
| `Option`: `some`/`none` | `(some 42)`, `none` |
| `Result`: `success`/`failure` | `(success 1)`, `(failure "boom")` |
| `print` of any value | `(print (failure "boom"))` → `(failure "boom")` |

See the [specification](./Lupus_V1.0_Specification_Russian.md) for the full feature set and semantics.

## UTF-8 fix

V0.2 had a bug parsing UTF-8 string literals (discussed in the repository's Discussions).
V0.3 fixes it in the lexer: strings with Cyrillic and emoji are read and printed correctly.

```lisp
(print "привет 🐺")   ; → привет 🐺
```

## Build

Requires Rust (stable, edition 2021). Build and run:

```bash
cargo build --release
./target/release/lupus examples/v03_demo.lupus
```

Tests (74: 72 integration + 2 doc) and checks:

```bash
cargo test --release
cargo clippy --all-targets
cargo fmt --check
python validate_tasks.py      # PASSED: 100/100
```

## Layout

- `src/` — Rust interpreter source.
- `tests/integration.rs` — integration tests.
- `examples/v03_demo.lupus` — V0.3 feature demo.
- `run_experiment.py` / `validate_tasks.py` / `tasks.json` — benchmark tooling.
- `results_*.json` — LM Studio experiment results.
- `legacy/` — materials from earlier phases (see also the [specification](./Lupus_V1.0_Specification_Russian.md)).
