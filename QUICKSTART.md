# Lupus Alpha v0.1 - Быстрый старт

## Установка

Убедитесь, что установлен **Python 3.10 или выше**. Никаких дополнительных пакетов не требуется.

## Первый запуск

### Способ 1: Запуск файла Lupus

```bash
python lupus_proto.py examples/calc.lupus
```

### Способ 2: Интерактивный режим REPL

```bash
python lupus_proto.py
```

Вводите выражения Lupus в приглашении `lupus>`. Результат каждого выражения печатается автоматически:

```
lupus> (define x 10)
lupus> (print (int->str x))
10
lupus> (+ 5 7)
12
lupus> (string-append "Hello" ", " "World!")
Hello, World!
lupus> выход
```

## Минимальный пример

Создайте файл `hello.lupus`:

```lisp
;; Комментарий
(print "Привет, Lupus!")

(define x 10)
(define y 20)

(define (add a b) (+ a b))

(print (int->str (add x y)))
```

Запустите:

```bash
python lupus_proto.py hello.lupus
```

Вывод:
```
Привет, Lupus!
30
```

## Примеры программ

В папке `examples/` находятся готовые примеры:

1. **calc.lupus** — вычисление факториалов
2. **list.lupus** — операции со списками
3. **fib.lupus** — последовательность Фибоначчи

Запустить все примеры разом:

```bash
python run_tests.py
```

## Основной синтаксис

### Переменные

```lisp
;; Неизменяемая переменная
(define name "Alice")
(define age 30)

;; Изменяемая переменная
(define-mutable counter 0)
(set! counter 1)
```

### Функции

```lisp
;; Однострочное тело
(define (add a b) (+ a b))

;; Многострочное тело — все выражения выполняются
(define (factorial n)
  (if (= n 0)
    1
    (* n (factorial (- n 1)))))

;; Функция без параметров
(define (say-hello) (print "Hello!"))
(say-hello)
```

### Условия

```lisp
(if (> x 10)
  (print "Большое число")
  (print "Маленькое число"))
```

### Логические операторы

```lisp
(and (> 5 3) (< 1 2))   ;; true — короткое замыкание
(or  (= 1 2) (= 2 2))   ;; true
(not (= 1 2))            ;; true
```

### Строки

```lisp
(string-append "Привет" ", " "мир!")  ;; "Привет, мир!"
```

### Циклы

```lisp
(define-mutable i 0)
(while (< i 5)
  (print (int->str i))
  (set! i (+ i 1)))
```

### Последовательность (begin)

```lisp
(begin
  (print "шаг 1")
  (print "шаг 2")
  (print "шаг 3"))
```

### Списки

```lisp
(define numbers (list 1 2 3 4 5))
(nth numbers 0)      ;; 1
(length numbers)     ;; 5
```

### Арифметика

```lisp
(+ 1 2 3)       ;; 6
(- 10 3)        ;; 7
(* 4 5)         ;; 20
(/ 20 4)        ;; 5
(% 7 2)         ;; 1 (остаток от деления)
```

## Встроенные функции

| Функция | Назначение |
|---------|-----------|
| `(print str)` | Вывод строки |
| `(int->str n)` | Преобразование числа в строку |
| `(string-append ...)` | Конкатенация строк |
| `(list ...)` | Создание списка |
| `(nth list i)` | Элемент списка по индексу |
| `(length list)` | Длина списка |
| `(if cond then else)` | Условный оператор |
| `(while cond body...)` | Цикл |
| `(begin forms...)` | Последовательное выполнение |
| `(and ...)` | Логическое И (короткое замыкание) |
| `(or ...)` | Логическое ИЛИ (короткое замыкание) |
| `(not x)` | Логическое НЕ |
| `(% a b)` | Остаток от деления |

## Структура проекта

```
.
├── lupus_proto.py           # Основной интерпретатор
├── run_tests.py             # Скрипт для запуска тестов
├── README_ALPHA.md          # Полная документация
├── QUICKSTART.md            # Этот файл
└── examples/
    ├── calc.lupus           # Пример: факториалы
    ├── list.lupus           # Пример: списки
    ├── fib.lupus            # Пример: Фибоначчи
    └── test_repl.lupus      # Пример: основные операции
```

## Часто задаваемые вопросы

### Q: Как использовать переменные в строках?

```lisp
(define age 25)
(print (string-append "Возраст: " (int->str age)))
```

### Q: Как создать функцию без параметров?

```lisp
(define (say-hello) (print "Hello!"))
(say-hello)
```

### Q: Как вернуть несколько значений?

```lisp
(define (get-pair) (list 1 2))
(nth (get-pair) 0)   ;; 1
```

### Q: Как проверить чётность числа?

```lisp
(define (even? n) (= (% n 2) 0))
(if (even? 4) (print "чётное") (print "нечётное"))
```

### Q: Могу ли я использовать вещественные числа?

Нет, только целые числа в этой версии. Float планируется в следующих версиях.

---

**Версия**: Lupus Alpha v0.1 | **Python**: 3.10+ | **Дата**: 2026
