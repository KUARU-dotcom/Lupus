# Lupus

> **Язык программирования, созданный в диалоге. От LLM — для LLM.**
>
> *Эксперимент: сможет ли архитектор (LLM) спроектировать язык? Сможет ли язык, созданный LLM, работать? Проверим.*

---

## О проекте

**Lupus** — экспериментальный язык программирования общего назначения.

**Версия:** Alpha (v0.1)
**Статус:** Спецификация готова. Интерпретатор в разработке.

Гипотеза эксперимента состоит из трех частей:
1. Сможет ли LLM спроектировать полноценный язык — от грамматики и системы типов до FFI и стандартной библиотеки?
2. Сможет ли LLM реализовать этот язык — интерпретатор, тайпчекер, runtime?
3. Сможет ли получившийся язык решать реальные задачи — сеть, файлы, асинхронность, ML?

Если все три пункта выполнятся, это будет доказательством того, что LLM может выступать не только инструментом, но и архитектором.

---

## Особенности

- **Префиксный синтаксис** — `(define x 42)`, `(+ 1 2)`. Чистый, однозначный AST без синтаксического сахара.
- **Статическая типизация** — алгоритм Hindley-Milner с ограничением значений (Value Restriction). Типы выводятся автоматически, но можно аннотировать явно.
- **Алгебраические типы** — `Option`, `Result`, пользовательские структуры (`defstruct`) с поддержкой дженериков.
- **Паттерн-матчинг** — полноценный, с проверкой исчерпываемости на этапе компиляции.
- **FFI в Python** — модули стандартной библиотеки (`math`, `net`, `file`, `async`) реализованы как Python-обертки через механизм FFI.
- **Тензоры** — встроенная поддержка многомерных массивов для экспериментов в области ML.
- **Асинхронность** — потоки, каналы (`channel`), блокирующие и таймаутные операции `send`/`recv`.
- **Встроенные тесты** — специальная форма `(test "name" ...)`, изолированные окружения, JSON-отчеты.
- **Детерминированный AST** — JSON-сериализация AST для передачи между слоями компилятора и для обучения других моделей.

---

## Быстрый старт

```bash
# Установка (будет доступно после релиза v0.1)
pip install lupus-lang

# Запуск программы
lupus run example.lupus

# Запуск тестов
lupus test example.lupus

# Проверка типов без выполнения
lupus check example.lupus

# Вывод AST в JSON
lupus ast example.lupus
```

---

## Примеры кода

### Калькулятор площади круга
```lupus
(import (senko math))

(define-public (circle-area (radius float)) -> float
  (* math/pi (* radius radius)))

(define r 10.0)
(print (string-append "Area: " (float->str (circle-area r))))

(test "circle-area-10"
  (assert (= (circle-area 10.0) 314.1592653589793)))
```

### HTTP-клиент
```lupus
(import (texas net) :as net)

(define-public (fetch (host str) (path str)) -> (Result str str)
  (match (net/tcp-connect host 80)
    ((success sock)
      (match (net/send sock (string-append "GET " path " HTTP/1.0\r\n"))
        ((success _)
          (match (net/recv sock 8192)
            ((success response) (net/close sock) (success response))
            ((failure err) (net/close sock) (failure err))))
        ((failure err) (net/close sock) (failure err))))
    ((failure err) (failure err))))
```

### Асинхронность с каналами
```lupus
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
```lupus
(defstruct (Node a)
  (value a)
  (left (Option (Node a)))
  (right (Option (Node a))))

(define tree (Node 10 (some (Node 5 none none)) none))

(match tree
  ((Node v left right)
    (print (string-append "Root: " (int->str v)))))
```

---

## Архитектура

```
┌─────────────────────────────────────────┐
│  CLI: run | test | check | ast          │
├─────────────────────────────────────────┤
│  Frontend: Lexer -> Parser -> AST(JSON) │
├─────────────────────────────────────────┤
│  Middle-end: Typechecker -> Linter      │
├─────────────────────────────────────────┤
│  Backend: Interpreter (tree-walk) + FFI │
├─────────────────────────────────────────┤
│  Runtime: Values + Environment + GC     │
└─────────────────────────────────────────┘
```

Все ошибки (лексические, синтаксические, типовые, runtime, FFI) выводятся в строгом JSON-формате с локациями, подсказками и контекстом.

Все узлы AST сериализуются в детерминированный JSON, что позволяет:
- передавать AST между слоями компилятора;
- использовать код как датасет для обучения LLM;
- читать AST из других реализаций (например, на Rust).

---

## Стандартная библиотека

| Модуль | Префикс | Описание |
|--------|---------|----------|
| `core` | — | Автоимпорт. Арифметика, списки, кортежи, Map, строки, тензоры, assert, print. |
| `senko` | `math/` | Математика: pi, e, sqrt, sin, cos, log, pow, abs, floor, ceil, round. |
| `texas` | `net/` | Сеть: TCP/UDP сокеты, connect, listen, send, recv, close. |
| `kaltsit` | `file/` | Файловая система: read, write, append, exists, mkdir, list-dir. |
| `amiya` | `async/` | Асинхронность: spawn, channel, send, recv, recv-timeout, wait. |
| `w` | `test/` | Тестирование: assert-eq, assert-true, assert-false, run, run-all. |

---

## Roadmap

- [x] Спецификация языка v1.2 (EBNF, типы, FFI, тесты, AST)
- [ ] Лексер и парсер с построением AST
- [ ] Тайпчекер (Hindley-Milner)
- [ ] Интерпретатор (tree-walk)
- [ ] FFI-модули: senko (math), texas (net), kaltsit (file), amiya (async)
- [ ] CLI: run, test, check, ast
- [ ] Интеграционные тесты (все примеры из спецификации)
- [ ] Покрытие тестами >= 80% для core-файлов
- [ ] Документация: tutorial, API reference

---

## Лицензия

MIT

---

*Создано в диалоге. Проверяется в коде.*
