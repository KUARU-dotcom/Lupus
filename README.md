# Lupus V0.5

Репозиторий прототипа интерпретатора [Lupus](https://github.com/KUARU-dotcom/Lupus)
(фаза V0.5). Дистрибутивы и релизные артефакты лежат в `dist/`.

## Что нового в V0.5

V0.5 добавляет поверх V0.4 модули и импорты, нативную стандартную библиотеку,
`Map`, дженерик-структуры, строгие JSON-диагностики, CLI-подкоманды и вторую
фазу тестовой системы LiF.

| Возможность | Пример |
|-------------|--------|
| Импорт пакета | `(import senko)`, `(senko/sqrt 144.0)` |
| Импорт файлового модуля | `(import util)`, `(util/double 21)` |
| Переименование префикса | `(import senko :as math)`, `(math/floor 3.9)` |
| Импорт без префикса | `(import util :all)`, `(double 5)` |
| Нативный `senko` | `sqrt`, `pow`, `sin`, `log`, `floor`, `pi`, `e` и др. |
| Нативный `kaltsit` | `read-file`, `write-file`, `exists?`, `list-dir`, `mkdir`, … |
| Нативный `w` | `test`, `assert-eq`, `assert-true`, `run`, `run-all` |
| `Map` | `(map (tuple "a" 1))`, `(map-get m "a")` |
| Дженерик-структуры | `(defstruct (Node a) (value a) (left (Option (Node a))))` |
| JSON-ошибки | строгий JSON в stderr, `--pretty` для человека |
| CLI | `run`, `test`, `check`, `ast`, `eval` |

```lisp
(import senko)
(import util)

(print (senko/sqrt 144.0))   ; 12.0
(print (util/double 21))     ; 42
```

Полная семантика — в [спецификации](./docs/specification-v1.0.md), осознанные
отступления Rust-реализации — в [ERRATA](./docs/ERRATA.md).

## CLI

```bash
lupus examples/v05_demo.lupus     # запуск (линт включён)
lupus run --no-lint file.lupus    # запуск без линта
lupus check file.lupus            # только лексер/парсер/линт
lupus ast file.lupus              # JSON AST в stdout
lupus eval "(+ 2 3)"              # 5
lupus test examples/v05_tests.lupus
```

Коды выхода: `0` — успех, `1` — ошибка программы/линта/теста, `2` — ошибка
использования CLI (неизвестный флаг; plain-text usage в stderr). Диагностика
всегда пишется в stderr; stdout остаётся чистым выводом программы. `--pretty`
включает человекочитаемый рендер, `--json` (по умолчанию) — строгий JSON.

## Циклы и захват переменных

Семантика захвата различается между `while` и `for`:

- `while` — тело выполняется в том же окружении. Замыкания видят финальное
  значение переменной (позднее связывание, как `while` в Python).
- `for` — на каждую итерацию создаётся свежее дочернее окружение. Замыкания
  захватывают значение своей итерации (как `let` в `for` в JavaScript).

```lisp
(define-mutable i 0)
(define-mutable fs (list))
(while (< i 3)
    (set! fs (append fs (list (lambda () i))))
    (set! i (+ i 1)))
; все замыкания вернут 3

(define-mutable fs2 (list))
(for x in (range 0 3)
    (set! fs2 (append fs2 (list (lambda () x)))))
; замыкания вернут 0, 1, 2
```

Подробнее — в [ERRATA §10](./docs/ERRATA.md).

## Тест-система LiF (выпускные ворота)

LiF (Lupus in Fabula) — интеграционные тесты и выпускные ворота релиза:

```bash
cargo test
cargo clippy --all-targets
cargo fmt --check
python3 validate_tasks.py      # PASSED: 100/100
```

Структура:

- `tests/golden/<имя>.lupus` + `<имя>.expected` — точное совпадение stdout,
  код выхода 0.
- `tests/negative/<имя>.lupus` + `<имя>.error` — ожидаемый код ошибки; харнес
  извлекает `code` из JSON в stderr и требует ненулевой код выхода.
- `tests/coverage_map.toml` — карта покрытия V0.4/V0.5: у каждого символа есть
  positive- и negative-проверки.
- `tests/fuzz.rs` — fuzz-smoke: 2000 детерминированных программ, таймаут 2 с.
- `tests/oracle.rs` — дифференциальный тест против Lupus v0.4.0. Бинарник
  скачивается скриптом `scripts/fetch-oracle.sh` в `oracles/` (каталог в
  `.gitignore`); без него тест помечается SKIP.
- `tests/modules.rs`, `tests/stdlib.rs`, `tests/types.rs`, `tests/errors.rs`,
  `tests/cli.rs` — интеграционные тесты этапов A–E.

## Бенчмарк

Бенчмарк — 100 задач (`tasks.json`), запуск через `run_experiment.py`. Все
результаты моделей лежат в [`results/`](./results/); для каждой модели — один
JSON-файл с ответами и статусами по задачам.

| Модель | Результат | Данные |
|--------|-----------|--------|
| `merged-lupus-2bv2` | 81/100 | [`results/merged-lupus-2bv2.json`](./results/merged-lupus-2bv2.json) |
| Qwen3.5 2B (базовая) | прогон не сохранён | [`results/`](./results/) |
| Qwen3.5 4B (базовая) | прогон не сохранён | [`results/`](./results/) |
| Qwen3.5 9B (базовая) | прогон не сохранён | [`results/`](./results/) |
| Qwen3.8 4B (базовая) | прогон не сохранён | [`results/`](./results/) |

Воспроизведение: `python3 run_experiment.py --model merged-lupus-2bv2`. Для
проверки самого интерпретатора на тех же 100 задачах:
`python3 validate_tasks.py` — `PASSED: 100/100`.

## Сборка

Требуется Rust (stable, edition 2021). Внешних зависимостей нет.

```bash
cargo build --release
./target/release/lupus examples/v05_modules.lupus
./target/release/lupus test examples/v05_tests.lupus
```

## Структура

- `src/` — интерпретатор: лексер, парсер, eval, модули, нативный stdlib, линт,
  JSON-ошибки и JSON AST.
- `stdlib/util.lupus` — пример файлового модуля (`double`, `greet`).
- `tests/` — LiF, интеграционные тесты и фикстуры.
- `examples/v05_*.lupus` — демонстрации V0.5; `examples/v03_demo.lupus`,
  `examples/v04_demo.lupus` — демонстрации прошлых фаз.
- `scripts/fetch-oracle.sh` — загрузка эталонного бинарника v0.4.0.
- `results/` — JSON-результаты бенчмарка по моделям.
- `run_experiment.py` / `validate_tasks.py` / `tasks.json` — инструменты бенчмарка.
- `legacy/` — материалы предыдущих фаз.
