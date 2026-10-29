# Release Notes — Lupus V0.7.0

Дата: 2026-10-29
Тег: v0.7.0
Коммит: см. тег `v0.7.0`

## Что нового

### Инструментарий качества V0.7

V0.7.0 — релиз «инструментальной» итерации: ядро интерпретатора не менялось,
но появились инфраструктура проверки качества и release-гейты.

| Направление | Что появилось | Итог |
|---|---|---|
| Grammar-based fuzzing | `src/generator.rs` | валидность программ **43.79% → 100%** |
| Property-based testing | `src/property.rs` | **9 свойств** семантики |
| Shrinking | `src/shrinker.rs` | 500-строчный фейл → **1 строка** |
| Differential fuzzing | `src/differential.rs` | **165** сравнений с V0.5.2, **0 unexpected** |

Плюс:

- **Фикс порога паузы GC.** Тест `tests/memory.rs::memory_leak_containers`
  теперь проверяет паузу в release-сборке с порогом **20 000 µs**
  (`LUPUS_GC_PAUSE_LIMIT_US` можно поднять на медленном CPU). Проверку утечки
  не трогали — она проходит.
- **CI:** отдельный `release-gates` job для release-прогона по `memory`,
  `gc_performance`, `property`, `differential` и `shrinker`; job
  `windows-build` — сборка и тесты на настоящей Windows (`windows-latest`,
  host MSVC). Debug-джоб остался без изменений.

## Почему V0.7.0, а не 0.6.1

- появились новые модули и гейты;
- обновилась тестовая база и CI;
- поведение интерпретатора не менялось, но сборка теперь проверяется и в
  release-профиле.

## Известные ограничения

- **GC-пауза.** Тест `memory_leak_containers` проверяет паузу GC только в
  release-сборке: в debug-сборке трассировка ощутимо медленнее. Порог 20 мс
  выбран с запасом, чтобы ловить деградацию на порядок, а не шум.
- HM-тайпчекер не реализован (V1.0). Аннотации типов проверяются
  синтаксически.
- Tensor (`exusiai`) отложен в V2.0.
- **Windows.** Релизный `.zip` собирается на машине мейнтейнера (CachyOS)
  через `./build_release.sh <version>` (кросс-компиляция mingw-w64) или через
  `Dockerfile.release`. Job `windows-build` в CI проверяет сборку и тесты на
  настоящей Windows, но не производит релизный артефакт. До V0.7.0
  Windows-сборка в CI не проверялась вообще.
- macOS-сборка не поставляется (нужен Apple SDK).

## Метрики

| Метрика | V0.6.0 | V0.7.0 | Δ |
|---|---|---|---|
| Валидность генератора | 43.79% | **100%** | +56.21 п.п. |
| Property-свойств | 0 | 9 | +9 |
| AST-shrink | — | 500 → 1 строк | новое |
| Differential-корпус | 0 | 165 сравнений | +165 |
| Неожиданных расхождений | — | **0** | — |

## Установка

```bash
wget https://github.com/KUARU-dotcom/Lupus/releases/download/v0.7.0/lupus-v0.7.0-linux-x86_64.tar.gz
tar xzf lupus-v0.7.0-linux-x86_64.tar.gz
./lupus --version
```

Из исходников:

```bash
git clone https://github.com/KUARU-dotcom/Lupus.git
cd Lupus && cargo build --release
./target/release/lupus --version
```

## Разработка

- **Архитектура и спецификация**: Qwen3.8 Max / Qwen3.7 Max
- **Реализация**: DeepSeek V4.1 Flash
- **Аудит и фаззинг**: NikoGPT (github.com/nikogpt-bot; мультимодельная
  агентская платформа, аудит выполнен на DeepSeek V4.1 Flash 1017)

## Документация

- [Спецификация V1.0](docs/specification-v1.0.md)
- [Отступления от спеки (ERRATA)](docs/ERRATA.md)
