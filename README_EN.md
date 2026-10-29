# Lupus V0.7.0

Prototype interpreter repository for [Lupus](https://github.com/KUARU-dotcom/Lupus)
(V0.7 phase). Release artifacts are published on the [Releases](https://github.com/KUARU-dotcom/Lupus/releases) page.

## What's new in V0.7.0

- **Quality tooling:** grammar-based generator, property-based tests,
  AST shrinker, and differential fuzzing.
- **Release gates:** a separate `release-gates` job in CI for the release
  profile.
- **GC:** the GC-pause threshold in the release test is 20 ms, with headroom
  for slow CPUs.

See the [CHANGELOG](./CHANGELOG.md) for the full list and the breaking changes.

## What's new in V0.6.0

- **Semantics:** `and`/`or` return values, one type policy for builtins, and
  `defstruct` constructors check field types (ERRATA §13).
- **GC:** conservative mark-and-sweep, `(gc-collect)`, and the
  `LUPUS_GC_THRESHOLD` threshold (ERRATA §14).
- **Networking and async:** `texas`, `lappland`, and `amiya`
  (ERRATA §15–16).
- **Embedding:** `kennel`, `pure`/`core`/`full` profiles, fuel limits, and
  `lupus eval --import` (ERRATA §17).

See the [CHANGELOG](./CHANGELOG.md) for the full list and the breaking changes.

## What's new in V0.5.3

- **Module cache memory leak fixed (#45):** the `Rc` cycle
  `environment → closure → environment` is broken; 20,000 definitions in one
  process: +21 MB before, +0.2 MB after.
- **Builtins are first-class values:** `(list-map (list 1 -2) abs)` → `(1 2)`;
  `list-filter` and `list-fold` added.
- **Runtime errors carry a position:** `location` is now filled for
  `runtime`/`lint`/`type`, not only `lex`/`parse` (ERRATA §6).
- **Fuzzing:** `cargo-fuzz` targets `parse_eval`/`lexer`, scheduled workflow.

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
lupus eval --import senko "(senko/sqrt 16.0)"  # 4.0
lupus test examples/v05_tests.lupus
LUPUS_FUEL=100000 lupus run file.lupus          # step limit
```

Without `--import`, `lupus eval` uses a core-only environment; the flag
pre-imports a comma-separated list of packages. See
[ERRATA §17](./docs/ERRATA.md) for details. The CLI runs with the `full`
profile and no limits unless `LUPUS_FUEL` is set.

Exit codes: `0` — success, `1` — program/lint/test failure, `2` — CLI usage
error (missing argument, extra positional arguments, unknown flag; plain-text
usage on stderr). Diagnostics always go to
stderr; stdout stays clean program output. `--pretty` enables a human-readable
rendering, `--json` (the default) emits strict JSON.

## Networking (texas)

```lisp
(import (texas net) :as net)

(define socket
  (match (net/tcp-connect "127.0.0.1" 8080)
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(print (net/send socket "ping"))
(net/close socket)
```

`texas` returns `(success ...)`/`(failure "net: ...")` instead of exceptions.
The canonical import is `(import (texas net) :as net)`; short aliases such as
`net/tcp-send` and `net/udp-recv` behave the same. Full semantics are in
[ERRATA §15](./docs/ERRATA.md).

## Fault injection (lappland)

```lisp
(import lappland)
(import (texas net) :as net)

(lappland/malice 1.0)  ; every following net call → failure
(lappland/delay 1.5)   ; extra 5 ms per call
(print (net/tcp-connect "127.0.0.1" 9)) ; (failure "net: injected malice")
```

`malice` takes a `float` in `0.0..=1.0`; `delay` takes a `float >= 1.0`.
`LUPUS_MALICE_SEED` makes the failure sequence reproducible
([ERRATA §15](./docs/ERRATA.md)).

## Asynchronous execution (amiya)

```lisp
(import (amiya async) :as async)

(define ch (async/channel))
(define worker
  (async/spawn
    (lambda ()
      (async/send ch "ready")
      (async/send ch 42))))
(print (async/recv ch))
(print (async/recv ch))
(async/wait worker)
```

`spawn` accepts only a zero-argument closure; a task must return `unit`.
Values and channels are marshalled through `SendValue`, and the environment is
snapshotted when the task starts ([ERRATA §16](./docs/ERRATA.md)).

## Embedding (kennel)

The public `lupus::kennel` host API runs Lupus code inside a Rust process,
captures `print`, and restricts what the program can do:

```rust
use lupus::kennel::{self, Config, Host, Profile};
use lupus::value::{Arity, Value};

fn main() -> Result<(), kennel::JsonError> {
    let mut host = Host::new();
    host.register_native("double", Arity::Exact(1), |args| match &args[0] {
        Value::Int(n) => Ok(Value::Int(n * 2)),
        _ => Err("host/double expects int".to_string()),
    });

    let config = Config {
        profile: Profile::Full,
        fuel: Some(10_000),
        ..Config::default()
    };
    let output = kennel::run_program("(print (host/double 21))", &config, &host)?;
    assert_eq!(output, "42\n");
    Ok(())
}
```

| Profile | Core bindings | `kaltsit` | `senko`/`w` | `texas`/`amiya`/`lappland` | host |
|---------|---------------|-----------|-------------|----------------------------|------|
| `pure`  | yes           | no        | no          | no                         | no   |
| `core`  | yes           | yes       | no          | no                         | no   |
| `full`  | yes           | yes       | yes         | yes                        | yes  |

`Profile::Pure` denies all imports; `Profile::Core` allows `kaltsit` and file
modules; `Profile::Full` enables every native package and `host/<name>` (the CLI
profile). A violation returns `profile-denied` with the package name.

`Config` also sets `max_steps`/`fuel`, `max_memory_values`, and `max_depth`.
Exhaustion returns a `JsonError` with code `exhausted`; host functions are called
as `(host/<name> ...)` without an `import`, and `Err(String)` becomes
`host-error`. `(set-fuel! n)` can only lower the remaining budget. The full
profile table and limit semantics are in [ERRATA §17](./docs/ERRATA.md).

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
cargo clippy --all-targets -- -D warnings
cargo fmt --check
python3 validate_tasks.py      # PASSED: 100/100
```

Layout:

- `tests/golden/<name>.lupus` + `<name>.expected` — exact stdout match, exit 0.
- `tests/negative/<name>.lupus` + `<name>.error` — expected error code; the
  harness extracts `code` from the JSON on stderr and requires a non-zero exit.
- `tests/coverage_map.toml` — V0.4–V0.6 coverage map: every symbol has positive
  and negative checks.
- `tests/fuzz.rs` — fuzz smoke: 10 000 deterministic programs, 2 s timeout.
- `tests/oracle.rs` — differential test against Lupus v0.4.0. The binary is
  fetched by `scripts/fetch-oracle.sh` into `oracles/` (gitignored); without it
  the test is marked SKIP.
- `tests/modules.rs`, `tests/stdlib.rs`, `tests/types.rs`, `tests/errors.rs`,
  `tests/cli.rs`, `tests/kennel.rs` — integration tests for stages A–F.

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
- `examples/v05_*.lupus` — V0.5 demos; `examples/v06_net.lupus`,
  `examples/v06_async.lupus` — V0.6 networking and async; `examples/v03_demo.lupus`,
  `examples/v04_demo.lupus` — demos from earlier phases.
- `scripts/fetch-oracle.sh` — v0.4.0 oracle download.
- `results/` — benchmark JSON results per model.
- `run_experiment.py` / `validate_tasks.py` / `tasks.json` — benchmark tooling.
- `legacy/` — materials from earlier phases.
