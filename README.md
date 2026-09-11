# Lupus V0.3

Репозиторий прототипа интерпретатора [Lupus](https://github.com/KUARU-dotcom/Lupus) (фаза V0.3).
Дистрибутивы и релизные артефакты V0.3 лежат в `dist/` (см. раздел «Сборка»).

## Что нового в V0.3

V0.3 — крупное расширение языка поверх V0.2. Добавлены условная конструкция `cond`,
циклы `for`/`range`, локальные связывания `let`, сопоставление `match`, `if-let`,
утверждения `assert`, функции высшего порядка для списков, типобезопасные `Option`
и `Result`, работа со строками и дополнительные арифметические функции.

| Возможность | Пример |
|-------------|--------|
| `cond` (многоусловный ветвитель) | `(cond ((< n 0) "neg") (else "pos"))` |
| `for … in (range …)` | `(for i in (range 1 11) (print i))` |
| `let` (локальные связывания) | `(let ((a 3) (b 4)) (+ a b))` |
| `match` (сопоставление с образцом) | `(match (some 42) (none 0) ((some v) v))` |
| `if-let` (условное связывание) | `(if-let (v (some 7)) (* v 2) 0)` |
| `assert` (утверждения) | `(assert (> x 0) "x должен быть > 0")` |
| Списки: `cons`/`head`/`tail`/`empty?`/`append` | `(cons 1 (list 2 3))` |
| `list-map`/`list-filter`/`list-fold` | `(list-map (list 1 2 3) (lambda (x) (* x 2)))` |
| Строки: `string-length`/`split`/`reverse` | `(string-reverse "abc")` |
| `str->int` (Option) | `(str->int "42")` → `(some 42)` |
| Арифметика: `pow`/`min`/`max`/`abs`/`mod` | `(pow 2 10)` |
| `Option`: `some`/`none` | `(some 42)`, `none` |
| `Result`: `success`/`failure` | `(success 1)`, `(failure "boom")` |
| `print` любого значения | `(print (failure "boom"))` → `(failure "boom")` |

Полный набор возможностей и семантику см. в [спецификации](./Lupus_V1.0_Specification_Russian.md).

## Исправление UTF-8

В V0.2 была обнаружена проблема с разбором строковых литералов в UTF-8
(подробности обсуждались в Discussions репозитория). В V0.3 баг устранён
в лексере: строки с кириллицей и эмодзи корректно читаются и печатаются.

```lisp
(print "привет 🐺")   ; → привет 🐺
```

## Сборка

Требуется Rust (stable, edition 2021). Сборка и запуск:

```bash
cargo build --release
./target/release/lupus examples/v03_demo.lupus
```

Тесты (74: 72 integration + 2 doc) и проверки:

```bash
cargo test --release
cargo clippy --all-targets
cargo fmt --check
python validate_tasks.py      # PASSED: 100/100
```

## Структура

- `src/` — исходный код интерпретатора на Rust.
- `tests/integration.rs` — интеграционные тесты.
- `examples/v03_demo.lupus` — демонстрация возможностей V0.3.
- `run_experiment.py` / `validate_tasks.py` / `tasks.json` — инструменты бенчмарка.
- `results_*.json` — результаты экспериментов на LM Studio.
- `legacy/` — материалы предыдущих фаз (см. также [спецификацию](./Lupus_V1.0_Specification_Russian.md)).
