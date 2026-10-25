# Fuzz (cargo-fuzz / libFuzzer)

Coverage-guided фаззинг интерпретатора. Дополняет `tests/fuzz.rs`
(структурированный рандом, 10k/20k программ в CI): тот ловит очевидное,
этот медленно и упорно лезет вглубь.

## Таргеты

| Таргет | Что фаззит | Что проверяет |
|---|---|---|
| `parse_eval` | `lexer → parser → interpreter` | паника, abort, зависание на произвольном вводе |
| `lexer` | только лексер | паника + инварианты позиций (монотонность, `col` в символах) |

## Локально

```bash
rustup toolchain install nightly
cargo install cargo-fuzz --locked

# один прогон
cargo +nightly fuzz run parse_eval -- -max_total_time=300

# корпус из golden/examples — хорошая затравка
mkdir -p fuzz/corpus/parse_eval
i=0; for f in tests/golden/*.lupus examples/*.lupus; do cp "$f" fuzz/corpus/parse_eval/seed_$i; i=$((i+1)); done

# воспроизвести находку
cargo +nightly fuzz run parse_eval fuzz/artifacts/parse_eval/crash-XXXX

# минимизировать вход для issue
cargo +nightly fuzz tmin parse_eval fuzz/artifacts/parse_eval/crash-XXXX
```

## В CI

`.github/workflows/fuzz.yml` — каждые 6 часов, 600 секунд на таргет,
артефакты в Actions, при находке автоматически заводится issue с меткой
`fuzz`. Для публичного репозитория раннеры бесплатны.

## Что НЕ считается находкой

- `Err(Error)` — штатная диагностика.
- `(exit n)` и `(loop ...)` — таргет их пропускает осознанно: первый убивает
  процесс фаззера, второй висит по определению.
- Stack overflow на глубине меньше 10 000 — таргет считает это находкой
  намеренно (лимит из ERRATA должен срабатывать раньше стека), но вычисление
  идёт на 256 MiB, чтобы не путать debug-стек с логическим лимитом.

## Обновление корпуса

Корпус коммитить не обязательно, но полезно: `fuzz/corpus/` в `.gitignore`,
в CI он пересобирается из `tests/golden` и `examples`. Если хочешь накапливать
покрытие между запусками — раскомментируй загрузку/выгрузку `corpus/`
в workflow (сейчас выгружается как артефакт, не коммитится).
