# Lupus V0.4

Репозиторий прототипа интерпретатора [Lupus](https://github.com/KUARU-dotcom/Lupus) (фаза V0.4).
Дистрибутивы и релизные артефакты V0.4 лежат в `dist/` (см. раздел «Сборка»).

## Что нового в V0.4

V0.4 добавляет поверх V0.3 числа с плавающей точкой, кортежи, пользовательские
структуры (`defstruct`), цикл `loop`/`exit`, а также тест-систему LiF — выпускные
ворота релиза.

| Возможность | Пример |
|-------------|--------|
| Float: литералы и арифметика | `(+ 1.5 2.25)` → `3.75` |
| Float: деление IEEE | `(/ 1.0 0.0)` → `inf`, `(/ 1.0 (- 0.0))` → `-inf` |
| Float: конверсии | `(int->float 7)`, `(float->int 3.7)` → `3`, `(str->float "3.5")` |
| Float: равенство | `(= 1 1.0)` → `false` |
| Tuple: кортежи | `(tuple 1 "a" true)` |
| Tuple: match-паттерн | `(match t ((tuple a b c) ...))` |
| `defstruct`: структуры | `(defstruct point (x int) (y int))` |
| Аксессоры | `(point-x p)`, `(point-y p)` |
| `loop` / `exit` | `(loop ... (exit 0))` |

Полный набор возможностей и семантику см. в [спецификации](./docs/specification-v1.0.md).

## Тест-система LiF (выпускные ворота)

LiF (Lupus in Fabula) — набор интеграционных тестов, которые становятся
выпускными воротами релиза. Прогоняется одной командой:

```bash
cargo test
```

Структура:

- `tests/golden/<имя>.lupus` + `<имя>.expected` — позитивные тесты: точное
  совпадение stdout байт-в-байт, код выхода 0.
- `tests/negative/<имя>.lupus` + `<имя>.error` — негативные тесты: подстрока
  сообщения об ошибке, код выхода обязан быть ненулевым.
- `tests/coverage_map.toml` — карта покрытия: каждый реализованный символ
  (спецформы + builtins + фичи V0.4) → списки `positive` и `negative`.
- `tests/lif.rs` — харнес: обходит golden/ и negative/, сверяет вывод и коды
  выхода, проверяет карту покрытия. Падение любого файла проваливает тест.

## Сборка

Требуется Rust (stable, edition 2021). Сборка и запуск:

```bash
cargo build --release
./target/release/lupus examples/v04_demo.lupus
```

Тесты и проверки:

```bash
cargo test
cargo clippy --all-targets
cargo fmt --check
python validate_tasks.py      # PASSED: 100/100
```

## Структура

- `src/` — исходный код интерпретатора на Rust.
- `tests/integration.rs` — интеграционные тесты.
- `tests/lif.rs` + `tests/{golden,negative}/` + `tests/coverage_map.toml` — тест-система LiF.
- `examples/v03_demo.lupus`, `examples/v04_demo.lupus` — демонстрации.
- `run_experiment.py` / `validate_tasks.py` / `tasks.json` — инструменты бенчмарка.
- `results_*.json` — результаты экспериментов на LM Studio.
- `legacy/` — материалы предыдущих фаз (см. также [спецификацию](./docs/specification-v1.0.md)).
