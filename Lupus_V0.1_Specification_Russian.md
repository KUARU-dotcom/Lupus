# LUPUS v1.2 — Финальная спецификация языка программирования (исправленная, финальная)

**Версия:** 1.2 (Final Corrected Specification)  
**Дата:** 2026-06-02  
**Статус:** Готов к реализации (Ready for Implementation)  
**Целевая аудитория:** GPT-5.5 Mimi, разработчики интерпретатора, архитекторы компилятора  

---

## Содержание

1. [Грамматика EBNF](#1-грамматика-ebnf)
2. [Встроенные типы и операции](#2-встроенные-типы-и-операции)
3. [Стандартная библиотека (API)](#3-стандартная-библиотека-api)
4. [Спецификация FFI](#4-спецификация-ffi)
5. [Формат тестов](#5-формат-тестов)
6. [Формат ошибок (JSON)](#6-формат-ошибок-json)
7. [Сериализация AST (JSON)](#7-сериализация-ast-json)
8. [Рекомендации по реализации (Python)](#8-рекомендации-по-реализации-python)
9. [Примеры программ](#9-примеры-программ)
10. [Чек-лист для GPT-5.5 Mimi](#10-чек-лист-для-gpt-55-mimi)

---

## 1. Грамматика EBNF

### 1.1. Лексические правила

```ebnf
(* === Лексер === *)

program       = { toplevel } , EOF ;

toplevel      = define
              | define_mutable
              | define_const
              | define_public
              | defstruct
              | import
              | test
              | directive ;

(* --- Токены --- *)

IDENTIFIER    = LETTER , { LETTER | DIGIT | "_" | "-" } ;
              (* Примечание: "/" НЕ входит в IDENTIFIER. Квалифицированные имена разбираются на этапе парсера. *)

TYPE_NAME     = UPPERCASE_LETTER , { LETTER | DIGIT | "_" } ;
              (* List, Option, Result, Tuple, Map, Tensor, Func, int, float, bool, str, unit *)

STRING        = '"' , { CHAR | ESCAPE } , '"' ;
CHAR          = any Unicode character except '"' and '\\' and control chars ;
ESCAPE        = '\\' , ('"' | '\\' | 'n' | 't' | 'r' | '0' | ('x' , HEX_DIGIT , HEX_DIGIT)) ;

INTEGER       = [ "-" ] , DIGIT , { DIGIT } ;
FLOAT         = [ "-" ] , DIGIT , { DIGIT } , "." , DIGIT , { DIGIT } , [ ("e" | "E") , ["-" | "+"] , DIGIT , {DIGIT} ] ;
BOOLEAN       = "true" | "false" ;

LITERAL_NONE  = "none" ;
LITERAL_UNIT  = "unit" ;

COMMENT_LINE  = ";;" , { any character except newline } , newline ;
COMMENT_BLOCK = "#|" , { any character } , "|#" ;

KEYWORD       = "define" | "define-mutable" | "define-const" | "define-public" | "set!" | "lambda"
              | "if" | "cond" | "else" | "match" | "if-let"
              | "while" | "for" | "in" | "loop"
              | "import" | "as" | "all" | "defstruct"
              | "test" | "assert"
              | "some" | "success" | "failure"
              | "->" | "#lupus" ;

(* --- Символы --- *)
LPAREN        = "(" ;
RPAREN        = ")" ;
ARROW         = "->" ;
SLASH         = "/" ;

(* --- Вспомогательные --- *)
LETTER        = "a" .. "z" | "A" .. "Z" ;
UPPERCASE_LETTER = "A" .. "Z" ;
DIGIT         = "0" .. "9" ;
HEX_DIGIT     = DIGIT | "a" .. "f" | "A" .. "F" ;
```

### 1.2. Синтаксические правила

```ebnf
(* === Парсер === *)

(* --- Верхний уровень --- *)

define        = LPAREN , "define" , IDENTIFIER , expr , RPAREN ;
              (* (define x 10) *)

define_mutable = LPAREN , "define-mutable" , IDENTIFIER , expr , RPAREN ;
              (* (define-mutable y 20) *)

define_const  = LPAREN , "define-const" , IDENTIFIER , expr , RPAREN ;
              (* (define-const pi 3.1416) — immutable, compile-time constant *)

define_public = LPAREN , "define-public" , func_header , body , RPAREN ;
              (* (define-public (add (a int) (b int)) -> int (+ a b)) *)
              (* Обязательна аннотация возвращаемого типа. *)
              (* Параметры могут быть с аннотацией: (a int) или без: a *)
              (* Если параметр без аннотации, тип выводится, но для public-функции *)
              (* тайп-чекер может потребовать явную аннотацию (warn). *)

func_header   = LPAREN , IDENTIFIER , { param } , RPAREN , [ "->" , type_expr ] ;
              (* Имя функции + параметры + опциональная аннотация возвращаемого типа *)

param         = IDENTIFIER
              | LPAREN , IDENTIFIER , type_expr , RPAREN ;
              (* аннотированный параметр: (a int) *)

set_bang      = LPAREN , "set!" , IDENTIFIER , expr , RPAREN ;
              (* (set! x (+ x 1)) *)

lambda        = LPAREN , "lambda" , LPAREN , { param } , RPAREN , [ "->" , type_expr ] , body , RPAREN ;
              (* (lambda (x) (* x x)) *)
              (* (lambda ((x int)) -> int (* x x)) *)

(* --- Тело (последовательность выражений) --- *)

body          = { expr } ;
              (* Одно или несколько выражений. Результат тела — значение последнего выражения. *)
              (* Все выражения кроме последнего должны иметь тип unit (или игнорируются). *)

(* --- Управляющие конструкции (все являются выражениями) --- *)

if_expr       = LPAREN , "if" , expr , expr , expr , RPAREN ;
              (* (if condition then-expr else-expr) *)

cond_expr     = LPAREN , "cond" , { cond_clause } , RPAREN ;
cond_clause   = LPAREN , expr , expr , RPAREN
              | LPAREN , "else" , expr , RPAREN ;

match_expr    = LPAREN , "match" , expr , { match_clause } , RPAREN ;
match_clause  = LPAREN , pattern , body , RPAREN ;
              (* ((some value) (print value)) — pattern + body *)

pattern       = "_"                              (* wildcard *)
              | IDENTIFIER                       (* переменная, связывается с любым значением *)
              | LPAREN , constructor , { pattern } , RPAREN ;
              (* (some value), (success data), (point x y) *)
              | LPAREN , "Tuple" , { pattern } , RPAREN ;
              (* (Tuple a b c) — паттерн-матчинг по кортежу *)

constructor   = IDENTIFIER ;
              (* none, some, success, failure, point, и т.д. *)

if_let        = LPAREN , "if-let" , LPAREN , IDENTIFIER , expr , RPAREN , expr , expr , RPAREN ;
              (* (if-let (value expr) then-expr else-expr) *)

(* --- Циклы --- *)

while_expr    = LPAREN , "while" , expr , body , RPAREN ;
              (* (while (< i 10) (print i) (set! i (+ i 1))) *)

for_expr      = LPAREN , "for" , IDENTIFIER , "in" , expr , body , RPAREN ;
              (* (for i in (range 0 10) (print i)) *)

loop_expr     = LPAREN , "loop" , body , RPAREN ;
              (* (loop (print "tick") (sleep 1000)) *)

(* --- Модули и импорт --- *)

import        = LPAREN , "import" , import_path , [ import_modifier ] , RPAREN ;
import_path   = LPAREN , IDENTIFIER , IDENTIFIER , RPAREN ;
              (* (senko math) — пакет senko, модуль math *)
import_modifier = ":as" , IDENTIFIER
                | ":all" ;

defstruct     = LPAREN , "defstruct" , defstruct_header , { struct_field } , RPAREN ;
defstruct_header = IDENTIFIER
                   | LPAREN , IDENTIFIER , { TYPE_NAME } , RPAREN ;
              (* (defstruct point (x int) (y int)) *)
              (* (defstruct (Node a) (value a) (left (Option (Node a)))) *)

struct_field  = LPAREN , IDENTIFIER , type_expr , RPAREN ;

(* --- Тесты (только на верхнем уровне) --- *)

test          = LPAREN , "test" , STRING , body , RPAREN ;
              (* (test "name" (assert ...)) *)

assert_expr   = LPAREN , "assert" , expr , RPAREN ;
              (* (assert (= 1 1)) *)

(* --- Директивы --- *)

directive     = LPAREN , "#lupus" , IDENTIFIER , { IDENTIFIER | STRING } , RPAREN ;
              (* (#lupus enable-check types), (#lupus ffi python "module.path") *)

(* --- Локальные определения (только внутри body) --- *)

local_define  = LPAREN , "define" , IDENTIFIER , expr , RPAREN
              | LPAREN , "define-mutable" , IDENTIFIER , expr , RPAREN ;
              (* Локальные переменные внутри функций/веток. Синтаксис идентичен toplevel. *)

(* --- Выражения --- *)

expr          = literal
              | IDENTIFIER
              | qualified_name
              | if_expr
              | cond_expr
              | match_expr
              | if_let
              | while_expr
              | for_expr
              | loop_expr
              | set_bang
              | local_define
              | assert_expr
              | call_expr ;
              (* Все управляющие конструкции и локальные define являются выражениями *)

call_expr     = LPAREN , operator , { expr } , RPAREN
              | LPAREN , expr , { expr } , RPAREN ;
              (* Вызов функции: (func args...) или (+ 1 2) *)

literal       = INTEGER | FLOAT | BOOLEAN | STRING | LITERAL_NONE | LITERAL_UNIT ;
              (* none и unit — отдельные токены, не IDENTIFIER *)

qualified_name = IDENTIFIER , SLASH , IDENTIFIER ;
              (* math/sqrt, net/tcp-connect *)
              (* Доступны только для импортированных модулей. *)

operator      = "+" | "-" | "*" | "/" | "%" | "=" | "!=" | "<" | ">" | "<=" | ">="
              | "and" | "or" | "not"
              | "list" | "tuple" | "map" | "range"
              | "string-append" | "int->str" | "float->str" | "str->int" | "str->float"
              | "string-split" | "string-length"
              | "some" | "success" | "failure"
              | "cons" | "head" | "tail" | "length" | "nth" | "empty?"
              | "list-map" | "list-filter" | "list-fold"
              | "map-get" | "map-set" | "map-keys" | "map-values" | "map-has?"
              | "tensor" | "tensor-shape" | "tensor-add" | "tensor-mul" | "tensor-transpose"
              | IDENTIFIER ; (* любая пользовательская функция или конструктор *)

(* --- Типы --- *)

type_expr     = "int" | "float" | "bool" | "str" | "unit"
              | IDENTIFIER    (* Пользовательские типы: point, person, socket, task, channel *)
              | TYPE_NAME     (* Параметры дженериков: a, b, T *)
              | LPAREN , "List" , type_expr , RPAREN
              | LPAREN , "Tuple" , { type_expr } , RPAREN
              | LPAREN , "Map" , type_expr , type_expr , RPAREN
              | LPAREN , "Tensor" , RPAREN
              | LPAREN , "Func" , { type_expr } , type_expr , RPAREN
              | LPAREN , "Option" , type_expr , RPAREN
              | LPAREN , "Result" , type_expr , type_expr , RPAREN
              | LPAREN , IDENTIFIER , { type_expr } , RPAREN ;
              (* Пользовательские дженерики: (Node a), (Tree a b) *)
```

### 1.3. Примечания к грамматике

- **Префиксная нотация:** Все операторы и вызовы функций — строго префиксные. Нет инфиксного синтаксиса. AST однозначен.
- **Отступы:** Не имеют синтаксического значения. Используются только для читаемости.
- **Квалифицированные имена:** Доступны только для импортированных модулей. Формат: `<module-alias>/<identifier>`. Символ `/` не может входить в простой IDENTIFIER.
- **Body (последовательность):** Тело функции, ветки `if`, `match`, циклов — это последовательность из 1+ выражений. Все выражения в body вычисляются последовательно, результатом является значение последнего выражения. Все выражения кроме последнего должны иметь тип `unit` (проверяется тайп-чекером).
- **Локальные define:** `define` и `define-mutable` могут использоваться как на верхнем уровне, так и внутри любого `body`. Область видимости — от точки определения до конца текущего `body`.
- **Вариативные функции:** Только встроенные конструкторы `list`, `tuple`, `map`, `string-append` и `+` (для 2+ аргументов) принимают переменное число аргументов. Все пользовательские функции имеют фиксированную арность, определённую при объявлении.
- **Тесты:** Форма `test` допустима только на верхнем уровне (`toplevel`), не внутри функций.
- **Дженерики в defstruct:** `(defstruct (Node a) ...)` создаёт параметризованный тип. При использовании: `(Node int)`.

---

## 2. Встроенные типы и операции

### 2.1. Скалярные типы

| Тип | Литерал | Операции | Примечания |
|-----|---------|----------|------------|
| `int` | `42`, `-7` | `+`, `-`, `*`, `/` (целочисленное деление), `%` (остаток), `=`, `!=`, `<`, `>`, `<=`, `>=` | 64-bit signed integer. Деление на 0 — runtime panic (`error: divide-by-zero`). |
| `float` | `3.14`, `-0.5` | `+`, `-`, `*`, `/`, `=`, `!=`, `<`, `>`, `<=`, `>=` | 64-bit IEEE 754. Деление на 0 — `inf` / `-inf` (IEEE поведение), не ошибка. |
| `bool` | `true`, `false` | `and`, `or`, `not`, `=` | Короткое замыкание (`and`/`or`) обязательно. |
| `str` | `"hello"` | `string-append` (конкатенация), `=`, `!=`, `<` (лексикографическое), `int->str`, `str->int`, `float->str`, `str->float`, `string-split`, `string-length` | UTF-8. Неизменяемый. |
| `unit` | `unit` | Нет операций | Тип с единственным значением `unit`. Используется для функций с side effects. |

### 2.2. Составные типы

#### `List` — однородный список

```lupus
(List int)        ;; тип
(list 1 2 3)      ;; конструктор
```

| Операция | Сигнатура | Описание |
|----------|-----------|----------|
| `list` | `(Func a ... (List a))` | Конструктор. Принимает 0+ элементов одного типа. |
| `cons` | `(Func a (List a) (List a))` | Добавляет элемент в голову. |
| `head` | `(Func (List a) (Option a))` | Первый элемент или `none`. |
| `tail` | `(Func (List a) (Option (List a)))` | Список без головы или `none`. |
| `length` | `(Func (List a) int)` | Количество элементов. |
| `nth` | `(Func (List a) int (Option a))` | Элемент по индексу (0-based) или `none`. |
| `empty?` | `(Func (List a) bool)` | Проверка на пустоту. |
| `list-map` | `(Func (List a) (Func a b) (List b))` | Трансформация каждого элемента. |
| `list-filter` | `(Func (List a) (Func a bool) (List a))` | Фильтрация. |
| `list-fold` | `(Func (List a) b (Func b a b) b)` | Свёртка (левосторонняя). `(list-fold xs init (lambda (acc x) ...))` |

#### `Tuple` — разнородный кортеж фиксированной длины

```lupus
(Tuple int str bool)   ;; тип
(tuple 1 "a" true)     ;; конструктор
```

**Доступ к элементам:** В v0.1 доступ к элементам кортежа осуществляется **исключительно через паттерн-матчинг** (включая паттерн `(Tuple a b c)`). Динамическая индексация (`tuple-nth`) не поддерживается, так как статическая типизация не может вывести тип элемента по динамическому индексу.

```lupus
(define t (tuple 1 "hello" true))
(match t
  ((Tuple a b c) (print b)))   ;; b имеет тип str
```

| Операция | Сигнатура | Описание |
|----------|-----------|----------|
| `tuple` | `(Func a b ... (Tuple a b ...))` | Конструктор. Арность фиксируется типом. |

#### `Map` — ассоциативный массив

```lupus
(Map str int)          ;; тип
(map (tuple "a" 1) (tuple "b" 2))  ;; конструктор из списка пар
```

| Операция | Сигнатура | Описание |
|----------|-----------|----------|
| `map` | `(Func (Tuple k v) ... (Map k v))` | Конструктор из пар (tuple key value). |
| `map-get` | `(Func (Map k v) k (Option v))` | Получение значения по ключу. |
| `map-set` | `(Func (Map k v) k v (Map k v))` | Возвращает новый Map (неизменяемость). |
| `map-keys` | `(Func (Map k v) (List k))` | Список ключей. |
| `map-values` | `(Func (Map k v) (List v))` | Список значений. |
| `map-has?` | `(Func (Map k v) k bool)` | Проверка наличия ключа. |

#### `Tensor` — многомерный массив (для ML)

```lupus
(Tensor)               ;; тип без параметров в v0.1
```

| Операция | Сигнатура | Описание |
|----------|-----------|----------|
| `tensor` | `(Func (List int) (List float) Tensor)` | Создание из shape и flat data. |
| `tensor-shape` | `(Func Tensor (List int))` | Размерности. |
| `tensor-add` | `(Func Tensor Tensor (Result Tensor str))` | Поэлементное сложение (проверка shape). |
| `tensor-mul` | `(Func Tensor Tensor (Result Tensor str))` | Матричное умножение. |
| `tensor-transpose` | `(Func Tensor Tensor)` | Транспонирование. |

#### `Func` — функциональный тип

```lupus
(Func int int int)     ;; (int, int) -> int
```

Функции являются first-class values. Замыкания (closures) поддерживаются через лексическое окружение.

### 2.3. Алгебраические типы

#### `Option` — наличие/отсутствие значения

```lupus
(Option int)           ;; тип
none                   ;; конструктор для любого Option
(some 42)              ;; конструктор с значением
```

| Конструктор | Содержимое | Паттерн |
|-------------|------------|---------|
| `none` | ничего | `(none)` |
| `some` | 1 значение типа `a` | `((some value))` |

#### `Result` — успех или ошибка

```lupus
(Result int str)       ;; успех: int, ошибка: str
(success 42)           ;; конструктор успеха
(failure "error msg")  ;; конструктор ошибки
```

| Конструктор | Содержимое | Паттерн |
|-------------|------------|---------|
| `success` | 1 значение типа `a` | `((success value))` |
| `failure` | 1 значение типа `e` | `((failure err))` |

**Правило:** Функции, которые могут "провалиться", должны возвращать `Option` или `Result`. Использование `Result` обязательно, если нужно передать информацию об ошибке. Использование `Option` — если достаточно факта отсутствия.

### 2.4. Пользовательские типы и непрозрачные типы

**Пользовательские типы (структуры):** При определении `(defstruct point (x int) (y int))` имя `point` автоматически становится допустимым в `type_expr`. Конструктор `(point 10 20)` и аксессоры `(point-x p)`, `(point-y p)` генерируются автоматически.

**Пользовательские дженерики:**
```lupus
(defstruct (Node a)
  (value a)
  (left (Option (Node a)))
  (right (Option (Node a))))

(define tree (Node 42 (some (Node 10 none none)) none))
```

**Непрозрачные типы (opaque):** Типы вроде `socket`, `task`, `channel`, объявленные в FFI-модулях, также доступны в `type_expr` через `IDENTIFIER`. Их внутреннее устройство скрыто, операции с ними возможны только через функции модуля.

---

## 3. Стандартная библиотека (API)

### 3.1. Модуль `core` — автоимпорт

Все функции `core` доступны без префикса. Нельзя переопределить имя `core`-функции в пользовательском коде (ошибка линтера `core-shadowing`).

| Функция | Сигнатура | Описание | Пример |
|---------|-----------|----------|--------|
| `+` | `(Func int int int)` или `(Func float float float)` | Сложение. Нельзя смешивать int и float. | `(+ 2 3)` → `5` |
| `-` | `(Func int int int)` или `(Func float float float)` | Вычитание. | `(- 5 2)` → `3` |
| `*` | `(Func int int int)` или `(Func float float float)` | Умножение. | `(* 3 4)` → `12` |
| `/` | `(Func int int int)` | Целочисленное деление. | `(/ 7 2)` → `3` |
| `/` | `(Func float float float)` | Деление float. | `(/ 7.0 2.0)` → `3.5` |
| `%` | `(Func int int int)` | Остаток от деления. | `(% 7 2)` → `1` |
| `=` | `(Func a a bool)` | Равенство. Работает для всех типов, кроме `Func`. | `(= 1 1)` → `true` |
| `!=` | `(Func a a bool)` | Неравенство. | `(!= 1 2)` → `true` |
| `<` | `(Func int int bool)` или `(Func float float bool)` или `(Func str str bool)` | Меньше. | `(< 1 2)` → `true` |
| `>` | аналогично `<` | Больше. | `(> 2 1)` → `true` |
| `<=` | аналогично `<` | Меньше или равно. | `(<= 2 2)` → `true` |
| `>=` | аналогично `<` | Больше или равно. | `(>= 2 2)` → `true` |
| `and` | `(Func bool bool bool)` | Логическое И (короткое замыкание). | `(and true false)` → `false` |
| `or` | `(Func bool bool bool)` | Логическое ИЛИ (короткое замыкание). | `(or true false)` → `true` |
| `not` | `(Func bool bool)` | Логическое НЕ. | `(not true)` → `false` |
| `print` | `(Func str unit)` | Вывод строки в stdout с newline. | `(print "hello")` |
| `string-append` | `(Func str ... str)` | Конкатенация 1+ строк. | `(string-append "a" "b")` → `"ab"` |
| `string-length` | `(Func str int)` | Длина строки в Unicode code points. | `(string-length "abc")` → `3` |
| `string-split` | `(Func str str (List str))` | Разделение строки по разделителю. | `(string-split "a,b,c" ",")` → `(list "a" "b" "c")` |
| `int->str` | `(Func int str)` | Преобразование int → str. | `(int->str 42)` → `"42"` |
| `float->str` | `(Func float str)` | Преобразование float → str. | `(float->str 3.14)` → `"3.14"` |
| `str->int` | `(Func str (Option int))` | Парсинг int. | `(str->int "42")` → `(some 42)` |
| `str->float` | `(Func str (Option float))` | Парсинг float. | `(str->float "3.14")` → `(some 3.14)` |
| `list` | `(Func a ... (List a))` | Конструктор списка. | `(list 1 2 3)` |
| `tuple` | `(Func a b ... (Tuple a b ...))` | Конструктор кортежа. | `(tuple 1 "a")` |
| `map` | `(Func (Tuple k v) ... (Map k v))` | Конструктор Map. | `(map (tuple "a" 1))` |
| `range` | `(Func int int (List int))` | Диапазон [start, end). | `(range 0 3)` → `(list 0 1 2)` |
| `assert` | `(Func bool unit)` | Проверка условия. Если `false` — runtime panic. | `(assert (= 1 1))` |
| `sleep` | `(Func int unit)` | Засыпание на N миллисекунд (синхронное). | `(sleep 1000)` |
| `exit` | `(Func int unit)` | Завершение программы с кодом. | `(exit 0)` |
| `cons` | `(Func a (List a) (List a))` | Добавление в голову списка. | `(cons 0 (list 1 2))` → `(list 0 1 2)` |
| `head` | `(Func (List a) (Option a))` | Первый элемент. | `(head (list 1 2))` → `(some 1)` |
| `tail` | `(Func (List a) (Option (List a)))` | Хвост списка. | `(tail (list 1 2))` → `(some (list 2))` |
| `length` | `(Func (List a) int)` | Длина списка. | `(length (list 1 2 3))` → `3` |
| `nth` | `(Func (List a) int (Option a))` | Элемент по индексу. | `(nth (list 1 2) 0)` → `(some 1)` |
| `empty?` | `(Func (List a) bool)` | Проверка на пустоту. | `(empty? (list))` → `true` |
| `list-map` | `(Func (List a) (Func a b) (List b))` | Трансформация. | `(list-map (list 1 2) (lambda (x) (* x 2)))` |
| `list-filter` | `(Func (List a) (Func a bool) (List a))` | Фильтрация. | `(list-filter (list 1 2 3) (lambda (x) (> x 1)))` |
| `list-fold` | `(Func (List a) b (Func b a b) b)` | Свёртка. | `(list-fold (list 1 2 3) 0 (lambda (acc x) (+ acc x)))` → `6` |
| `map-get` | `(Func (Map k v) k (Option v))` | Получение из Map. | `(map-get m "key")` |
| `map-set` | `(Func (Map k v) k v (Map k v))` | Запись в Map. | `(map-set m "key" 42)` |
| `map-keys` | `(Func (Map k v) (List k))` | Ключи Map. | `(map-keys m)` |
| `map-values` | `(Func (Map k v) (List v))` | Значения Map. | `(map-values m)` |
| `map-has?` | `(Func (Map k v) k bool)` | Проверка ключа. | `(map-has? m "key")` |
| `tensor` | `(Func (List int) (List float) Tensor)` | Создание тензора. | `(tensor (list 2 3) (list 1.0 2.0 ...))` |
| `tensor-shape` | `(Func Tensor (List int))` | Shape тензора. | `(tensor-shape t)` |
| `tensor-add` | `(Func Tensor Tensor (Result Tensor str))` | Сложение тензоров. | `(tensor-add t1 t2)` |
| `tensor-mul` | `(Func Tensor Tensor (Result Tensor str))` | Умножение тензоров. | `(tensor-mul t1 t2)` |
| `tensor-transpose` | `(Func Tensor Tensor)` | Транспонирование. | `(tensor-transpose t)` |

### 3.2. Модуль `senko` (math)

Префикс: `math/`. Импорт: `(import (senko math))`.

| Функция | Сигнатура | Описание | Пример |
|---------|-----------|----------|--------|
| `math/pi` | `float` | Константа π. | `math/pi` → `3.1415926535...` |
| `math/e` | `float` | Константа e. | `math/e` → `2.7182818284...` |
| `math/sqrt` | `(Func float float)` | Квадратный корень. | `(math/sqrt 25.0)` → `5.0` |
| `math/sin` | `(Func float float)` | Синус (радианы). | `(math/sin math/pi)` → `0.0` |
| `math/cos` | `(Func float float)` | Косинус (радианы). | `(math/cos 0.0)` → `1.0` |
| `math/tan` | `(Func float float)` | Тангенс. | `(math/tan 0.0)` → `0.0` |
| `math/log` | `(Func float float)` | Натуральный логарифм. | `(math/log math/e)` → `1.0` |
| `math/log10` | `(Func float float)` | Десятичный логарифм. | `(math/log10 100.0)` → `2.0` |
| `math/pow` | `(Func float float float)` | Возведение в степень. | `(math/pow 2.0 3.0)` → `8.0` |
| `math/abs` | `(Func float float)` | Модуль. | `(math/abs -3.0)` → `3.0` |
| `math/floor` | `(Func float float)` | Округление вниз. | `(math/floor 3.7)` → `3.0` |
| `math/ceil` | `(Func float float)` | Округление вверх. | `(math/ceil 3.2)` → `4.0` |
| `math/round` | `(Func float float)` | Округление до ближайшего. | `(math/round 3.5)` → `4.0` |
| `math/min` | `(Func float float float)` | Минимум из двух. | `(math/min 1.0 2.0)` → `1.0` |
| `math/max` | `(Func float float float)` | Максимум из двух. | `(math/max 1.0 2.0)` → `2.0` |

### 3.3. Модуль `texas` (net)

Префикс: `net/`. Импорт: `(import (texas net))` или `(import (texas net) :as net)`.

| Функция | Сигнатура | Описание | Пример |
|---------|-----------|----------|--------|
| `net/tcp-connect` | `(Func str int (Result socket str))` | TCP-соединение. host, port. | `(net/tcp-connect "example.com" 80)` |
| `net/tcp-listen` | `(Func int (Result socket str))` | TCP-сервер, слушать порт. | `(net/tcp-listen 8080)` |
| `net/accept` | `(Func socket (Result socket str))` | Принять соединение (сервер). | `(net/accept sock)` |
| `net/send` | `(Func socket str (Result int str))` | Отправить строку. Возвращает количество байт. | `(net/send sock "GET / HTTP/1.0\r\n")` |
| `net/recv` | `(Func socket int (Result str str))` | Получить до N байт. | `(net/recv sock 4096)` |
| `net/recv-line` | `(Func socket (Result str str))` | Получить строку до `\n`. | `(net/recv-line sock)` |
| `net/close` | `(Func socket unit)` | Закрыть сокет. | `(net/close sock)` |
| `net/udp-bind` | `(Func int (Result socket str))` | UDP-сокет. | `(net/udp-bind 53)` |
| `net/udp-send-to` | `(Func socket str int str (Result int str))` | UDP отправка. | `(net/udp-send-to sock "host" 53 "data")` |
| `net/udp-recv-from` | `(Func socket int (Result (Tuple str int str) str))` | UDP получение. Возвращает `(host, port, data)`. | `(net/udp-recv-from sock 1024)` |

**Тип `socket`:** Opaque type. Нельзя создать напрямую из Lupus, только через функции `texas`.

### 3.4. Модуль `kaltsit` (file)

Префикс: `file/`. Импорт: `(import (kaltsit file))` или `(import (kaltsit file) :as file)`.

| Функция | Сигнатура | Описание | Пример |
|---------|-----------|----------|--------|
| `file/read-file` | `(Func str (Result str str))` | Чтение всего файла в строку. | `(file/read-file "data.txt")` |
| `file/write-file` | `(Func str str (Result unit str))` | Запись строки в файл (перезапись). | `(file/write-file "out.txt" "hello")` |
| `file/append-file` | `(Func str str (Result unit str))` | Дозапись в файл. | `(file/append-file "log.txt" "line\n")` |
| `file/exists?` | `(Func str bool)` | Проверка существования. | `(file/exists? "data.txt")` |
| `file/is-dir?` | `(Func str bool)` | Является ли директорией. | `(file/is-dir? "/tmp")` |
| `file/list-dir` | `(Func str (Result (List str) str))` | Список файлов в директории. | `(file/list-dir "/tmp")` |
| `file/mkdir` | `(Func str (Result unit str))` | Создать директорию. | `(file/mkdir "newdir")` |
| `file/delete` | `(Func str (Result unit str))` | Удалить файл. | `(file/delete "old.txt")` |
| `file/size` | `(Func str (Result int str))` | Размер файла в байтах. | `(file/size "data.txt")` |

### 3.5. Модуль `amiya` (async)

Префикс: `async/`. Импорт: `(import (amiya async))` или `(import (amiya async) :as async)`.

| Функция | Сигнатура | Описание | Пример |
|---------|-----------|----------|--------|
| `async/spawn` | `(Func (Func unit) task)` | Запускает функцию в отдельном потоке/задаче. **Функция-аргумент должна возвращать `unit`.** | `(async/spawn (lambda () (print "hi")))` |
| `async/sleep` | `(Func int unit)` | Асинхронный сон (не блокирует другие задачи). | `(async/sleep 1000)` |
| `async/channel` | `(Func (channel a))` | Создать канал (типизированный). | `(async/channel)` |
| `async/send` | `(Func (channel a) a unit)` | Отправить в канал. | `(async/send ch 42)` |
| `async/recv` | `(Func (channel a) a)` | **Блокирующее** получение из канала. | `(async/recv ch)` |
| `async/recv-timeout` | `(Func (channel a) int (Option a))` | Получение с таймаутом (мс). `none` если таймаут. | `(async/recv-timeout ch 1000)` |
| `async/wait` | `(Func task unit)` | Дождаться завершения задачи. | `(async/wait t)` |
| `async/wait-all` | `(Func (List task) unit)` | Дождаться всех задач. | `(async/wait-all (list t1 t2))` |

**Типы `task` и `channel`:** Opaque types. Создаются только через `amiya`.

**Семантика:** В Python-прототипе `spawn` использует `threading.Thread`. Каналы реализованы через `queue.Queue`. `async/recv` блокирует поток до появления данных (не busy-wait). `async/recv-timeout` использует `queue.get(timeout=ms/1000)`.

### 3.6. Модуль `w` (test)

Форма `test` является **специальной формой** языка (встроенной в парсер), а не функцией. Импорт модуля `w` не требуется для использования `(test ...)`. Однако функции `test/assert-eq`, `test/assert-true`, `test/assert-false` доступны после `(import (w test))` или через префикс `test/`.

| Функция | Сигнатура | Описание | Пример |
|---------|-----------|----------|--------|
| `test` | Специальная форма | Объявление теста (см. раздел 5). | `(test "name" (assert ...))` |
| `test/assert-eq` | `(Func a a unit)` | Assert равенства с выводом diff. | `(test/assert-eq 2 (+ 1 1))` |
| `test/assert-true` | `(Func bool unit)` | Assert true. | `(test/assert-true (> 2 1))` |
| `test/assert-false` | `(Func bool unit)` | Assert false. | `(test/assert-false (= 1 2))` |
| `test/run` | `(Func (List str) int)` | Запуск тестов по именам. | `(test/run (list "add-works"))` |
| `test/run-all` | `(Func int)` | Запуск всех тестов в файле. | `(test/run-all)` |

---

## 4. Спецификация FFI

### 4.1. Общие принципы

- FFI позволяет реализовать модули стандартной библиотеки на языке хоста (Python в v0.1).
- Lupus-код не может напрямую вызывать Python. Вместо этого Python-функции **регистрируются** как символы Lupus через механизм FFI.
- FFI-директива является **декларативной**: она указывает интерпретатору, где найти реализацию.
- Директива должна находиться **в начале файла модуля** (до любых `define` или `import`).
- **Все вызовы Python-функций через FFI оборачиваются в `try...except`** (см. раздел 4.5). Необработанные Python-исключения не должны приводить к краху интерпретатора Lupus.

### 4.2. Директива FFI

```lupus
(#lupus ffi python "<module.path>")
```

- `python` — язык хоста (в v0.1 только `python`).
- `<module.path>` — Python-импортный путь (например, `lupus_modules.senko`).
- Если файл является **пользовательским скриптом**, а не модулем, FFI-директива запрещена (ошибка линтера `invalid-directive`).

### 4.3. Требования к Python-модулю

Python-модуль должен экспортировать словарь `__lupus_exports__`:

```python
# lupus_modules/senko.py
import math

__lupus_exports__ = {
    "pi": ("float", lambda: math.pi),
    "e": ("float", lambda: math.e),
    "sqrt": ("(Func float float)", lambda x: math.sqrt(x)),
    "sin": ("(Func float float)", math.sin),
    "cos": ("(Func float float)", math.cos),
    "log": ("(Func float float)", math.log),
    "pow": ("(Func float float float)", lambda x, y: math.pow(x, y)),
}
```

Формат записи: `{"имя_в_lupus": ("строка_типа", callable)}`

**Требования к callable:**
1. Принимает столько аргументов, сколько указано в типе.
2. Возвращает значение, соответствующее типу.
3. Если функция возвращает `Result`, она должна возвращать кортеж `("success", value)` или `("failure", error_msg)`.
4. Если функция возвращает `Option`, она должна возвращать `None` (конвертируется в `none`) или произвольное значение (конвертируется в `(some value)`).
5. Для констант (вроде `pi`) callable — это thunk (функция без аргументов), вызываемая один раз при загрузке модуля.

### 4.4. Процесс загрузки FFI

1. Интерпретатор встречает `(import (senko math))`.
2. Разрешает путь модуля (см. раздел 4.6).
3. Если найдена директива `(#lupus ffi python "lupus_modules.senko")`, выполняет `importlib.import_module("lupus_modules.senko")`.
4. Читает `__lupus_exports__`.
5. Для каждого ключа создаёт внутренний Lupus-объект `Func` с:
   - `name`: `math/<key>`
   - `type`: распарсенный тип из строки
   - `impl`: Python-callable (обёрнутый в адаптер конвертации типов)
6. Регистрирует в окружении модуля `math`.

### 4.5. Обработка ошибок в FFI

Все вызовы Python-функций через FFI **обязаны** быть обёрнуты в `try...except`:

```python
def wrap_python_func(func, lupus_type):
    def wrapper(args):
        try:
            py_args = [value_to_python(a) for a in args]
            result = func(*py_args)
            return python_to_value(result, lupus_type)
        except ZeroDivisionError as e:
            return make_runtime_error("ffi-runtime-error", str(e))
        except TypeError as e:
            return make_runtime_error("ffi-type-mismatch", str(e))
        except Exception as e:
            return make_runtime_error("ffi-runtime-error", f"{type(e).__name__}: {e}")
    return wrapper
```

Если Python-функция "упала" с необработанным исключением:
- Интерпретатор Lupus **не должен** падать.
- Должна быть сгенерирована JSON-ошибка с кодом `ffi-runtime-error`.
- Для функций, возвращающих `Result`, исключение может быть транслировано в `(failure "...")`.
- Для функций, не возвращающих `Result`/`Option`, генерируется runtime panic.

### 4.6. Разрешение имён модулей

При `(import (senko math))`:
1. Интерпретатор ищет файл `stdlib/senko/math.lupus` (или `senko/math.lupus` в `LUPUS_PATH`).
2. Если файл найден, парсит его как модуль Lupus.
3. Если в файле есть FFI-директива, загружает Python-реализацию.
4. Все `define-public` из файла и все экспорты FFI становятся доступными под префиксом `math/`.

### 4.7. Пример полного биндинга модуля `senko`

**Файл модуля Lupus:** `stdlib/senko/math.lupus`
```lupus
;; stdlib/senko/math.lupus
(#lupus ffi python "lupus_modules.senko")

;; Дополнительные чистые функции на Lupus можно добавить здесь:
(define-public (deg->rad deg) -> float
  (* math/pi (/ deg 180.0)))
```

**Файл Python-реализации:** `lupus_modules/senko.py`
```python
import math

def _ensure_float(x):
    if not isinstance(x, (int, float)):
        raise TypeError(f"Expected float, got {type(x)}")
    return float(x)

__lupus_exports__ = {
    "pi": ("float", lambda: math.pi),
    "e": ("float", lambda: math.e),
    "sqrt": ("(Func float float)", lambda x: math.sqrt(_ensure_float(x))),
    "sin": ("(Func float float)", lambda x: math.sin(_ensure_float(x))),
    "cos": ("(Func float float)", lambda x: math.cos(_ensure_float(x))),
    "tan": ("(Func float float)", lambda x: math.tan(_ensure_float(x))),
    "log": ("(Func float float)", lambda x: math.log(_ensure_float(x))),
    "log10": ("(Func float float)", lambda x: math.log10(_ensure_float(x))),
    "pow": ("(Func float float float)", lambda x, y: math.pow(_ensure_float(x), _ensure_float(y))),
    "abs": ("(Func float float)", lambda x: abs(_ensure_float(x))),
    "floor": ("(Func float float)", lambda x: math.floor(_ensure_float(x))),
    "ceil": ("(Func float float)", lambda x: math.ceil(_ensure_float(x))),
    "round": ("(Func float float)", lambda x: round(_ensure_float(x))),
    "min": ("(Func float float float)", lambda x, y: min(_ensure_float(x), _ensure_float(y))),
    "max": ("(Func float float float)", lambda x, y: max(_ensure_float(x), _ensure_float(y))),
}
```

### 4.8. Ошибки FFI

| Код ошибки | Условие |
|------------|---------|
| `ffi-module-not-found` | Python-модуль не найден |
| `ffi-export-missing` | Ключ в `__lupus_exports__` не найден |
| `ffi-type-mismatch` | Возвращаемое значение Python не соответствует декларированному типу |
| `ffi-arity-mismatch` | Python-функция приняла другое количество аргументов |
| `ffi-runtime-error` | Python-исключение при выполнении функции (ZeroDivisionError, и т.д.) |

---

## 5. Формат тестов

### 5.1. Объявление тестов

Тесты объявляются внутри `.lupus` файлов с помощью специальной формы `test`:

```lupus
(test "уникальное-имя-теста"
  expr1
  expr2
  ...)
```

- `"уникальное-имя-теста"` — строковый идентификатор. Должен быть уникальным в пределах файла. Дублирование — ошибка линтера `test-name-duplicate`.
- `expr1, expr2, ...` — тело теста (последовательность выражений). Обычно содержит `assert`.
- Тесты **не выполняются** при обычном запуске программы (`lupus run`). Они выполняются только при `lupus test <file>`.
- Тесты **не влияют** на вывод программы при обычном запуске.

### 5.2. Запуск тестов

```bash
lupus test file.lupus          # запуск всех тестов в файле
lupus test file.lupus --name "add-works"  # запуск конкретного теста
lupus test dir/                # рекурсивный запуск всех .lupus файлов
```

### 5.3. Вывод результатов

Интерпретатор выводит JSON Lines (JSONL):

```json
{"type": "test-start", "name": "add-works", "file": "calc.lupus"}
{"type": "test-pass", "name": "add-works", "file": "calc.lupus", "duration_ms": 0.5}
{"type": "test-fail", "name": "safe-divide-zero", "file": "calc.lupus", "error": {"code": "assert-failed", "location": {"line": 15, "col": 3}, "message": "Assertion failed: (= (safe-divide 10 0) none)"}, "duration_ms": 1.2}
{"type": "test-summary", "total": 5, "passed": 4, "failed": 1, "file": "calc.lupus"}
```

### 5.4. Правила тестов

1. **Изоляция:** Каждый тест запускается в **свежем окружении** (fresh environment). `define` и `define-mutable` из теста не видны другим тестам. `define-public` из основного кода файла видны всем тестам.
2. **Порядок:** Тесты выполняются в порядке объявления в файле.
3. **Падение:** Если `assert` проваливается, тест немедленно прерывается (fail-fast). Остальные тесты продолжают выполняться.
4. **Side effects:** `print` внутри теста подавляется по умолчанию (выводится только при флаге `--verbose`).

---

## 6. Формат ошибок (JSON)

### 6.1. Общая структура

Все ошибки, предупреждения и runtime panic выводятся в строгом JSON:

```json
{
  "severity": "error" | "warning" | "info",
  "phase": "lex" | "parse" | "type" | "lint" | "runtime" | "ffi",
  "code": "уникальный-код",
  "message": "Человекочитаемое описание",
  "location": {
    "file": "путь/к/файлу.lupus",
    "line": 12,
    "col": 5,
    "span": {"start": 120, "end": 135}
  },
  "hint": "Возможное исправление или пояснение",
  "context": {
    "line_text": "  (define x \"hello\")",
    "token": "x"
  }
}
```

### 6.2. Полный каталог ошибок

| Код | Фаза | Сообщение | Пример |
|-----|------|-----------|--------|
| `unknown-token` | lex | Неизвестный токен | `@` |
| `unclosed-string` | lex | Незакрытая строка | `"hello` |
| `unclosed-comment` | lex | Незакрытый блок-комментарий | `#| ...` |
| `unexpected-token` | parse | Неожиданный токен | `(define 1 2)` — 1 не IDENTIFIER |
| `missing-rparen` | parse | Пропущена закрывающая скобка | `(define x 10` |
| `type-mismatch` | type | Несоответствие типов | `(+ 1 "a")` |
| `unknown-identifier` | type | Неизвестный идентификатор | `(foo 1)` — foo не определён |
| `unknown-module` | type | Неизвестный модуль | `(import (unknown mod))` |
| `unknown-module-symbol` | type | Неизвестный символ модуля | `(math/unknown 1)` |
| `arity-mismatch` | type | Неверное количество аргументов | `(+ 1)` — ожидается 2 |
| `missing-return-type` | type | Отсутствует аннотация возвращаемого типа у public-функции | `(define-public (foo x) x)` |
| `missing-param-type` | type | Отсутствует аннотация типа параметра у public-функции | `(define-public (foo x) -> int x)` |
| `immutable-assignment` | type | Попытка set! неизменяемой переменной | `(set! x 1)` где x из `define` |
| `duplicate-definition` | lint | Дублирующееся определение | два `(define x ...)` |
| `unused-variable` | lint | Неиспользуемая переменная | `(define y 10)` без использования y |
| `core-shadowing` | lint | Переопределение core-функции | `(define + 1)` |
| `test-name-duplicate` | lint | Дублирующееся имя теста | два `(test "foo" ...)` |
| `test-in-function` | lint | Тест объявлен внутри функции | `(define (f) (test "x" ...))` |
| `divide-by-zero` | runtime | Деление на ноль | `(/ 1 0)` |
| `assert-failed` | runtime | Assert вернул false | `(assert false)` |
| `index-out-of-bounds` | runtime | Индекс вне диапазона | `(nth (list 1) 5)` |
| `match-non-exhaustive` | type | Неполный паттерн-матчинг | `(match (some 1) ((none) 0))` — пропущен `some` |
| `match-redundant` | lint | Избыточный паттерн | Паттерн после `else` или `_` |
| `invalid-directive` | lint | Недопустимая директива | `(#lupus unknown)` |
| `ffi-module-not-found` | ffi | Python-модуль не найден | `(#lupus ffi python "missing")` |
| `ffi-type-mismatch` | ffi | Несоответствие типов FFI | Python-функция вернула `str` вместо `float` |
| `ffi-arity-mismatch` | ffi | Неверная арность FFI | Python-функция приняла 3 вместо 2 |
| `ffi-runtime-error` | ffi | Исключение в Python-функции | Деление на ноль в Python-модуле |
| `value-restriction` | type | Нарушение ограничения значений | Полиморфный mutable без явной аннотации |

---

## 7. Сериализация AST (JSON)

### 7.1. Цель

AST должно сериализоваться в JSON для:
- Передачи между слоями (Parser → Typechecker → Interpreter).
- Сохранения в датасет для обучения LLM.
- Чтения Rust-версией на этапе 2.

### 7.2. Формат узла

Каждый узел AST — объект с обязательными полями:

```json
{
  "kind": "тип_узла",
  "span": {"file": "f.lupus", "start": 120, "end": 135, "line": 5, "col": 2},
  "data": { ... }
}
```

### 7.3. Типы узлов

| `kind` | Поля `data` | Описание |
|--------|-------------|----------|
| `Program` | `toplevels: [Node]` | Корень |
| `Define` | `name: str`, `mutable: bool`, `public: bool`, `constant: bool`, `value: Node`, `type_annotation: Type \| null` | Определение |
| `SetBang` | `name: str`, `value: Node` | Изменение mutable |
| `Lambda` | `params: [Param]`, `body: [Node]`, `return_type: Type \| null` | Анонимная функция |
| `Param` | `name: str`, `type_annotation: Type \| null` | Параметр функции |
| `If` | `condition: Node`, `then_branch: [Node]`, `else_branch: [Node]` | Условие |
| `Cond` | `clauses: [(condition: Node, body: [Node])]` | Множественное ветвление |
| `Match` | `expr: Node`, `clauses: [(pattern: Pattern, body: [Node])]` | Паттерн-матчинг |
| `PatternVar` | `name: str` | Паттерн-переменная |
| `PatternWildcard` | `{}` | Паттерн `_` |
| `PatternConstructor` | `constructor: str`, `args: [Pattern]` | Паттерн-конструктор |
| `PatternTuple` | `args: [Pattern]` | Паттерн кортежа `(Tuple a b c)` |
| `IfLet` | `binding: (name: str, expr: Node)`, `then_branch: [Node]`, `else_branch: [Node]` | If-let |
| `While` | `condition: Node`, `body: [Node]` | Цикл while |
| `For` | `var: str`, `iter: Node`, `body: [Node]` | Цикл for |
| `Loop` | `body: [Node]` | Бесконечный цикл |
| `Import` | `package: str`, `module: str`, `alias: str \| null`, `import_all: bool` | Импорт |
| `DefStruct` | `name: str`, `type_params: [str]`, `fields: [(name: str, type: Type)]` | Структура |
| `Test` | `name: str`, `body: [Node]` | Тест |
| `Assert` | `expr: Node` | Assert |
| `Directive` | `name: str`, `args: [str]` | Директива |
| `Call` | `func: Node`, `args: [Node]` | Вызов функции |
| `Identifier` | `name: str` | Идентификатор |
| `QualifiedName` | `module: str`, `name: str` | Квалифицированное имя |
| `LiteralInt` | `value: int` | Целое |
| `LiteralFloat` | `value: float` | Float |
| `LiteralBool` | `value: bool` | Bool |
| `LiteralStr` | `value: str` | Строка |
| `LiteralNone` | `{}` | none |
| `LiteralUnit` | `{}` | unit |
| `Type` | `kind: str`, `params: [Type]` | Типовое выражение |

### 7.4. Пример сериализации

**Исходный код:**
```lupus
(define (add a b) -> int
  (+ a b))

(define x 42)
```

**JSON AST:**
```json
{
  "kind": "Program",
  "span": {"file": "example.lupus", "start": 0, "end": 42, "line": 1, "col": 1},
  "data": {
    "toplevels": [
      {
        "kind": "Define",
        "span": {"file": "example.lupus", "start": 0, "end": 35, "line": 1, "col": 1},
        "data": {
          "name": "add",
          "mutable": false,
          "public": false,
          "constant": false,
          "value": {
            "kind": "Lambda",
            "span": {"file": "example.lupus", "start": 8, "end": 35, "line": 1, "col": 9},
            "data": {
              "params": [
                {"kind": "Param", "data": {"name": "a", "type_annotation": null}},
                {"kind": "Param", "data": {"name": "b", "type_annotation": null}}
              ],
              "body": [
                {
                  "kind": "Call",
                  "span": {"file": "example.lupus", "start": 28, "end": 34, "line": 2, "col": 3},
                  "data": {
                    "func": {"kind": "Identifier", "data": {"name": "+"}},
                    "args": [
                      {"kind": "Identifier", "data": {"name": "a"}},
                      {"kind": "Identifier", "data": {"name": "b"}}
                    ]
                  }
                }
              ],
              "return_type": {"kind": "Type", "data": {"kind": "int", "params": []}}
            }
          },
          "type_annotation": null
        }
      },
      {
        "kind": "Define",
        "span": {"file": "example.lupus", "start": 37, "end": 50, "line": 4, "col": 1},
        "data": {
          "name": "x",
          "mutable": false,
          "public": false,
          "constant": false,
          "value": {"kind": "LiteralInt", "data": {"value": 42}},
          "type_annotation": null
        }
      }
    ]
  }
}
```

### 7.5. Требования к сериализации

- **Deterministic:** Порядок полей в JSON фиксирован (kind, span, data).
- **Span обязателен:** Каждый узел содержит span для точной диагностики.
- **Type annotations:** Если тип не указан явно, `type_annotation` и `return_type` — `null`.
- **No comments:** Комментарии не включаются в AST (игнорируются на этапе лексинга).
- **Body как массив:** Все тела (функций, веток, циклов) сериализуются как массивы узлов `[Node]`.

---

## 8. Рекомендации по реализации (Python)

### 8.1. Архитектура слоёв

```
┌─────────────────────────────────────────┐
│  CLI (lupus run / lupus test / lupus check) │
├─────────────────────────────────────────┤
│  Frontend                               │
│  ├── Lexer (lark/ply или ручной)        │
│  ├── Parser (LALR/Earley)               │
│  └── AST Builder (JSON-serializable)    │
├─────────────────────────────────────────┤
│  Middle-end                             │
│  ├── Typechecker (Hindley-Milner subset)│
│  ├── Linter (style & naming rules)      │
│  └── Borrow-checker stub (v0.1: no-op)  │
├─────────────────────────────────────────┤
│  Backend                                │
│  ├── Interpreter (tree-walk)            │
│  └── FFI Bridge (Python adapter)        │
├─────────────────────────────────────────┤
│  Runtime                                │
│  ├── Value representations (Python obj) │
│  ├── Environment (lexical scopes)       │
│  └── GC (CPython refcount)              │
└─────────────────────────────────────────┘
```

### 8.2. Лексер

**Рекомендуемая библиотека:** `lark` (с грамматикой EBNF из раздела 1) или ручной лексер на `re`.

**Требования:**
- Поддержка Unicode для строк и идентификаторов.
- Отслеживание позиций (line, col, start, end) для каждого токена.
- Комментарии превращаются в токен `COMMENT` и отбрасываются парсером, или игнорируются лексером.
- `none` и `unit` лексируются как отдельные токены `LITERAL_NONE` и `LITERAL_UNIT`, а не как `IDENTIFIER` или `KEYWORD`.
- `true`/`false` лексируются как `BOOLEAN`.

**Пример ручного лексера (концепт):**
```python
import re

TOKEN_SPEC = [
    ('COMMENT_LINE',  r';;[^\n]*'),
    ('COMMENT_BLOCK', r'#\|[^|]*\|#'),
    ('FLOAT',         r'-?\d+\.\d+([eE][-+]?\d+)?'),
    ('INTEGER',       r'-?\d+'),
    ('STRING',        r'"([^"\\]|\\.)*"'),
    ('ARROW',         r'->'),
    ('LPAREN',        r'\('),
    ('RPAREN',        r'\)'),
    ('SLASH',         r'/'),
    ('LITERAL_UNIT',  r'\bunit\b'),
    ('LITERAL_NONE',  r'\bnone\b'),
    ('BOOLEAN',       r'\b(true|false)\b'),
    ('IDENTIFIER',    r'[a-zA-Z_][a-zA-Z0-9_\-]*'),
    ('SKIP',          r'[ \t\n]+'),
    ('MISMATCH',      r'.'),
]
```

### 8.3. Парсер

**Рекомендуемая библиотека:** `lark` с LALR-разбором.

**Почему lark:**
- Прямая поддержка EBNF.
- Генерация AST через `Transformer`.
- Хорошая диагностика ошибок.

**Альтернатива:** Ручной рекурсивный спуск (проще для контроля, но больше кода).

**Ключевые моменты парсера:**
- `body` парсится как последовательность `expr` до закрывающей скобки текущего уровня.
- `define-public` требует `func_header` (имя в скобках с параметрами) и `body`.
- Параметры в `define-public` могут быть с аннотацией `(a int)` или без (`a`). Если без — тип выводится, но тайп-чекер может выдать `warn`.
- `local_define` отличается от `toplevel define` только контекстом (парсер может использовать одинаковые правила).
- `test` допустим только на верхнем уровне.
- `defstruct` поддерживает дженерики: `(defstruct (Node a) ...)`.

### 8.4. Тайп-чекер

**Алгоритм:** Упрощённый Hindley-Milner с ограниченной полиморфией.

**Ключевые решения:**
- **Типовые переменные:** `a`, `b`, `t1`, `t2` — для вывода.
- **Унификация:** Стандартный алгоритм унификации с occurs check.
- **Окружение типов (Gamma):** Словарь `имя -> схема типа`.
- **Полиморфизм:** Let-polymorphism (обобщение типов при `define` и `define-const`).
- **Value Restriction:** Для `define-mutable` полиморфизм **запрещён**. Тип mutable-переменной не обобщается (monomorphic). Если тип не может быть выведен однозначно — требуется явная аннотация.
- **Ограничения:** Нет полиморфизма высшего порядка для пользовательских типов в v0.1.

**Псевдокод:**
```python
def infer(expr, env):
    if expr is LiteralInt: return Type("int")
    if expr is LiteralUnit: return Type("unit")
    if expr is LiteralNone: return Type("Option", [fresh_var()])
    if expr is Identifier: return env.lookup(expr.name)
    if expr is Call:
        func_type = infer(expr.func, env)
        arg_types = [infer(arg, env) for arg in expr.args]
        result_type = fresh_var()
        unify(func_type, Type("Func", arg_types + [result_type]))
        return result_type
    if expr is Lambda:
        new_env = env.extend({p.name: fresh_var() for p in expr.params})
        body_types = [infer(e, new_env) for e in expr.body]
        # Все выражения в body кроме последнего должны быть unit
        for t in body_types[:-1]:
            unify(t, Type("unit"))
        param_types = [new_env.lookup(p.name) for p in expr.params]
        return Type("Func", param_types + [body_types[-1]])
    if expr is Define:
        val_type = infer(expr.value, env)
        if expr.type_annotation:
            unify(val_type, parse_type(expr.type_annotation))
        if expr.mutable:
            # Value restriction: no generalization for mutable
            env = env.extend({expr.name: val_type})
        else:
            env = env.extend({expr.name: generalize(val_type, env)})
        return Type("unit")
    if expr is SetBang:
        # Проверяем, что переменная mutable и тип совпадает
        var_type = env.lookup(expr.name)
        val_type = infer(expr.value, env)
        unify(var_type, val_type)
        return Type("unit")
```

**Требования к ошибкам:**
- Если унификация не удалась — выдать `type-mismatch` с обеими сторонами унификации.
- Если `define-public` без полной аннотации — `missing-return-type` или `missing-param-type`.
- Если `set!` применён к `define` (immutable) — `immutable-assignment`.
- Если `define-mutable` с полиморфным типом без аннотации — `value-restriction`.

### 8.5. Интерпретатор

**Стратегия:** Tree-walk interpreter (обход AST).

**Окружение (Environment):**
- Иерархический словарь с родительской ссылкой.
- `define` создаёт запись в текущем окружении.
- `lambda` захватывает текущее окружение (closure).
- `set!` ищет переменную в текущем и родительских окружениях, изменяет первую найденную mutable.

**Значения (Value):**
```python
class Value:
    pass

class VInt(Value): ...
class VFloat(Value): ...
class VBool(Value): ...
class VStr(Value): ...
class VList(Value): ...
class VTuple(Value): ...
class VMap(Value): ...
class VUnit(Value): ...
class VNone(Value): ...
class VSome(Value): ...
class VSuccess(Value): ...
class VFailure(Value): ...
class VClosure(Value):  # env, params, body
    pass
class VOpaque(Value):   # для socket, task, channel
    pass
```

**Вызов функций:**
- Встроенные (core): Python-функция, принимает `List[Value]`, возвращает `Value`.
- Пользовательские: Создание нового окружения, связывание параметров, выполнение тела (последовательно, результат — последнее выражение).
- FFI: Адаптер, конвертирующий `Value` ↔ Python-типы.

**Body evaluation:**
```python
def eval_body(body, env):
    result = VUnit()
    for expr in body:
        result = eval_expr(expr, env)
    return result
```

### 8.6. FFI Bridge

```python
class FFIBridge:
    def __init__(self):
        self.modules = {}

    def load(self, module_path, lupus_module_name):
        py_module = importlib.import_module(module_path)
        exports = py_module.__lupus_exports__
        for name, (type_str, func) in exports.items():
            lupus_type = parse_type(type_str)
            wrapped = self.wrap_python_func(func, lupus_type)
            register_in_module(lupus_module_name, name, wrapped)

    def wrap_python_func(self, func, lupus_type):
        def wrapper(args):
            try:
                py_args = [value_to_python(a) for a in args]
                result = func(*py_args)
                return python_to_value(result, lupus_type)
            except Exception as e:
                return make_ffi_error("ffi-runtime-error", f"{type(e).__name__}: {e}")
        return wrapper

    def value_to_python(self, v):
        if isinstance(v, VInt): return v.value
        if isinstance(v, VFloat): return v.value
        if isinstance(v, VStr): return v.value
        if isinstance(v, VBool): return v.value
        if isinstance(v, VNone): return None
        if isinstance(v, VSome): return self.value_to_python(v.value)
        if isinstance(v, VSuccess): return ("success", self.value_to_python(v.value))
        if isinstance(v, VFailure): return ("failure", self.value_to_python(v.value))
        if isinstance(v, VList): return [self.value_to_python(x) for x in v.items]
        if isinstance(v, VTuple): return tuple(self.value_to_python(x) for x in v.items)
        if isinstance(v, VUnit): return None
        raise TypeError(f"Cannot convert {type(v)} to Python")

    def python_to_value(self, py_val, lupus_type):
        # Обратная конвертация с проверкой типа
        ...
```

### 8.7. Модуль `amiya` (async) — реализация

```python
import threading
import queue
import time

class Task:
    def __init__(self, func):
        self.thread = threading.Thread(target=func)
        self.thread.start()

    def wait(self):
        self.thread.join()

class Channel:
    def __init__(self):
        self.q = queue.Queue()

    def send(self, value):
        self.q.put(value)

    def recv(self):
        return self.q.get()  # блокирующее

    def recv_timeout(self, timeout_ms):
        try:
            return self.q.get(timeout=timeout_ms / 1000.0)
        except queue.Empty:
            return None
```

### 8.8. CLI

```bash
lupus run <file.lupus> [args...]     # выполнить программу
lupus test <file.lupus>               # запустить тесты
lupus check <file.lupus>              # линт + типы (без выполнения)
lupus ast <file.lupus>                # вывести AST в JSON
lupus eval <expr>                     # выполнить одно выражение (REPL-mode)
```

### 8.9. Зависимости (requirements.txt)

```
lark>=1.1.0
click>=8.0.0
```

---

## 9. Примеры программ

### 9.1. Калькулятор площади круга

```lupus
;; calculator.lupus
(import (senko math))

(define-const pi math/pi)

(define-public (circle-area (radius float)) -> float
  (* math/pi (* radius radius)))

(define-public (circle-circumference (radius float)) -> float
  (* 2.0 (* math/pi radius)))

(define r 10.0)
(print (string-append "Radius: " (float->str r)))
(print (string-append "Area: " (float->str (circle-area r))))
(print (string-append "Circumference: " (float->str (circle-circumference r))))

(test "circle-area-10"
  (assert (= (circle-area 10.0) 314.1592653589793)))

(test "circle-circumference-10"
  (assert (= (circle-circumference 10.0) 62.83185307179586)))
```

### 9.2. HTTP-клиент (GET-запрос)

```lupus
;; http_client.lupus
(import (texas net) :as net)
(import (kaltsit file) :as file)

(define-public (fetch (host str) (path str)) -> (Result str str)
  (match (net/tcp-connect host 80)
    ((success sock)
      (match (net/send sock (string-append "GET " path " HTTP/1.0\r\nHost: " host "\r\n\r\n"))
        ((success bytes-sent)
          (match (net/recv sock 8192)
            ((success response)
              (net/close sock)
              (success response))
            ((failure err)
              (net/close sock)
              (failure (string-append "recv failed: " err)))))
        ((failure err)
          (net/close sock)
          (failure (string-append "send failed: " err)))))
    ((failure err)
      (failure (string-append "connect failed: " err)))))

(match (fetch "example.com" "/")
  ((success data)
    (match (file/write-file "page.html" data)
      ((success _) (print "Saved to page.html"))
      ((failure err) (print (string-append "Write failed: " err)))))
  ((failure err)
    (print (string-append "Fetch failed: " err))))
```

### 9.3. Асинхронный таймер с каналами

```lupus
;; async_timer.lupus
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
  (define msg (async/recv ch))
  (print msg)
  (set! total (- total 1)))

(print "All ticks received")
```

### 9.4. Обработка CSV-файла

```lupus
;; csv_processor.lupus
(import (kaltsit file) :as file)

(define-public (parse-int-lines (content str)) -> (List int)
  (define lines (string-split content "\n"))
  (define parsed (list-map lines str->int))
  (define valid-nums (list-filter parsed (lambda (x) (not (= x none)))))
  (list-map valid-nums (lambda (opt) (match opt ((some v) v) ((none) 0)))))

(define-public (sum-lines (filename str)) -> (Result int str)
  (match (file/read-file filename)
    ((success content)
      (define nums (parse-int-lines content))
      (success (list-fold nums 0 (lambda (acc x) (+ acc x)))))
    ((failure err)
      (failure err))))

;; Запуск
(match (sum-lines "numbers.txt")
  ((success total) (print (string-append "Sum: " (int->str total))))
  ((failure err) (print (string-append "Error: " err))))

(test "parse-int-lines"
  (define result (parse-int-lines "1\n2\n3"))
  (assert (= (length result) 3)))
```

### 9.5. Модульное тестирование математики

```lupus
;; math_test.lupus
(import (senko math))

(define-public (factorial (n int)) -> int
  (if (<= n 1)
    1
    (* n (factorial (- n 1)))))

(define-public (fibonacci (n int)) -> int
  (if (<= n 1)
    n
    (+ (fibonacci (- n 1)) (fibonacci (- n 2)))))

;; --- Тесты ---
(test "factorial-0"
  (assert (= (factorial 0) 1)))

(test "factorial-5"
  (assert (= (factorial 5) 120)))

(test "fibonacci-10"
  (assert (= (fibonacci 10) 55)))

(test "math-sqrt-16"
  (assert (= (math/sqrt 16.0) 4.0)))

(test "math-sin-zero"
  (assert (= (math/sin 0.0) 0.0)))

(test "math-pow"
  (assert (= (math/pow 2.0 10.0) 1024.0)))

(test "match-some"
  (match (some 42)
    ((some x) (assert (= x 42)))
    ((none) (assert false))))

(test "match-none"
  (match none
    ((some _) (assert false))
    ((none) (assert true))))

(test "if-let-some"
  (if-let (v (some 7))
    (assert (= v 7))
    (assert false)))

(test "if-let-none"
  (if-let (v none)
    (assert false)
    (assert true)))
```

### 9.6. Структуры данных, Map и дженерики

```lupus
;; structs.lupus

(defstruct point
  (x float)
  (y float))

(defstruct person
  (name str)
  (age int))

(defstruct (Node a)
  (value a)
  (left (Option (Node a)))
  (right (Option (Node a))))

(define p (point 3.0 4.0))
(define alice (person "Alice" 30))
(define tree (Node 10 (some (Node 5 none none)) none))

(define dist (math/sqrt (+ (* (point-x p) (point-x p)) (* (point-y p) (point-y p)))))
(print (string-append "Distance: " (float->str dist)))

(define registry (map (tuple "alice" alice) (tuple "bob" (person "Bob" 25))))
(match (map-get registry "alice")
  ((some val)
    (print (string-append "Found: " (person-name val))))
  ((none)
    (print "Not found")))

(test "point-distance"
  (assert (= dist 5.0)))

(test "map-get-existing"
  (match (map-get registry "alice")
    ((some val) (assert (= (person-age val) 30)))
    ((none) (assert false))))

(test "map-get-missing"
  (match (map-get registry "charlie")
    ((some _) (assert false))
    ((none) (assert true))))

(test "generic-node"
  (match tree
    ((Node v left right)
      (assert (= v 10)))))
```

---

## 10. Чек-лист для GPT-5.5 Mimi

### 10.1. Файлы, которые должен сгенерировать код

| № | Файл | Описание | Критичность |
|---|------|----------|-------------|
| 1 | `lupus/lexer.py` | Токенизатор (ручной или на lark) | Обязательно |
| 2 | `lupus/parser.py` | Парсер, строящий AST (JSON-serializable) | Обязательно |
| 3 | `lupus/ast_nodes.py` | Классы узлов AST с методом `.to_json()` | Обязательно |
| 4 | `lupus/types.py` | Определения типов, унификация, среда типов | Обязательно |
| 5 | `lupus/typechecker.py` | Вывод типов (Hindley-Milner), проверка аннотаций | Обязательно |
| 6 | `lupus/linter.py` | Проверки стиля, неиспользуемых переменных, дублирования | Обязательно |
| 7 | `lupus/interpreter.py` | Tree-walk интерпретатор, окружения, замыкания | Обязательно |
| 8 | `lupus/values.py` | Представление runtime-значений | Обязательно |
| 9 | `lupus/environment.py` | Иерархические окружения (scope) | Обязательно |
| 10 | `lupus/ffi.py` | Загрузка Python-модулей, обёртка функций, обработка исключений | Обязательно |
| 11 | `lupus/errors.py` | Формирование JSON-ошибок | Обязательно |
| 12 | `lupus/cli.py` | Точка входа: `lupus run`, `lupus test`, `lupus check`, `lupus ast` | Обязательно |
| 13 | `lupus/core_builtins.py` | Реализация всех функций `core` (+, -, *, list, map, assert, ...) | Обязательно |
| 14 | `lupus_modules/senko.py` | FFI-реализация модуля math | Обязательно |
| 15 | `lupus_modules/texas.py` | FFI-реализация модуля net (через socket) | Обязательно |
| 16 | `lupus_modules/kaltsit.py` | FFI-реализация модуля file (через os, pathlib) | Обязательно |
| 17 | `lupus_modules/amiya.py` | FFI-реализация модуля async (через threading, queue) | Обязательно |
| 18 | `lupus_modules/w.py` | FFI-реализация модуля test (или встроенный в интерпретатор) | Обязательно |
| 19 | `stdlib/core.lupus` | Определения и документация core-модуля (если часть на Lupus) | Опционально |
| 20 | `tests/test_lexer.py` | Юнит-тесты лексера | Обязательно |
| 21 | `tests/test_parser.py` | Юнит-тесты парсера | Обязательно |
| 22 | `tests/test_typechecker.py` | Юнит-тесты тайп-чекера | Обязательно |
| 23 | `tests/test_interpreter.py` | Юнит-тесты интерпретатора | Обязательно |
| 24 | `tests/test_ffi.py` | Юнит-тесты FFI | Обязательно |
| 25 | `tests/integration/` | Интеграционные тесты: все примеры из раздела 9 | Обязательно |
| 26 | `docs/spec.md` | Копия этой спецификации | Обязательно |
| 27 | `docs/grammar.ebnf` | Формальная грамматика | Обязательно |
| 28 | `docs/api.md` | Документация API стандартной библиотеки | Обязательно |
| 29 | `docs/tutorial.md` | Учебник для LLM (как писать на Lupus) | Обязательно |
| 30 | `Makefile` / `pyproject.toml` | Сборка, установка, запуск тестов | Обязательно |

### 10.2. Требования к качеству кода

1. **Покрытие тестами:** Не менее 80% строк для `lexer.py`, `parser.py`, `typechecker.py`, `interpreter.py`.
2. **JSON-ошибки:** Все ошибки (включая runtime panic) должны выводиться в формате раздела 6.
3. **AST:** Метод `.to_json()` должен быть детерминированным и соответствовать разделу 7.
4. **FFI:** Каждый Python-модуль (`lupus_modules/*.py`) должен содержать `__lupus_exports__`. Все вызовы обёрнуты в `try...except`.
5. **CLI:** Поддержка флагов `--json` (вывод только JSON), `--verbose`, `--no-lint`.
6. **Документация:** Каждая публичная функция имеет docstring с описанием и типами.

### 10.3. Порядок генерации (рекомендация для GPT-5.5)

1. **Сначала** AST и лексер (фундамент).
2. **Затем** парсер + тесты парсера.
3. **Затем** типовая система + тайп-чекер + тесты.
4. **Затем** runtime (values, environment) + интерпретатор + тесты.
5. **Затем** FFI + модули стандартной библиотеки.
6. **Затем** CLI + интеграционные тесты.
7. **Наконец** документация.

### 10.4. Критерии приёмки (Definition of Done)

- [ ] Все примеры из раздела 9 выполняются без ошибок (`lupus run` и `lupus test`).
- [ ] Команда `lupus check` проходит без ошибок для всех `.lupus` файлов в `examples/`.
- [ ] `lupus ast example.lupus` выводит валидный JSON, который проходит валидацию по схеме раздела 7.
- [ ] FFI-модули `senko`, `texas`, `kaltsit`, `amiya`, `w` загружаются и работают.
- [ ] Интерпретатор корректно обрабатывает все ошибки из каталога раздела 6.
- [ ] Тестовое покрытие ≥ 80% для core-файлов.
- [ ] Python-исключения в FFI-модулях не приводят к краху интерпретатора (выдают JSON-ошибку).

---

## Приложение А. Словарь терминов

| Термин | Описание |
|--------|----------|
| **AST** | Abstract Syntax Tree — абстрактное синтаксическое дерево. |
| **EBNF** | Extended Backus-Naur Form — форма записи грамматик. |
| **FFI** | Foreign Function Interface — механизм вызова функций хоста. |
| **Hindley-Milner** | Алгоритм вывода типов с полиморфизмом. |
| **LALR** | Look-Ahead LR — алгоритм разбора (используется в lark). |
| **Opaque type** | Тип, внутреннее устройство которого скрыто от языка (socket, task). |
| **Prelude / Core** | Автоматически импортируемый набор функций. |
| **REPL** | Read-Eval-Print Loop — интерактивный режим. |
| **Span** | Диапазон позиций в исходном коде (файл, строка, колонка, байты). |
| **Unit** | Тип с единственным значением `unit`, используется для side-effect функций. |
| **Value Restriction** | Ограничение, запрещающее полиморфизм для mutable-переменных в HM. |

---

*Документ подготовлен для GPT-5.5 Mimi. Все разделы самодостаточны и содержат достаточно информации для реализации языка Lupus v1.2 на Python.*
