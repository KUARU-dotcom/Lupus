# Lupus

> **Язык программирования, созданный в диалоге. От LLM — для LLM.**
>
> *Эксперимент: сможет ли LLM спроектировать язык? Сможет ли язык, созданный LLM, работать лучше для LLM?*

---

## О проекте

**Lupus** — экспериментальный язык программирования общего назначения.

**Версия:** Alpha (v0.1) · **Статус:** Прототип интерпретатора готов.

Гипотеза эксперимента состоит из трёх частей:

1. Сможет ли LLM спроектировать полноценный язык — от грамматики и системы типов до FFI и стандартной библиотеки?
2. Сможет ли LLM реализовать этот язык — интерпретатор, тайпчекер, runtime?
3. Будет ли LLM (1.5B–7B параметров) делать на 20%+ меньше ошибок на Lupus, чем на Python?

Если все три пункта выполнятся, это докажет, что LLM может выступать не только инструментом, но и **архитектором**.

---

## Быстрый старт

```bash
git clone https://github.com/KUARU-dotcom/Lupus
cd Lupus
git checkout prototype

python lupus_proto.py examples/calc.lupus  # запуск файла
python lupus_proto.py                       # интерактивный REPL
```

**Требования:** Python 3.10+, никаких зависимостей.

Полноценный CLI (`lupus run`, `lupus check`, `lupus ast`) запланирован в v0.2 на Rust.

---

## Особенности

- **Префиксный синтаксис** — `(define x 42)`, `(+ 1 2)`. Чистый однозначный AST без синтаксического сахара.
- **Статическая типизация** — алгоритм Hindley-Milner с Value Restriction. Типы выводятся автоматически.
- **Алгебраические типы** — `Option`, `Result`, пользовательские структуры (`defstruct`) с дженериками.
- **Паттерн-матчинг** — с проверкой исчерпываемости на этапе компиляции.
- **FFI в Python** — модули stdlib реализованы как Python-обёртки.
- **Тензоры** — встроенная поддержка многомерных массивов для ML.
- **Асинхронность** — потоки, каналы, `send`/`recv` с таймаутом.
- **Встроенные тесты** — форма `(test "name" ...)`, изолированные окружения, JSON-отчёты.
- **Детерминированный AST** — JSON-сериализация для передачи между слоями и для обучения LLM.

---

## Примеры кода

### Калькулятор площади круга

```lisp
(import (senko math))

(define-public (circle-area (radius float)) -> float
  (* math/pi (* radius radius)))

(define r 10.0)
(print (string-append "Area: " (float->str (circle-area r))))

(test "circle-area-10"
  (assert (= (circle-area 10.0) 314.1592653589793)))
```

### HTTP-клиент

```lisp
(import (texas net) :as net)

(define-public (fetch (host str) (path str)) -> (Result str str)
  (match (net/tcp-connect host 80)
    ((success sock)
      (match (net/send sock (string-append "GET " path " HTTP/1.0\r\n"))
        ((success _)
          (match (net/recv sock 8192)
            ((success response) (net/close sock) (success response))
            ((failure err)      (net/close sock) (failure err))))
        ((failure err) (net/close sock) (failure err))))
    ((failure err) (failure err))))
```

### Асинхронность с каналами

```lisp
(import (amiya async) :as async)

(define-public (ticker (id str) (count int) (ch (channel str)))
  (define-mutable i 0)
  (while (< i count)
    (async/sleep 500)
    (async/send ch (string-append "Tick from " id " #" (int->str i)))
    (set! i (+ i 1))))

(define ch (async/channel))
(async/spawn (lambda () (ticker "A" 3 ch)))
(async/spawn (lambda () (ticker "B" 3 ch)))

(define-mutable total 6)
(while (> total 0)
  (print (async/recv ch))
  (set! total (- total 1)))
```

### Дженерики и структуры данных

```lisp
(defstruct (Node a)
  (value a)
  (left  (Option (Node a)))
  (right (Option (Node a))))

(define tree (Node 10 (some (Node 5 none none)) none))

(match tree
  ((Node v _ _)
    (print (string-append "Root: " (int->str v)))))
```

---

## Архитектура

```
┌─────────────────────────────────────────┐
│  CLI: run | test | check | ast          │
├─────────────────────────────────────────┤
│  Frontend: Lexer → Parser → AST (JSON)  │
├─────────────────────────────────────────┤
│  Middle-end: Typechecker → Linter       │
├─────────────────────────────────────────┤
│  Backend: Interpreter (tree-walk) + FFI │
├─────────────────────────────────────────┤
│  Runtime: Values + Environment + GC     │
└─────────────────────────────────────────┘
```

Все ошибки выводятся в строгом JSON-формате с локациями и подсказками.
AST сериализуется детерминированно — для передачи между слоями и обучения LLM.

---

## Стандартная библиотека

| Модуль | Префикс | Описание |
|--------|---------|----------|
| `core` | — | Автоимпорт. Арифметика, списки, Map, строки, тензоры, assert, print. |
| `senko` | `math/` | Математика: pi, e, sqrt, sin, cos, log, pow, abs, floor, ceil. |
| `texas` | `net/` | Сеть: TCP/UDP сокеты, connect, listen, send, recv, close. |
| `kaltsit` | `file/` | Файловая система: read, write, append, exists, mkdir, list-dir. |
| `amiya` | `async/` | Асинхронность: spawn, channel, send, recv, recv-timeout, wait. |
| `w` | `test/` | Тестирование: assert-eq, assert-true, run, run-all. |

---

## Ветки

| Ветка | Содержимое |
|-------|-----------|
| `main` | Этот файл. Описание проекта и видение. |
| `specification` | Полная спецификация языка v1.0 (EBNF, типы, FFI, AST). |
| `prototype` | Рабочий прототип интерпретатора Alpha v0.1 на Python. |

---

## Roadmap

- [x] Спецификация языка v1.0 (EBNF, типы, FFI, тесты, AST)
- [x] Прототип интерпретатора (tree-walk) — ветка `prototype`
- [ ] Тайпчекер (Hindley-Milner)
- [ ] FFI-модули: senko (math), texas (net), kaltsit (file), amiya (async)
- [ ] CLI: run, test, check, ast
- [ ] Эксперимент: 100 задач, сравнение Lupus vs Python на малых LLM
- [ ] Rust-реализация (при успехе эксперимента)

---

## Лицензия

MIT

---

*Создано в диалоге. Проверяется в коде.*
