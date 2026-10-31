# Lupus V0.7.0

Репозиторий прототипа интерпретатора [Lupus](https://github.com/KUARU-dotcom/Lupus)
(фаза V0.7). Дистрибутивы и релизные артефакты публикуются в [Releases](https://github.com/KUARU-dotcom/Lupus/releases).

## Что нового в V0.7.0

- **Инструментарий качества:** grammar-based генератор, property-based тесты,
  AST-shrinker и differential fuzzing.
- **Release-гейты:** отдельный `release-gates` job в CI для release-прогона.
- **GC:** порог паузы GC в release-тесте — 20 мс, с запасом на медленный CPU.

Полный список изменений и BREAKING — в [CHANGELOG](./CHANGELOG.md).

## Что нового в V0.6.0

- **Семантика:** `and`/`or` возвращают значения, действует единая политика
  типов, конструкторы `defstruct` проверяют типы полей (ERRATA §13).
- **GC:** консервативный mark-and-sweep, `(gc-collect)` и порог
  `LUPUS_GC_THRESHOLD` (ERRATA §14).
- **Сеть и асинхронность:** пакеты `texas`, `lappland` и `amiya`
  (ERRATA §15–16).
- **Встраивание:** `kennel`, профили `pure`/`core`/`full`, fuel-лимиты и
  `lupus eval --import` (ERRATA §17).

Полный список изменений и BREAKING — в [CHANGELOG](./CHANGELOG.md).

## Что нового в V0.5.3

- **Утечка памяти в кэше модулей закрыта (#45):** `Rc`-цикл
  `окружение → замыкание → окружение` разорван, 20 000 определений в одном
  процессе: было +21 МБ, стало +0.2 МБ.
- **Встроенные функции стали значениями:** `(list-map (list 1 -2) abs)` → `(1 2)`,
  работает передача builtin как аргумента; добавлены `list-filter`, `list-fold`.
- **Runtime-ошибки с позицией:** `location` теперь заполняется и для
  `runtime`/`lint`/`type`, а не только `lex`/`parse` (ERRATA §6).
- **Фаззинг:** `cargo-fuzz` таргеты `parse_eval`/`lexer`, workflow каждые 6 часов.

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
lupus eval --import senko "(senko/sqrt 16.0)"  # 4.0
lupus test examples/v05_tests.lupus
LUPUS_FUEL=100000 lupus run file.lupus          # лимит шагов
```

`lupus eval` без `--import` работает в окружении только core; флаг
пре-импортирует перечисленные через запятую пакеты. Подробности —
в [ERRATA §17](./docs/ERRATA.md). CLI работает на профиле `full` и без
лимитов, если не задан `LUPUS_FUEL`.

Коды выхода: `0` — успех, `1` — ошибка программы/линта/теста, `2` — ошибка
использования CLI (нет аргумента, лишние позиционные, неизвестный флаг;
plain-text usage в stderr). Диагностика
всегда пишется в stderr; stdout остаётся чистым выводом программы. `--pretty`
включает человекочитаемый рендер, `--json` (по умолчанию) — строгий JSON.

## Сеть (texas)

```lisp
(import (texas net) :as net)

(define socket
  (match (net/tcp-connect "127.0.0.1" 8080)
    ((success value) value)
    ((failure message) (print message) (exit 1))))
(print (net/send socket "ping"))
(net/close socket)
```

`texas` возвращает `(success ...)`/`(failure "net: ...")` вместо исключений.
Канонический импорт — `(import (texas net) :as net)`; короткие алиасы
`net/tcp-send`, `net/udp-recv` и другие работают так же. Полная семантика —
в [ERRATA §15](./docs/ERRATA.md).

## Инъекция ошибок (lappland)

```lisp
(import lappland)
(import (texas net) :as net)

(lappland/malice 1.0)  ; каждый следующий net-вызов → failure
(lappland/delay 1.5)   ; дополнительно 5 мс на каждый вызов
(print (net/tcp-connect "127.0.0.1" 9)) ; (failure "net: injected malice")
```

`malice` принимает `float` в диапазоне `0.0..=1.0`, `delay` — `float >= 1.0`.
`LUPUS_MALICE_SEED` делает последовательность отказов воспроизводимой
([ERRATA §15](./docs/ERRATA.md)).

## Асинхронность (amiya)

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

`spawn` принимает только замыкание без параметров; задача обязана вернуть
`unit`. Значения и каналы переносятся через `SendValue`, а окружение
копируется в момент запуска ([ERRATA §16](./docs/ERRATA.md)).

## Встраивание (kennel)

Публичный host-API `lupus::kennel` запускает код Lupus внутри Rust-процесса,
захватывает `print` и позволяет ограничить возможности программы:

```rust
use lupus::kennel::{self, Config, Host, Profile};
use lupus::value::{Arity, Value};

fn main() -> Result<(), kennel::JsonError> {
    let mut host = Host::new();
    host.register_native("double", Arity::Exact(1), |args| match &args[0] {
        Value::Int(n) => Ok(Value::Int(n * 2)),
        _ => Err("host/double ожидает int".to_string()),
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

| Профиль | Core-биндинги | `kaltsit` | `senko`/`w` | `texas`/`amiya`/`lappland` | host |
|---------|---------------|-----------|-------------|----------------------------|------|
| `pure`  | да            | нет       | нет         | нет                        | нет  |
| `core`  | да            | да        | нет         | нет                        | нет  |
| `full`  | да            | да        | да          | да                         | да   |

`Profile::Pure` запрещает любые импорты; `Profile::Core` разрешает `kaltsit` и
файловые модули; `Profile::Full` — все нативные пакеты и `host/<имя>` (профиль
CLI). Нарушение даёт `profile-denied` с именем пакета.

`Config` также задаёт `max_steps`/`fuel`, `max_memory_values` и `max_depth`.
Исчерпание лимита возвращает `JsonError` с кодом `exhausted`; `Host`-функция
вызывается как `(host/<имя> ...)` без `import`, а её `Err(String)` становится
кодом `host-error`. Форма `(set-fuel! n)` может только уменьшать остаток.
Полная таблица профилей и семантика лимитов — в
[ERRATA §17](./docs/ERRATA.md).

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
cargo clippy --all-targets -- -D warnings
cargo fmt --check
python3 validate_tasks.py      # PASSED: 100/100
```

Структура:

- `tests/golden/<имя>.lupus` + `<имя>.expected` — точное совпадение stdout,
  код выхода 0.
- `tests/negative/<имя>.lupus` + `<имя>.error` — ожидаемый код ошибки; харнес
  извлекает `code` из JSON в stderr и требует ненулевой код выхода.
- `tests/coverage_map.toml` — карта покрытия V0.4–V0.6: у каждого символа есть
  positive- и negative-проверки.
- `tests/fuzz.rs` — fuzz-smoke: 10 000 детерминированных программ, таймаут 2 с.
- `tests/oracle.rs` — дифференциальный тест против Lupus v0.4.0. Бинарник
  скачивается скриптом `scripts/fetch-oracle.sh` в `oracles/` (каталог в
  `.gitignore`); без него тест помечается SKIP.
- `tests/property.rs` — property-based тесты семантики: 9 свойств на
  детерминированных входах; при нарушении — минимальный контрпример
  через shrink (V0.7).
- `tests/differential.rs` — differential-тесты против эталона V0.5.2:
  сравнение exit/stdout/stderr.code, известные breaking changes
  классифицируются как ожидаемые (V0.7).
- `tests/shrinker.rs` — минимизация воспроизводителя бага на уровне AST:
  500 строк → 1 (V0.7).
- `tests/modules.rs`, `tests/stdlib.rs`, `tests/types.rs`, `tests/errors.rs`,
  `tests/cli.rs`, `tests/kennel.rs` — интеграционные тесты этапов A–F.

## Бенчмарк

Бенчмарк — 100 задач (`tasks.json`), запуск через `run_experiment.py`. Все
результаты моделей лежат в [`results/`](./results/); для каждой модели — один
JSON-файл с ответами и статусами по задачам.

| Модель | Результат | Данные |
|--------|-----------|--------|
| `merged-lupus-2bv2` | 81/100 | [`results/results_merged-lupus-2bv2_v051.json`](./results/results_merged-lupus-2bv2_v051.json) |
| Qwen3.5 2B (базовая) | 35/100 | [`results/results_Qwen3.5_2B_base_v051.json`](./results/results_Qwen3.5_2B_base_v051.json) |
| Qwen2.5-Coder 14B (базовая) | прогон не сохранён в репозитории; для воспроизведения запустите `run_experiment.py` | [`results/`](./results/) |

В таблице приведены только сохранённые прогоны из `results/` — не все
запускавшиеся модели, а те, чьи JSON-файлы лежат в репозитории.

Базовая линия V0.5.1 (новых замеров в V0.5.3 не делалось) — `merged-lupus-2bv2` (81/100). Воспроизведение:
`python3 run_experiment.py --model merged-lupus-2bv2`. Для проверки самого
интерпретатора на тех же 100 задачах: `python3 validate_tasks.py` —
`PASSED: 100/100`.

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
- `examples/v05_*.lupus` — демонстрации V0.5; `examples/v06_net.lupus`,
  `examples/v06_async.lupus` — сеть и асинхронность V0.6; `examples/v03_demo.lupus`,
  `examples/v04_demo.lupus` — демонстрации прошлых фаз.
- `scripts/fetch-oracle.sh` — загрузка эталонного бинарника v0.4.0.
- `results/` — JSON-результаты бенчмарка по моделям.
- `run_experiment.py` / `validate_tasks.py` / `tasks.json` — инструменты бенчмарка.
- `legacy/` — материалы предыдущих фаз.
