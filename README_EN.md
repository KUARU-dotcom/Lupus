# Lupus V0.4

Prototype interpreter repository for [Lupus](https://github.com/KUARU-dotcom/Lupus) (V0.4 phase).
V0.4 release artifacts are placed under `dist/` (see "Build" below).

## What's new in V0.4

V0.4 adds, on top of V0.3: floating-point numbers, tuples, user-defined structs
(`defstruct`), a `loop`/`exit` cycle, and the LiF test system — the release gates.

| Feature | Example |
|---------|---------|
| Float: literals and arithmetic | `(+ 1.5 2.25)` → `3.75` |
| Float: IEEE division | `(/ 1.0 0.0)` → `inf`, `(/ 1.0 (- 0.0))` → `-inf` |
| Float: conversions | `(int->float 7)`, `(float->int 3.7)` → `3`, `(str->float "3.5")` |
| Float: equality | `(= 1 1.0)` → `false` |
| Tuple: tuples | `(tuple 1 "a" true)` |
| Tuple: match pattern | `(match t ((tuple a b c) ...))` |
| `defstruct`: structs | `(defstruct point (x int) (y int))` |
| Accessors | `(point-x p)`, `(point-y p)` |
| `loop` / `exit` | `(loop ... (exit 0))` |

See the [specification](./docs/specification-v1.0.md) for the full feature set and semantics.

## LiF test system (release gates)

LiF (Lupus in Fabula) is a set of integration tests that act as the release gates.
Run everything with one command:

```bash
cargo test
```

Layout:

- `tests/golden/<name>.lupus` + `<name>.expected` — positive tests: exact
  byte-for-byte stdout match, exit code 0.
- `tests/negative/<name>.lupus` + `<name>.error` — negative tests: error-message
  substring, exit code must be non-zero.
- `tests/coverage_map.toml` — coverage map: every implemented symbol
  (special forms + builtins + V0.4 features) → `positive` and `negative` lists.
- `tests/lif.rs` — harness: walks `golden/` and `negative/`, compares stdout and
  exit codes, and validates the coverage map. Failure of any file fails the test.

## Build

Requires Rust (stable, edition 2021). Build and run:

```bash
cargo build --release
./target/release/lupus examples/v04_demo.lupus
```

Tests and checks:

```bash
cargo test
cargo clippy --all-targets
cargo fmt --check
python validate_tasks.py      # PASSED: 100/100
```

## Layout

- `src/` — Rust interpreter source.
- `tests/integration.rs` — integration tests.
- `tests/lif.rs` + `tests/{golden,negative}/` + `tests/coverage_map.toml` — the LiF test system.
- `examples/v03_demo.lupus`, `examples/v04_demo.lupus` — feature demos.
- `run_experiment.py` / `validate_tasks.py` / `tasks.json` — benchmark tooling.
- `results_*.json` — LM Studio experiment results.
- `legacy/` — materials from earlier phases (see also the [specification](./docs/specification-v1.0.md)).
