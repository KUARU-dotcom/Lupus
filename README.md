# Lupus - V0.2 (Rust Rewrite)

Полностью функциональный **Rust интерпретатор** для Lupus — экспериментального Lisp-подобного языка программирования, оптимизированного для взаимодействия с LLM и генерации кода.

## 📋 Описание

**Lupus** — экспериментальный язык программирования на основе синтаксиса Lisp (S-выражения). Данная версия (V0.2) — полнофункциональный **tree-walk интерпретатор**, реализованный на Rust с модульной архитектурой, достигший **81% успешности** на комплексном бенчмарке.

### ✨ Основные возможности

- ✅ **Переменные** - определение неизменяемых и изменяемых переменных
- ✅ **Функции** - определение функций, рекурсия, функции высшего порядка
- ✅ **Замыкания** - лексическая область видимости с полной поддержкой замыканий
- ✅ **Операторы** - арифметика, сравнение, логические операции
- ✅ **Управление потоком** - условия (`if`), циклы (`while`), группировка (`begin`)
- ✅ **Структуры данных** - списки, базовые операции с данными
- ✅ **Ввод-вывод** - вывод в консоль и преобразование типов
- ✅ **Интерпретатор** - запуск файлов с надёжной обработкой ошибок
- ✅ **Модульная архитектура** - чистое разделение: лексер, парсер, интерпретатор, runtime

## 🚀 Быстрый старт

### Из исходников

```bash
git clone https://github.com/KUARU-dotcom/Lupus.git
cd Lupus
cargo build --release
./target/release/lupus examples/hello.lupus
```

### Готовые бинарники

Скачайте с [Releases](https://github.com/KUARU-dotcom/Lupus/releases):

```bash
# Linux/macOS
./lupus examples/factorial.lupus

# Windows
lupus.exe examples/factorial.lupus
```

### Первая программа

```lisp
;; factorial.lupus
(define (factorial n)
  (if (= n 0)
    1
    (* n (factorial (- n 1)))))

(print (int->str (factorial 5)))  ;; Вывод: 120
```

## 📊 Результаты бенчмарка

V0.2 достигает **81% успешности** на наборе из 100 разнообразных задач программирования:

| Модель | Успешность | Задач |
|--------|-----------|-------|
| **merged-lupus-2bV2 (LoRA)** | **81%** | 100 |
| Qwen 2B (baseline) | ~45% | 100 |
| Python (merged-python-2b) | 90% | 100 |

### Успешность по категориям

| Категория | Успешность |
|-----------|-----------|
| Арифметика | 100% |
| Логика | 100% |
| Циклы | 87% |
| Рекурсия | 80% |
| Списки | 73% |
| Строки | 70% |
| Комбинированные задачи | 64% |

## 📚 Документация

- **[QUICKSTART.md](QUICKSTART.md)** - Руководство для начинающих с примерами
- **[SYNTAX.md](SYNTAX.md)** - Полная спецификация языка
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Дизайн и реализация интерпретатора

## 📁 Структура проекта

```
Lupus/
├── src/
│   ├── main.rs              # Точка входа CLI
│   ├── lib.rs               # Публичный API библиотеки
│   ├── lexer.rs             # Токенизация (1000+ строк)
│   ├── parser.rs            # Построение AST (600+ строк)
│   ├── ast.rs               # Определения узлов AST
│   ├── interpreter.rs       # Tree-walk интерпретатор (2300+ строк)
│   ├── environment.rs       # Управление областью видимости
│   ├── value.rs             # Представление значений runtime
│   ├── error.rs             # Типы и обработка ошибок
│   └── builtins/
│       ├── mod.rs
│       ├── arithmetic.rs    # Арифметические операции
│       ├── comparison.rs    # Операции сравнения
│       ├── logic.rs         # Логические операции
│       ├── strings.rs       # Работа со строками
│       └── lists.rs         # Работа со списками
├── examples/
│   ├── factorial.lupus
│   ├── loops.lupus
│   ├── lists.lupus
│   └── closures.lupus
├── Cargo.toml
├── README.md                # Этот файл
└── LICENSE
```

## 💻 Примеры кода

### Пример 1: Факториал (рекурсия)

```lisp
(define (factorial n)
  (if (= n 0)
    1
    (* n (factorial (- n 1)))))

(print (int->str (factorial 5)))  ;; Вывод: 120
```

### Пример 2: Работа со списками

```lisp
(define numbers (list 1 2 3 4 5))
(define length (length numbers))
(define first (nth numbers 0))

(print (int->str first))    ;; Вывод: 1
(print (int->str length))   ;; Вывод: 5
```

### Пример 3: Циклы и изменяемые переменные

```lisp
(define-mutable sum 0)
(define-mutable i 1)

(while (<= i 10)
  (set! sum (+ sum i))
  (set! i (+ i 1)))

(print (int->str sum))  ;; Вывод: 55
```

### Пример 4: Функции вы��шего порядка и замыкания

```lisp
(define (make-multiplier factor)
  (lambda (x) (* x factor)))

(define times5 (make-multiplier 5))
(print (int->str (times5 3)))  ;; Вывод: 15

(define times10 (make-multiplier 10))
(print (int->str (times10 2)))  ;; Вывод: 20
```

### Пример 5: Условная логика

```lisp
(define (grade score)
  (if (>= score 90)
    "A"
    (if (>= score 80)
      "B"
      (if (>= score 70)
        "C"
        "F"))))

(print (grade 85))  ;; Вывод: B
```

## 🎯 Поддерживаемые конструкции

### Синтаксис

```lisp
;; Комментарии начинаются с ;;

;; Определения переменных
(define x 10)                    ;; Неизменяемая константа
(define-mutable y 20)            ;; Изменяемая переменная
(set! y 25)                      ;; Присваивание

;; Функции
(define (add a b) (+ a b))       ;; Именованная функция
(define multiply (lambda (a b) (* a b)))  ;; Лямбда

;; Вызовы функций
(add 5 3)                        ;; => 8

;; Условия
(if (> x 0) "положительное" "отрицательное")

;; Циклы
(while (< i 10) (set! i (+ i 1)))

;; Группировка
(begin
  (define a 1)
  (define b 2)
  (+ a b))
```

### Встроенные функции

#### Арифметика
```lisp
(+ 1 2)          ;; Сложение        => 3
(- 5 3)          ;; Вычитание       => 2
(* 4 2)          ;; Умножение       => 8
(/ 10 2)         ;; Деление         => 5
(% 7 2)          ;; Остаток от деления => 1
```

#### Сравнение
```lisp
(= 5 5)          ;; Равно           => true
(!= 5 3)         ;; Не равно        => true
(< 3 5)          ;; Меньше          => true
(> 5 3)          ;; Больше          => true
(<= 5 5)         ;; Меньше или равно => true
(>= 5 5)         ;; Больше или равно => true
```

#### Логика
```lisp
(and true false) ;; Логическое И    => false
(or false true)  ;; Логическое ИЛИ  => true
(not false)      ;; Логическое НЕ   => true
```

#### Списки
```lisp
(list 1 2 3)     ;; Создание списка => (1 2 3)
(nth lst 0)      ;; Элемент по индексу
(length lst)     ;; Длина списка
(car lst)        ;; Первый элемент (Lisp традиция)
(cdr lst)        ;; Остаток списка (Lisp традиция)
```

#### Строки и ввод-вывод
```lisp
(print "привет")                 ;; Вывод в консоль
(int->str 42)                    ;; Преобразование int в строку
(string-append "a" "b")          ;; Конкатенация строк
(string-length "hello")          ;; Длина строки
```

## 🏗️ Архитектура

Lupus V0.2 использует модульный, хорошо разделённый дизайн интерпретатора:

### 1. **Лексер** (`src/lexer.rs`)
- Токенизация исходного кода Lupus
- Поддержка: целых чисел, вещественных чисел, строк, символов, скобок, комментариев
- ~1000 строк кода на Rust

### 2. **Парсер** (`src/parser.rs`)
- Парсер рекурсивного спуска
- Построение абстрактного синтаксического дерева (AST) из токенов
- Парсинг S-выражений с правильной вложенностью
- Восстановление ошибок и диагностика
- ~600 строк кода на Rust

### 3. **Интерпретатор** (`src/interpreter.rs`)
- Tree-walk интерпретатор с немедленным вычислением
- Управление окружением для области видимости переменных
- Поддержка замыканий с лексическим связыванием
- Обработка специальных форм (`define`, `lambda`, `if`, `while`)
- Диспетчеризация встроенных функций
- ~2300 строк кода на Rust

### 4. **Runtime**
- **`value.rs`** - Представление значений runtime (целые числа, логические значения, функции, списки и т.д.)
- **`environment.rs`** - Управление областью видимости переменных с цепочкой родителей
- **`error.rs`** - Типы ошибок и отчётность

### 5. **Встроенные функции** (`src/builtins/`)
- Модульная библиотека встроенных функций
- Организована по категориям: арифметика, сравнение, логика, строки, списки

## ⚠️ Известные ограничения

- **Нет обработки исключений** - ошибки приводят к завершению программы
- **Нет модулей/пространств имён** - единое глобальное пространство
- **Нет объектно-ориентированных возможностей** - функциональная парадигма
- **Ограниченные структуры данных** - списки — основная коллекция
- **Базовая стандартная библиотека** - только необходимые встроенные функции
- **Нет FFI** - невозможно вызывать внешние библиотеки (планируется для V1.0)

## 🔮 Дорожная карта

### V0.2 (Текущая)
- ✅ Полный Rust интерпретатор
- ✅ Модульная архитектура
- ✅ 81% успешности на бенчмарке
- ✅ Полный набор встроенных функций

### V0.3 (Планируется)
- ⏳ Интерактивный режим REPL
- ⏳ Дополнительные управляющие конструкции: `cond`, `for`, `do-while`
- ⏳ Функции на списках: `map`, `filter`, `fold`
- ⏳ Улучшенные сообщения об ошибках и трассировка стека

### V1.0 (Будущее)
- ⏳ Система типов (постепенная типизация)
- ⏳ Система модулей и импорты
- ⏳ `defstruct` для пользовательских типов данных
- ⏳ FFI к библиотекам C/Rust
- ⏳ Паттерн-матчинг (`match`)
- ⏳ Система макросов
- ⏳ Проходы оптимизации (tail call, constant folding)
- ⏳ Полная стандартная библиотека

## 📖 Наследие: Python-прототип (V0.1)

Исходный Python-прототип (`lupus_proto.py`, ~980 строк) использовался для:
- Валидации дизайна языка
- Генерации тренировочных данных для fine-tuning LLM
- Служить эталонной реализацией

Python версия теперь архивирована. Rust переписка — текущая производственная версия.

Для доступа к коду V0.1:
```bash
git log --oneline | grep "proto"  # Найти коммиты, ссылающиеся на прототип
# Или проверить директорию `legacy/`
```

## 🧪 Тестирование

```bash
# Сборка релиза
cargo build --release

# Запуск программы
./target/release/lupus examples/factorial.lupus

# Запуск всех примеров
for f in examples/*.lupus; do
  echo "Running $f..."
  ./target/release/lupus "$f"
done

# Запуск с отладкой (в разработке)
RUST_LOG=debug ./target/release/lupus examples/factorial.lupus
```

## 📋 Требования

- **Rust 1.70+** (для сборки из исходников)
- **Никаких внешних зависимостей** - использует только стандартную библиотеку Rust
- **Linux или Windows** - кроссплатформенная поддержка

## 🔗 Ссылки

- **[Спецификация Lupus V1.0](https://github.com/KUARU-dotcom/Lupus/blob/specification/Lupus_V1.0_Specification_Russian.md)** - Полный дизайн языка
- **[Обсуждения на GitHub](https://github.com/KUARU-dotcom/Lupus/discussions)** - Сообщество и обсуждение дизайна
- **[Releases на GitHub](https://github.com/KUARU-dotcom/Lupus/releases)** - Скачивание бинарников
- **[Issues](https://github.com/KUARU-dotcom/Lupus/issues)** - Отчёты об ошибках и запросы функций

## 📄 Лицензия

Лицензия MIT - см. файл [LICENSE](LICENSE).

---

**Версия**: V0.2 (Rust Rewrite)  
**Статус**: Стабильный прототип  
**Последнее обновление**: 2026  
**Успешность бенчмарка**: 81% (100 задач)  
**Язык**: Rust (1.70+)  
**Зависимости**: Нет (только стандартная библиотека)

```markdown
# Lupus - V0.2 (Rust Rewrite)

A fully functional **Rust interpreter** for Lupus — an experimental Lisp-like programming language optimized for LLM interaction and code generation.

## 📋 Description

**Lupus** is an experimental programming language based on Lisp syntax (S-expressions). This version (V0.2) is a fully functional **tree-walk interpreter** implemented in Rust with a modular architecture, achieving **81% success rate** on a comprehensive benchmark.

### ✨ Key Features

- ✅ **Variables** - definition of immutable and mutable variables
- ✅ **Functions** - function definitions, recursion, higher-order functions
- ✅ **Closures** - lexical scoping with full closure support
- ✅ **Operators** - arithmetic, comparison, logical operations
- ✅ **Flow Control** - conditionals (`if`), loops (`while`), grouping (`begin`)
- ✅ **Data Structures** - lists, basic data operations
- ✅ **I/O** - console output and type conversion
- ✅ **Interpreter** - file execution with robust error handling
- ✅ **Modular Architecture** - clean separation: lexer, parser, interpreter, runtime

## 🚀 Quick Start

### From Source

```bash
git clone https://github.com/KUARU-dotcom/Lupus.git
cd Lupus
cargo build --release
./target/release/lupus examples/hello.lupus
```

### Pre-built Binaries

Download from [Releases](https://github.com/KUARU-dotcom/Lupus/releases):

```bash
# Linux/macOS
./lupus examples/factorial.lupus

# Windows
lupus.exe examples/factorial.lupus
```

### First Program

```lisp
;; factorial.lupus
(define (factorial n)
  (if (= n 0)
    1
    (* n (factorial (- n 1)))))

(print (int->str (factorial 5)))  ;; Output: 120
```

## 📊 Benchmark Results

V0.2 achieves **81% success rate** on a suite of 100 diverse programming tasks:

| Model | Success Rate | Tasks |
|--------|-----------|-------|
| **merged-lupus-2bV2 (LoRA)** | **81%** | 100 |
| Qwen 2B (baseline) | ~45% | 100 |
| Python (merged-python-2b) | 90% | 100 |

### Success Rate by Category

| Category | Success Rate |
|-----------|-----------|
| Arithmetic | 100% |
| Logic | 100% |
| Loops | 87% |
| Recursion | 80% |
| Lists | 73% |
| Strings | 70% |
| Combined Tasks | 64% |

## 📚 Documentation

- **[QUICKSTART.md](QUICKSTART.md)** - Beginner's guide with examples
- **[SYNTAX.md](SYNTAX.md)** - Complete language specification
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Interpreter design and implementation

## 📁 Project Structure

```
Lupus/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Public library API
│   ├── lexer.rs             # Tokenization (1000+ lines)
│   ├── parser.rs            # AST construction (600+ lines)
│   ├── ast.rs               # AST node definitions
│   ├── interpreter.rs       # Tree-walk interpreter (2300+ lines)
│   ├── environment.rs       # Scope management
│   ├── value.rs             # Runtime value representation
│   ├── error.rs             # Error types and handling
│   └── builtins/
│       ├── mod.rs
│       ├── arithmetic.rs    # Arithmetic operations
│       ├── comparison.rs    # Comparison operations
│       ├── logic.rs         # Logical operations
│       ├── strings.rs       # String operations
│       └── lists.rs         # List operations
├── examples/
│   ├── factorial.lupus
│   ├── loops.lupus
│   ├── lists.lupus
│   └── closures.lupus
├── Cargo.toml
├── README.md                # This file
└── LICENSE
```

## 💻 Code Examples

### Example 1: Factorial (Recursion)

```lisp
(define (factorial n)
  (if (= n 0)
    1
    (* n (factorial (- n 1)))))

(print (int->str (factorial 5)))  ;; Output: 120
```

### Example 2: Working with Lists

```lisp
(define numbers (list 1 2 3 4 5))
(define length (length numbers))
(define first (nth numbers 0))

(print (int->str first))    ;; Output: 1
(print (int->str length))   ;; Output: 5
```

### Example 3: Loops and Mutable Variables

```lisp
(define-mutable sum 0)
(define-mutable i 1)

(while (<= i 10)
  (set! sum (+ sum i))
  (set! i (+ i 1)))

(print (int->str sum))  ;; Output: 55
```

### Example 4: Higher-Order Functions and Closures

```lisp
(define (make-multiplier factor)
  (lambda (x) (* x factor)))

(define times5 (make-multiplier 5))
(print (int->str (times5 3)))  ;; Output: 15

(define times10 (make-multiplier 10))
(print (int->str (times10 2)))  ;; Output: 20
```

### Example 5: Conditional Logic

```lisp
(define (grade score)
  (if (>= score 90)
    "A"
    (if (>= score 80)
      "B"
      (if (>= score 70)
        "C"
        "F"))))

(print (grade 85))  ;; Output: B
```

## 🎯 Supported Constructs

### Syntax

```lisp
;; Comments start with ;;

;; Variable definitions
(define x 10)                    ;; Immutable constant
(define-mutable y 20)            ;; Mutable variable
(set! y 25)                      ;; Assignment

;; Functions
(define (add a b) (+ a b))       ;; Named function
(define multiply (lambda (a b) (* a b)))  ;; Lambda

;; Function calls
(add 5 3)                        ;; => 8

;; Conditionals
(if (> x 0) "positive" "negative")

;; Loops
(while (< i 10) (set! i (+ i 1)))

;; Grouping
(begin
  (define a 1)
  (define b 2)
  (+ a b))
```

### Built-in Functions

#### Arithmetic
```lisp
(+ 1 2)          ;; Addition        => 3
(- 5 3)          ;; Subtraction     => 2
(* 4 2)          ;; Multiplication  => 8
(/ 10 2)         ;; Division        => 5
(% 7 2)          ;; Modulo          => 1
```

#### Comparison
```lisp
(= 5 5)          ;; Equal           => true
(!= 5 3)         ;; Not equal       => true
(< 3 5)          ;; Less than       => true
(> 5 3)          ;; Greater than    => true
(<= 5 5)         ;; Less or equal   => true
(>= 5 5)         ;; Greater or equal => true
```

#### Logic
```lisp
(and true false) ;; Logical AND     => false
(or false true)  ;; Logical OR      => true
(not false)      ;; Logical NOT     => true
```

#### Lists
```lisp
(list 1 2 3)     ;; Create list     => (1 2 3)
(nth lst 0)      ;; Element by index
(length lst)     ;; List length
(car lst)        ;; First element (Lisp tradition)
(cdr lst)        ;; Rest of list (Lisp tradition)
```

#### Strings and I/O
```lisp
(print "hello")                  ;; Console output
(int->str 42)                    ;; Int to string conversion
(string-append "a" "b")          ;; String concatenation
(string-length "hello")          ;; String length
```

## 🏗️ Architecture

Lupus V0.2 uses a modular, well-separated interpreter design:

### 1. **Lexer** (`src/lexer.rs`)
- Tokenizes Lupus source code
- Supports: integers, floats, strings, symbols, parentheses, comments
- ~1000 lines of Rust code

### 2. **Parser** (`src/parser.rs`)
- Recursive descent parser
- Builds Abstract Syntax Tree (AST) from tokens
- Parses S-expressions with proper nesting
- Error recovery and diagnostics
- ~600 lines of Rust code

### 3. **Interpreter** (`src/interpreter.rs`)
- Tree-walk interpreter with eager evaluation
- Environment management for variable scoping
- Closure support with lexical binding
- Special form handling (`define`, `lambda`, `if`, `while`)
- Built-in function dispatch
- ~2300 lines of Rust code

### 4. **Runtime**
- **`value.rs`** - Runtime value representation (integers, booleans, functions, lists, etc.)
- **`environment.rs`** - Variable scope management with parent chaining
- **`error.rs`** - Error types and reporting

### 5. **Built-in Functions** (`src/builtins/`)
- Modular built-in function library
- Organized by category: arithmetic, comparison, logic, strings, lists

## ⚠️ Known Limitations

- **No exception handling** - errors cause program termination
- **No modules/namespaces** - single global namespace
- **No object-oriented features** - functional paradigm
- **Limited data structures** - lists are the primary collection
- **Minimal standard library** - only essential built-in functions
- **No FFI** - cannot call external libraries (planned for V1.0)

## 🔮 Roadmap

### V0.2 (Current)
- ✅ Full Rust interpreter
- ✅ Modular architecture
- ✅ 81% benchmark success rate
- ✅ Complete set of built-in functions

### V0.3 (Planned)
- ⏳ Interactive REPL mode
- ⏳ Additional control structures: `cond`, `for`, `do-while`
- ⏳ List functions: `map`, `filter`, `fold`
- ⏳ Improved error messages and stack traces

### V1.0 (Future)
- ⏳ Type system (gradual typing)
- ⏳ Module system and imports
- ⏳ `defstruct` for user-defined data types
- ⏳ FFI to C/Rust libraries
- ⏳ Pattern matching (`match`)
- ⏳ Macro system
- ⏳ Optimization passes (tail call, constant folding)
- ⏳ Full standard library

## 📖 Legacy: Python Prototype (V0.1)

The original Python prototype (`lupus_proto.py`, ~980 lines) was used for:
- Validating language design
- Generating training data for LLM fine-tuning
- Serving as a reference implementation

The Python version is now archived. The Rust rewrite is the current production version.

To access V0.1 code:
```bash
git log --oneline | grep "proto"  # Find commits referencing the prototype
# Or check the `legacy/` directory
```

## 🧪 Testing

```bash
# Build release
cargo build --release

# Run a program
./target/release/lupus examples/factorial.lupus

# Run all examples
for f in examples/*.lupus; do
  echo "Running $f..."
  ./target/release/lupus "$f"
done

# Run with debugging (in development)
RUST_LOG=debug ./target/release/lupus examples/factorial.lupus
```

## 📋 Requirements

- **Rust 1.70+** (for building from source)
- **No external dependencies** - uses only Rust's standard library
- **Linux or Windows** - cross-platform support

## 🔗 Links

- **[Lupus V1.0 Specification](https://github.com/KUARU-dotcom/Lupus/blob/specification/Lupus_V1.0_Specification_Russian.md)** - Full language design
- **[GitHub Discussions](https://github.com/KUARU-dotcom/Lupus/discussions)** - Community and design discussions
- **[GitHub Releases](https://github.com/KUARU-dotcom/Lupus/releases)** - Binary downloads
- **[Issues](https://github.com/KUARU-dotcom/Lupus/issues)** - Bug reports and feature requests

## 📄 License

MIT License - see [LICENSE](LICENSE) file.

---

**Version**: V0.2 (Rust Rewrite)  
**Status**: Stable prototype  
**Last Updated**: 2026  
**Benchmark Success**: 81% (100 tasks)  
**Language**: Rust (1.70+)  
**Dependencies**: None (standard library only)
