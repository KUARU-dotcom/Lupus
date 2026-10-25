# Lupus V0.5

Prototype interpreter repository for [Lupus](https://github.com/KUARU-dotcom/Lupus)
(V0.5 phase). Release artifacts are placed under `dist/`.

## What's new in V0.5

V0.5 adds, on top of V0.4: modules and imports, a native standard library,
`Map`, generic structs, strict JSON diagnostics, CLI subcommands, and the second
phase of the LiF test system.

| Feature | Example |
|---------|---------|
| Package import | `(import senko)`, `(senko/sqrt 144.0)` |
| File module import | `(import util)`, `(util/double 21)` |
| Prefix alias | `(import senko :as math)`, `(math/floor 3.9)` |
| Unprefixed import | `(import util :all)`, `(double 5)` |
| Native `senko` | `sqrt`, `pow`, `sin`, `log`, `floor`, `pi`, `e`, etc. |
| Native `kaltsit` | `read-file`, `write-file`, `exists?`, `list-dir`, `mkdir`, … |
| Native `w` | `test`, `assert-eq`, `assert-true`, `run`, `run-all` |
| `Map` | `(map (tuple "a" 1))`, `(map-get m "a")` |
| Generic structs | `(defstruct (Node a) (value a) (left (Option (Node a))))` |
| JSON errors | strict JSON on stderr, `--pretty` for humans |
| CLI | `run`, `test`, `check`, `ast`, `eval` |

```lisp
(import senko)
(import util)

(print (senko/sqrt 144.0))   ; 12.0
(print (util/double 21))     ; 42
```

See the [specification](./docs/specification-v1.0.md) for the full semantics and
[ERRATA](./docs/ERRATA.md) for deliberate deviations of the Rust implementation.

## CLI

```bash
lupus examples/v05_demo.lupus     # run (lint enabled)
lupus run --no-lint file.lupus    # run without lint
lupus check file.lupus            # lexer/parser/lint only
lupus ast file.lupus              # JSON AST on stdout
lupus eval "(+ 2 3)"              # 5
lupus test examples/v05_tests.lupus
```

Exit codes: `0` — success, `1` — program/lint/test failure, `2` — CLI usage
error (unknown flag; plain-text usage on stderr). Diagnostics always go to
stderr; stdout stays clean program output. `--pretty` enables a human-readable
rendering, `--json` (the default) emits strict JSON.

## Loops and variable capture

Capture semantics differ between `while` and `for`:

- `while` — the body runs in the same environment. Closures see the final value
  of the variable (late binding, like Python's `while`).
- `for` — a fresh child environment is created on every iteration. Closures
  capture the value of their own iteration (like `let` in JavaScript's `for`).

```lisp
(define-mutable i 0)
(define-mutable fs (list))
(while (< i 3)
    (set! fs (append fs (list (lambda () i))))
    (set! i (+ i 1)))
; all closures return 3

(define-mutable fs2 (list))
(for x in (range 0 3)
    (set! fs2 (append fs2 (list (lambda () x)))))
; closures return 0, 1, 2
```

See [ERRATA §10](./docs/ERRATA.md) for details.

## LiF test system (release gates)

LiF (Lupus in Fabula) is the integration test suite and release gate:

```bash
cargo test
cargo clippy --all-targets
cargo fmt --check
python3 validate_tasks.py      # PASSED: 100/100
```

Layout:

- `tests/golden/<name>.lupus` + `<name>.expected` — exact stdout match, exit 0.
- `tests/negative/<name>.lupus` + `<name>.error` — expected error code; the
  harness extracts `code` from the JSON on stderr and requires a non-zero exit.
- `tests/coverage_map.toml` — V0.4/V0.5 coverage map: every symbol has positive
  and negative checks.
- `tests/fuzz.rs` — fuzz smoke: 2000 deterministic programs, 2 s timeout.
- `tests/oracle.rs` — differential test against Lupus v0.4.0. The binary is
  fetched by `scripts/fetch-oracle.sh` into `oracles/` (gitignored); without it
  the test is marked SKIP.
- `tests/modules.rs`, `tests/stdlib.rs`, `tests/types.rs`, `tests/errors.rs`,
  `tests/cli.rs` — integration tests for stages A–E.

## Benchmark

The benchmark is 100 tasks (`tasks.json`), run via `run_experiment.py`. All model
results live under [`results/`](./results/); each model has one JSON file with
per-task answers and statuses.

| Model | Result | Data |
|-------|--------|------|
| `merged-lupus-2bv2` | 81/100 | [`results/results_merged-lupus-2bv2_v051.json`](./results/results_merged-lupus-2bv2_v051.json) |
| Qwen3.5 2B (base) | 35/100 | [`results/results_Qwen3.5_2B_base_v051.json`](./results/results_Qwen3.5_2B_base_v051.json) |
| Qwen2.5-Coder 14B (base) | run not stored in the repository; to reproduce, run `run_experiment.py` | [`results/`](./results/) |

The V0.5.1 baseline is `merged-lupus-2bv2` (81/100). Reproduce:
`python3 run_experiment.py --model merged-lupus-2bv2`. To validate the
interpreter itself on the same 100 tasks:
`python3 validate_tasks.py` — `PASSED: 100/100`.

## Build

Requires Rust (stable, edition 2021). No external dependencies.

```bash
cargo build --release
./target/release/lupus examples/v05_modules.lupus
./target/release/lupus test examples/v05_tests.lupus
```

## Layout

- `src/` — interpreter: lexer, parser, eval, modules, native stdlib, lint,
  JSON errors, and JSON AST.
- `stdlib/util.lupus` — sample file module (`double`, `greet`).
- `tests/` — LiF, integration tests, and fixtures.
- `examples/v05_*.lupus` — V0.5 demos; `examples/v03_demo.lupus`,
  `examples/v04_demo.lupus` — demos from earlier phases.
- `scripts/fetch-oracle.sh` — v0.4.0 oracle download.
- `results/` — benchmark JSON results per model.
- `run_experiment.py` / `validate_tasks.py` / `tasks.json` — benchmark tooling.
- `legacy/` — materials from earlier phases.
