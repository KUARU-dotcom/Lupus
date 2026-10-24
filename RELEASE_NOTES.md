# Release Notes — Lupus V0.5.0

Дата: 2026-10-25
Тег: v0.5.0
Коммит: 68d186c

## Что нового

### Модульная система
- `(import senko)`, `(import kaltsit)`, `(import w)` — нативные пакеты в Rust
- Префиксы: `senko/pi`, `kaltsit/read-file`, `w/assert-eq`
- Модификаторы `:as` и `:all` (`:all` под линтом `import-collision`)
- Файловые модули с кэшем загрузки (исполняются ровно один раз)
- Детекция циклических импортов (`import-cycle`)
- Env-переменная `LUPUS_PATH` для пользовательских каталогов модулей

### Нативная стандартная библиотека (Rust, без Python-FFI)
- `senko` (math): pi, e, sqrt, sin, cos, tan, pow, log, log10, abs, floor, ceil, round, min, max
- `kaltsit` (file): read-file, write-file, append-file, exists?, is-dir?, list-dir, mkdir, delete, size
- `w` (test): форма `(test "имя" body...)`, assert-eq/true/false, JSONL-вывод по §5.3

### Новые типы
- `Map` — ассоциативные массивы: map-get, map-set, map-keys, map-values, map-has?
- Дженерик-структуры: `(defstruct (Node a) (value a) (next (Option (Node a))))`

### JSON-ошибки (§6 спеки V1.0)
- stderr всегда строгий JSON с полями severity, phase, code, message, location, hint, context
- Флаг `--pretty` для человекочитаемого рендера
- 32 кода ошибок, включая новые: `integer-overflow`, `invalid-exit-code`,
  `stack-depth-exceeded`, `nesting-too-deep`, `io-error`, `import-collision`, `import-cycle`

### CLI
- `lupus run <file>` (и по умолчанию)
- `lupus test <path>` — прогон тестов с JSONL-выводом
- `lupus check <file>` — линтер (unused-variable, core-shadowing, missing-return-type и др.)
- `lupus ast <file>` — JSON-сериализация AST (§7 спеки)
- `lupus eval "<expr>"` — REPL для одного выражения
- Флаги: `--pretty`, `--verbose`, `--no-lint`, `--json`, `--lenient-parens`, `--version`

### Тест-система LiF фаза 2
- Fuzz: 10 000 программ локально, 20 000 в CI (env `LUPUS_FUZZ_N`)
- Oracle-дифференциал против v0.4.0: 104 файла, 92 PASS / 0 FAIL / 12 SKIP
- Coverage map: 111 символов языка с positive+negative ссылками
- Структурированные генераторы: random, edge_cases, type_mix, modules, strings

### Безопасность рантайма
- Лимит глубины рекурсии: 10 000 (`stack-depth-exceeded` вместо SIGSEGV)
- Лимит вложенности скобок: 1 000 (`nesting-too-deep` вместо паники парсера)
- Проверенная арифметика: `checked_div`/`checked_add`/`checked_mul` — ни одной паники на i64::MIN
- Eval в отдельном потоке со стеком 1 GiB (обоснование: debug-сборка переполняет
  стандартный стек раньше логического лимита)

## Breaking changes от V0.4

- **FFI через Python заменён нативными модулями.** Директива `(#lupus ffi python "...")`
  теперь даёт ошибку линта `invalid-directive` с hint о замене.
- **Negative-тесты**: `.error`-файлы теперь содержат код ошибки (одна строка), а не
  русскую подстроку. Харнес `lif` матчит поле `code` из JSON stderr.
- **`run_experiment.py`**: детект ошибок только по `returncode != 0`, без подстрок stdout.
- **`(exit n)`**: валидация 0..=255, иначе runtime-ошибка `invalid-exit-code`.
- **Арифметика i64**: переполнения дают `integer-overflow` вместо паники.

## Известные ограничения

- HM-тайпчекер не реализован (отложен в V1.0). Аннотации типов в `define-public`
  проверяются только синтаксически (наличие `->` и типов параметров).
- Tensor (`exusiai`) отложен в V2.0.
- macOS-сборка: кросс-компиляция требует Apple SDK (osxcross), бинарники не поставляются.
- Oracle-дифференциал: для exit≠0 сравнивает stdout по префиксу (V0.4.0 печатал
  диагностику в stdout, V0.5 — JSON в stderr). Расхождение в хвосте текста ошибки
  дифференциалом не ловится.

## Метрики

| Метрика | V0.4.0 | V0.5.0 | Δ |
|---|---|---|---|
| Тестов cargo | 115 | 198 | +72% |
| Golden-файлов | 17 | 34 | +100% |
| Negative-файлов | 71 | 108 | +52% |
| Fuzz-программ | 2 000 | 10 000 (20 000 в CI) | ×5 (×10) |
| Oracle-файлов | 34 | 104 | +206% |
| Кодов ошибок | ~25 | 32 | +7 |
| Покрытие спеки V1.0 | ~35% | ~65–70% | +30 п.п. |

## Установка

```bash
# Из релизного архива
wget https://github.com/KUARU-dotcom/Lupus/releases/download/v0.5.0/lupus-v0.5.0-linux-x86_64.tar.gz
tar xzf lupus-v0.5.0-linux-x86_64.tar.gz
./lupus --version

# Из исходников
git clone https://github.com/KUARU-dotcom/Lupus.git
cd Lupus
cargo build --release
./target/release/lupus --version
```

## Разработка

- **Архитектура и спецификация**: Qwen3.8 Max / Qwen3.7 Max
- **Реализация**: DeepSeek V4.1 Flash (через OpenCode)
- **Аудит**: NikoGPT (github.com/nikogpt-bot; мультимодельная агентская платформа,
  аудит выполнен на DeepSeek V4.1 Flash 1017) — независимый аудит rc1/rc2,
  выявление блокеров, проверка borrow-паник в RefCell, кэша модулей, oracle-логики
- **Стоимость релиза**: ~$0.20 (суммарно все агентские сессии)

## Документация

- [Спецификация V1.0](docs/specification-v1.0.md)
- [Отступления от спеки (ERRATA)](docs/ERRATA.md)
- [GitHub Discussion](https://github.com/KUARU-dotcom/Lupus/discussions/22)
