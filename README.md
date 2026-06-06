# Lupus Alpha v0.1

**Статус:** Рабочий прототип для проверки гипотезы  
**Цель:** Проверить, что LLM (1.5B–7B) пишет на Lupus правильнее, чем на Python  
**Ограничения:** Минимальное подмножество языка (без типов, без async, без FFI)

## Как запустить

```bash
# Запуск файла
python lupus_proto.py examples/calc.lupus

# Интерактивный REPL
python lupus_proto.py
```

**Требования:** Python 3.10+, никаких зависимостей.

## Что работает

- [x] Переменные — `(define x 10)`
- [x] Изменяемые переменные — `(define-mutable x 0)`, `(set! x 1)`
- [x] Функции — `(define (fn a b) body)`
- [x] Многострочное тело функции — `(define (f x) expr1 expr2 expr3)`
- [x] Замыкания и лексический скопинг
- [x] Рекурсия
- [x] Арифметика — `+`, `-`, `*`, `/`, `%` (только int)
- [x] Унарный минус — `(- 5)` → `-5`
- [x] Сравнение — `=`, `!=`, `<`, `>`, `<=`, `>=`
- [x] Логика — `(and ...)`, `(or ...)`, `(not x)` (короткое замыкание)
- [x] Условный оператор — `(if cond then else)`
- [x] Цикл — `(while cond body...)`
- [x] Последовательность — `(begin expr1 expr2 ...)`
- [x] Строки — `(string-append "a" "b")`, `(int->str 42)`
- [x] Списки — `(list ...)`, `(nth lst i)`, `(length lst)`
- [x] Вывод — `(print "text")`
- [x] Интерактивный REPL с выводом результатов

## Что НЕ работает (планируется)

- ❌ Статическая типизация (Hindley-Milner)
- ❌ Модули — `senko`, `texas`, `kaltsit`, `amiya`
- ❌ Асинхронность — `async/spawn`, каналы
- ❌ `match`, `if-let`, `cond`
- ❌ `Option`, `Result`, `defstruct`
- ❌ FFI (вызов Python из Lupus)
- ❌ Встроенные тесты — форма `(test ...)`
- ❌ `float` (только `int`)

## Примеры

```lisp
;; Факториал
(define (factorial n)
  (if (= n 0) 1 (* n (factorial (- n 1)))))
(print (int->str (factorial 10)))

;; Строки
(print (string-append "Результат: " (int->str (factorial 5))))

;; Логика
(if (and (> 5 3) (not (= 1 2)))
  (print "всё верно")
  (print "что-то не так"))

;; Цикл с изменяемой переменной
(define-mutable i 0)
(while (< i 5)
  (print (int->str i))
  (set! i (+ i 1)))
```
