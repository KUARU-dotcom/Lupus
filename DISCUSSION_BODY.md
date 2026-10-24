Релиз: https://github.com/KUARU-dotcom/Lupus/releases/tag/v0.5.0

## Коротко

V0.5.0 — самый большой релиз в истории проекта. Добавлена модульная система
(senko/kaltsit/w), нативная стандартная библиотека, Map, дженерик-структуры,
JSON-ошибки по спеке §6, CLI-подкоманды и фаза 2 тест-системы LiF (fuzz 10 000
программ, oracle-дифференциал против v0.4.0).

## Ключевые фичи

- **Модули**: `(import senko)`, `:as`/`:all`, файловые модули с кэшем
- **Stdlib**: senko (math), kaltsit (file), w (test) — нативно в Rust
- **Типы**: Map, дженерик-структуры `(defstruct (Node a) ...)`
- **JSON-ошибки**: stderr всегда JSON, `--pretty` для людей
- **CLI**: run / test / check / ast / eval
- **LiF фаза 2**: fuzz 10 000 программ, oracle 104 файла (92 PASS / 0 FAIL)

## Цифры

- 198 тестов (cargo test), 0 failed
- 34 golden + 108 negative файлов
- Fuzz 10 000 программ за 40 секунд, 0 паник
- Clippy 0 warnings, fmt чистый
- validate_tasks.py: 100/100
- Покрытие спеки V1.0: ~65–70% (было 35% в V0.4)

## Кто делал

- **Архитектура**: Qwen3.8 Max / Qwen3.7 Max
- **Код**: DeepSeek V4.1 Flash 1017 через OpenCode
- **Аудит**: NikoGPT (github.com/nikogpt-bot; мультимодельная агентская платформа,
  аудит выполнен на DeepSeek V4.1 Flash 1017) — независимый аудит rc1/rc2,
  нашёл и помог закрыть блокеры (паники на i64::MIN, `(exit 256)`,
  Rust-Debug в тестовом выводе)

Стоимость релиза: ~$0.20 (все агентские сессии суммарно).

## Установка

Скачать Linux/Windows бинарники из релиза выше, либо собрать из исходников:

```bash
git clone https://github.com/KUARU-dotcom/Lupus.git
cd Lupus
cargo build --release
./target/release/lupus examples/v05_modules.lupus
```

## Что дальше

V0.6: texas (net), lappland (error injection), amiya (async), kennel (host-API
для встраивания в игры/приложения).

## Ссылки

- [Релиз v0.5.0](https://github.com/KUARU-dotcom/Lupus/releases/tag/v0.5.0)
- [Спецификация V1.0](https://github.com/KUARU-dotcom/Lupus/blob/main/docs/specification-v1.0.md)
- [ERRATA](https://github.com/KUARU-dotcom/Lupus/blob/main/docs/ERRATA.md)
