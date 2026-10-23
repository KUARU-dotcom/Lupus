# Lupus V0.4.0 — float, кортежи, структуры и тест-система LiF

Релиз: https://github.com/KUARU-dotcom/Lupus/releases/tag/v0.4.0

## Что нового
- **float**: вещественные числа, IEEE-деление (`(/ 1.0 0.0)` → `inf`),
  конверсии `int->float`, `float->int`, `str->float`, `float->str`;
  смешивание int и float — осознанная ошибка.
- **tuple**: разнородные кортежи + паттерн `(Tuple a b c)` в match.
- **defstruct**: пользовательские структуры — конструктор, аксессоры
  (`point-x`), match-паттерны `((point x y) ...)`.
- **loop / exit**: бесконечные циклы с явным выходом.
- **LiF (Lupus in Fabula)**: собственная тест-система — 17 golden-прогонов,
  70 negative-кейсов, карта покрытия на 64 символа; всё прогоняется
  через `cargo test` и CI-гейт на GitHub Actions.

## Качество
- 115 тестов (110 integration + 3 LiF + 2 doc), clippy 0 warning, fmt чисто.
- validate_tasks.py: 100/100.
- Бенчмарк merged-lupus-2bv2: 81/100 без регрессии; дообучение на синтаксисе
  V0.4 запланировано после V0.5 (датасет v3).

## Установка
Скачать бинарники (Linux/Windows) из релиза выше, либо:
git clone https://github.com/KUARU-dotcom/Lupus.git && cd Lupus
cargo build --release && ./target/release/lupus examples/v04_demo.lupus

## Дорожная карта
V0.5: модули и import, модуль senko (math) через qualified names,
LiF фаза 2 (форма test, CLI lupus test, JSONL, fuzz-smoke).

Обсуждаем здесь: баги, идеи, вопросы по синтаксису — всё приветствуется.
