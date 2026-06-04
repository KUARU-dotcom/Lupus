# LUPUS V1.0 — Programming Language Specification

**Version:** 1.0  
**Date:** 2026-06-04  
**Status:** Ready for Implementation  
**Target Audience:** LLMs, interpreter developers, compiler architects  

---

## Table of Contents

1. [EBNF Grammar](#1-ebnf-grammar)
2. [Built-in Types and Operations](#2-built-in-types-and-operations)
3. [Standard Library (API)](#3-standard-library-api)
4. [FFI Specification](#4-ffi-specification)
5. [Test Format](#5-test-format)
6. [Error Format (JSON)](#6-error-format-json)
7. [AST Serialization (JSON)](#7-ast-serialization-json)
8. [Implementation Recommendations (Python)](#8-implementation-recommendations-python)
9. [Program Examples](#9-program-examples)
10. [LLM Checklist](#10-llm-checklist)

---

## 1. EBNF Grammar

### 1.1. Lexical Rules

```ebnf
(* === Lexer === *)

program       = { toplevel } , EOF ;

toplevel      = define
              | define_mutable
              | define_const
              | define_public
              | defstruct
              | import
              | test
              | directive ;

(* --- Tokens --- *)

IDENTIFIER    = ( LETTER | "_" ) , { LETTER | DIGIT | "_" | "-" } ;
              (* Note: "/" is NOT part of IDENTIFIER. Qualified names are parsed at the parser stage. *)

TYPE_NAME     = UPPERCASE_LETTER , { LETTER | DIGIT | "_" } ;
              (* List, Option, Result, Tuple, Map, Tensor, Func, int, float, bool, str, unit *)

STRING        = '"' , { CHAR | ESCAPE } , '"' ;
CHAR          = any Unicode character except '"' and '\\' and control chars ;
ESCAPE        = '\\' , ('"' | '\\' | 'n' | 't' | 'r' | '0' | ('x' , HEX_DIGIT , HEX_DIGIT) | ('u' , HEX_DIGIT , HEX_DIGIT , HEX_DIGIT , HEX_DIGIT)) ;

INTEGER       = DIGIT , { DIGIT } ;
FLOAT         = DIGIT , { DIGIT } , "." , DIGIT , { DIGIT } , [ ("e" | "E") , ["-" | "+"] , DIGIT , {DIGIT} ] ;
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

(* --- Symbols --- *)
LPAREN        = "(" ;
RPAREN        = ")" ;
ARROW         = "->" ;
SLASH         = "/" ;

(* --- Auxiliaries --- *)
LETTER        = "a" .. "z" | "A" .. "Z" ;
UPPERCASE_LETTER = "A" .. "Z" ;
DIGIT         = "0" .. "9" ;
HEX_DIGIT     = DIGIT | "a" .. "f" | "A" .. "F" ;
```

### 1.2. Syntactic Rules

```ebnf
(* === Parser === *)

(* --- Top level --- *)

define        = LPAREN , "define" , IDENTIFIER , expr , RPAREN
              | LPAREN , "define" , func_header , body , RPAREN ;
              (* (define x 10) or (define (add a b) -> int (+ a b)) *)

define_mutable = LPAREN , "define-mutable" , IDENTIFIER , expr , RPAREN ;
              (* (define-mutable y 20) *)

define_const  = LPAREN , "define-const" , IDENTIFIER , expr , RPAREN ;
              (* (define-const pi 3.1416) — immutable, compile-time constant *)

define_public = LPAREN , "define-public" , func_header , body , RPAREN ;
              (* (define-public (add (a int) (b int)) -> int (+ a b)) *)
              (* Return type annotation and all parameter annotations are mandatory. *)
              (* Parameters may be annotated: (a int) or bare: a *)
              (* If a parameter has no annotation in define-public, the type is inferred, but the type checker *)
              (* generates a hard error missing-param-type. *)

func_header   = LPAREN , IDENTIFIER , { param } , RPAREN , [ "->" , type_expr ] ;
              (* Function name + parameters + optional return type annotation *)
              (* For define-public, omitting -> type_expr at the type-checking stage is an error. *)

param         = IDENTIFIER
              | LPAREN , IDENTIFIER , type_expr , RPAREN ;
              (* annotated parameter: (a int) *)

set_bang      = LPAREN , "set!" , IDENTIFIER , expr , RPAREN ;
              (* (set! x (+ x 1)) *)

lambda        = LPAREN , "lambda" , LPAREN , { param } , RPAREN , [ "->" , type_expr ] , body , RPAREN ;
              (* (lambda (x) (* x x)) *)
              (* (lambda ((x int)) -> int (* x x)) *)

(* --- Body (sequence of expressions) --- *)

body          = { expr } ;
              (* One or more expressions. The result of a body is the value of the last expression. *)
              (* All expressions except the last must have type unit (or are ignored). *)

(* --- Control constructs (all are expressions) --- *)

if_expr       = LPAREN , "if" , expr , expr , expr , RPAREN ;
              (* (if condition then-expr else-expr) *)

cond_expr     = LPAREN , "cond" , { cond_clause } , RPAREN ;
cond_clause   = LPAREN , expr , expr , RPAREN
              | LPAREN , "else" , expr , RPAREN ;

match_expr    = LPAREN , "match" , expr , { match_clause } , RPAREN ;
match_clause  = LPAREN , pattern , body , RPAREN ;
              (* ((some value) (print value)) — pattern + body *)

pattern       = "_"                              (* wildcard *)
              | IDENTIFIER                       (* variable, binds to any value *)
              | "none"                           (* none pattern — special rule, since none is LITERAL_NONE, not IDENTIFIER *)
              | LPAREN , constructor , { pattern } , RPAREN
              (* (some value), (success data), (point x y) *)
              | LPAREN , "Tuple" , { pattern } , RPAREN ;
              (* (Tuple a b c) — tuple pattern matching *)

constructor   = IDENTIFIER ;
              (* none, some, success, failure, point, etc. *)

if_let        = LPAREN , "if-let" , LPAREN , IDENTIFIER , expr , RPAREN , expr , expr , RPAREN ;
              (* (if-let (value expr) then-expr else-expr) *)
              (* In v0.1 if-let works only with Option. For Result use match. *)

(* --- Loops --- *)

while_expr    = LPAREN , "while" , expr , body , RPAREN ;
              (* (while (< i 10) (print i) (set! i (+ i 1))) *)
              (* Return type: unit *)

for_expr      = LPAREN , "for" , IDENTIFIER , "in" , expr , body , RPAREN ;
              (* (for i in (range 0 10) (print i)) *)
              (* Return type: unit *)

loop_expr     = LPAREN , "loop" , body , RPAREN ;
              (* (loop (print "tick") (sleep 1000)) *)
              (* Return type: unit. In v0.1 loop exits only via (exit 0). No break/return constructs. *)

(* --- Modules and import --- *)

import        = LPAREN , "import" , import_path , [ import_modifier ] , RPAREN ;
import_path   = LPAREN , IDENTIFIER , IDENTIFIER , RPAREN ;
              (* (senko math) — package senko, module math *)
import_modifier = ":as" , IDENTIFIER
                | ":all" ;

defstruct     = LPAREN , "defstruct" , defstruct_header , { struct_field } , RPAREN ;
defstruct_header = IDENTIFIER
                   | LPAREN , IDENTIFIER , { TYPE_NAME } , RPAREN ;
              (* (defstruct point (x int) (y int)) *)
              (* (defstruct (Node a) (value a) (left (Option (Node a)))) *)

struct_field  = LPAREN , IDENTIFIER , type_expr , RPAREN ;

(* --- Tests (top level only) --- *)

test          = LPAREN , "test" , STRING , body , RPAREN ;
              (* (test "name" (assert ...)) *)

assert_expr   = LPAREN , "assert" , expr , RPAREN ;
              (* (assert (= 1 1)) *)

(* --- Directives --- *)

directive     = LPAREN , "#lupus" , IDENTIFIER , { IDENTIFIER | STRING } , RPAREN ;
              (* (#lupus enable-check types), (#lupus ffi python "module.path") *)

(* --- Local definitions (inside body only) --- *)

local_define  = LPAREN , "define" , IDENTIFIER , expr , RPAREN
              | LPAREN , "define-mutable" , IDENTIFIER , expr , RPAREN ;
              (* Local variables inside functions/branches. Syntax identical to toplevel. *)

(* --- Expressions --- *)

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
              (* All control constructs and local defines are expressions *)

call_expr     = LPAREN , operator , { expr } , RPAREN
              | LPAREN , expr , { expr } , RPAREN ;
              (* Function call: (func args...) or (+ 1 2) *)

literal       = INTEGER | FLOAT | BOOLEAN | STRING | LITERAL_NONE | LITERAL_UNIT ;
              (* none and unit are separate tokens, not IDENTIFIER *)

qualified_name = IDENTIFIER , SLASH , IDENTIFIER ;
              (* math/sqrt, net/tcp-connect *)
              (* Available only for imported modules. *)

operator      = "+" | "-" | "*" | "/" | "%" | "=" | "!=" | "<" | ">" | "<=" | ">="
              | "not" | "and" | "or"
              | "list" | "tuple" | "map" | "range"
              | "string-append" | "int->str" | "float->str" | "str->int" | "str->float"
              | "string-split" | "string-length"
              | "some" | "success" | "failure"
              | "cons" | "head" | "tail" | "length" | "nth" | "empty?"
              | "list-map" | "list-filter" | "list-fold"
              | "map-get" | "map-set" | "map-keys" | "map-values" | "map-has?"
              | "tensor" | "tensor-shape" | "tensor-add" | "tensor-mul" | "tensor-transpose"
              | IDENTIFIER ; (* any user-defined function or constructor *)

(* --- Types --- *)

type_expr     = "int" | "float" | "bool" | "str" | "unit"
              | IDENTIFIER    (* User-defined types: point, person, socket, task, channel *)
              | TYPE_NAME     (* Generic parameters: a, b, T *)
              | LPAREN , "List" , type_expr , RPAREN
              | LPAREN , "Tuple" , { type_expr } , RPAREN
              | LPAREN , "Map" , type_expr , type_expr , RPAREN
              | LPAREN , "Tensor" , RPAREN
              | LPAREN , "Func" , { type_expr } , type_expr , RPAREN
              | LPAREN , "Option" , type_expr , RPAREN
              | LPAREN , "Result" , type_expr , type_expr , RPAREN
              | LPAREN , IDENTIFIER , { type_expr } , RPAREN ;
              (* User-defined generics: (Node a), (Tree a b) *)
              (* Built-in parameterized opaque types: (channel str) *)
```

### 1.3. Grammar Notes

- **Prefix notation:** All operators and function calls are strictly prefix. No infix syntax. The AST is unambiguous.
- **Indentation:** Has no syntactic meaning. Used only for readability.
- **Qualified names:** Available only for imported modules. Format: `<module-alias>/<identifier>`. The `/` character cannot be part of a plain IDENTIFIER.
- **Body (sequence):** The body of a function, `if` branch, `match` branch, or loop is a sequence of 1+ expressions. All expressions in a body are evaluated sequentially; the result is the value of the last expression. All expressions except the last must have type `unit` (checked by the type checker).
- **Local defines:** `define` and `define-mutable` may be used at the top level or inside any `body`. Scope is from the point of definition to the end of the current `body`.
- **Unary minus:** Handled by the parser as a call `(- 0 x)`, not at the lexer level. Negative number literals are not allowed in source code; use `(- 0 5)` for `-5` and `(- 0 3.5)` for `-3.5`.
- **Variadic functions:** Only the built-in constructors `list`, `tuple`, `map`, `string-append`, and `+` (for 2+ arguments) accept a variable number of arguments. All user-defined functions have fixed arity determined at declaration.
- **Special built-in forms `and` and `or`:** The forms `and` and `or` are not ordinary functions. They are special built-in forms of the compiler supporting short-circuit (lazy) evaluation and accepting 2+ arguments. Example: `(and (> x 0) (< x 10))`. They appear in `operator` for syntactic parsing, but are semantically handled by the interpreter as special forms.
- **Tests:** The `test` form is allowed only at the top level (`toplevel`), not inside functions.
- **Generics in defstruct:** `(defstruct (Node a) ...)` creates a parameterized type. Usage: `(Node int)`.
- **Parameterized opaque types:** FFI types such as `(channel str)` are successfully parsed by the existing parser rule `LPAREN , IDENTIFIER , { type_expr } , RPAREN`. For the type checker, `channel` is a built-in parameterized opaque type, analogous to user-defined generics but with hidden internals.
- **Import modifier `:all`:** Imports all `define-public` symbols and FFI exports from the target module directly into the current environment without a prefix. If an imported name collides with an existing name in the current module, the linter/type checker must emit an `import-collision` error.

---

## 2. Built-in Types and Operations

### 2.1. Scalar Types

| Type | Literal | Operations | Notes |
|------|---------|------------|-------|
| `int` | `42` | `+`, `-`, `*`, `/` (integer division), `%` (remainder), `=`, `!=`, `<`, `>`, `<=`, `>=` | 64-bit signed integer. Division by zero — runtime panic (`error: divide-by-zero`). Negative numbers in code are written via unary minus: `(- 0 7)`. |
| `float` | `3.14` | `+`, `-`, `*`, `/`, `=`, `!=`, `<`, `>`, `<=`, `>=` | 64-bit IEEE 754. Division by zero — `inf` / `-inf` (IEEE behavior), not an error. Negative numbers in code are written via unary minus: `(- 0 3.5)`. |
| `bool` | `true`, `false` | `not`, `=` | `not` is an ordinary function. `and` and `or` are special forms with mandatory short-circuiting. |
| `str` | `"hello"` | `string-append` (concatenation), `=`, `!=`, `<` (lexicographic), `int->str`, `str->int`, `float->str`, `str->float`, `string-split`, `string-length` | UTF-8. Immutable. |
| `unit` | `unit` | No operations | Type with the single value `unit`. Used for side-effect functions. |

### 2.2. Composite Types

#### `List` — homogeneous list

```lupus
(List int)        ;; type
(list 1 2 3)      ;; constructor
```

| Operation | Signature | Description |
|-----------|-----------|-------------|
| `list` | `(Func a ... (List a))` | Constructor. Accepts 0+ elements of the same type. |
| `cons` | `(Func a (List a) (List a))` | Prepends an element. |
| `head` | `(Func (List a) (Option a))` | First element or `none`. |
| `tail` | `(Func (List a) (Option (List a)))` | List without head or `none`. |
| `length` | `(Func (List a) int)` | Number of elements. |
| `nth` | `(Func (List a) int (Option a))` | Element by index (0-based) or `none`. |
| `empty?` | `(Func (List a) bool)` | Emptiness check. |
| `list-map` | `(Func (List a) (Func a b) (List b))` | Transforms each element. |
| `list-filter` | `(Func (List a) (Func a bool) (List a))` | Filtering. |
| `list-fold` | `(Func (List a) b (Func b a b) b)` | Left fold. `(list-fold xs init (lambda (acc x) ...))` |

#### `Tuple` — heterogeneous fixed-length tuple

```lupus
(Tuple int str bool)   ;; type
(tuple 1 "a" true)     ;; constructor
```

**Element access:** In v0.1 tuple elements are accessed **exclusively via pattern matching** (including the `(Tuple a b c)` pattern). Dynamic indexing (`tuple-nth`) is not supported because static typing cannot infer the element type from a dynamic index.

```lupus
(define t (tuple 1 "hello" true))
(match t
  ((Tuple a b c) (print b)))   ;; b has type str
```

| Operation | Signature | Description |
|-----------|-----------|-------------|
| `tuple` | `(Func a b ... (Tuple a b ...))` | Constructor. Arity is fixed by the type. |

#### `Map` — associative array

```lupus
(Map str int)          ;; type
(map (tuple "a" 1) (tuple "b" 2))  ;; constructor from pairs
```

| Operation | Signature | Description |
|-----------|-----------|-------------|
| `map` | `(Func (Tuple k v) ... (Map k v))` | Constructor from pairs (tuple key value). |
| `map-get` | `(Func (Map k v) k (Option v))` | Get value by key. |
| `map-set` | `(Func (Map k v) k v (Map k v))` | Returns a new Map (immutability). |
| `map-keys` | `(Func (Map k v) (List k))` | List of keys. |
| `map-values` | `(Func (Map k v) (List v))` | List of values. |
| `map-has?` | `(Func (Map k v) k bool)` | Key presence check. |

#### `Tensor` — multi-dimensional array (for ML)

```lupus
(Tensor)               ;; type without parameters in v0.1
```

| Operation | Signature | Description |
|-----------|-----------|-------------|
| `tensor` | `(Func (List int) (List float) Tensor)` | Create from shape and flat data. |
| `tensor-shape` | `(Func Tensor (List int))` | Dimensions. |
| `tensor-add` | `(Func Tensor Tensor (Result Tensor str))` | Element-wise addition (shape check). |
| `tensor-mul` | `(Func Tensor Tensor (Result Tensor str))` | Matrix multiplication. |
| `tensor-transpose` | `(Func Tensor Tensor)` | Transposition. |

#### `Func` — function type

```lupus
(Func int int int)     ;; (int, int) -> int
```

Functions are first-class values. Closures are supported via lexical environment.

### 2.3. Algebraic Types

#### `Option` — presence/absence of a value

```lupus
(Option int)           ;; type
none                   ;; constructor for any Option
(some 42)              ;; constructor with a value
```

| Constructor | Contents | Pattern |
|-------------|----------|---------|
| `none` | nothing | `(none)` |
| `some` | 1 value of type `a` | `((some value))` |

#### `Result` — success or failure

```lupus
(Result int str)       ;; success: int, error: str
(success 42)           ;; success constructor
(failure "error msg")  ;; failure constructor
```

| Constructor | Contents | Pattern |
|-------------|----------|---------|
| `success` | 1 value of type `a` | `((success value))` |
| `failure` | 1 value of type `e` | `((failure err))` |

**Rule:** Functions that may "fail" must return `Option` or `Result`. `Result` is mandatory if error information needs to be passed. `Option` is used when the mere absence of a value is sufficient.

### 2.4. User-defined and Opaque Types

**User-defined types (structs:** When defining `(defstruct point (x int) (y int))`, the name `point` automatically becomes valid in `type_expr`. The constructor `(point 10 20)` and accessors `(point-x p)`, `(point-y p)` are generated automatically.

**User-defined generics:**
```lupus
(defstruct (Node a)
  (value a)
  (left (Option (Node a)))
  (right (Option (Node a))))

(define tree (Node 42 (some (Node 10 none none)) none))
```

**Opaque types:** Types such as `socket`, `task`, `channel`, declared in FFI modules, are also available in `type_expr` via `IDENTIFIER`. Their internals are hidden; operations on them are possible only through module functions. The `channel` type is a built-in parameterized opaque type: `(channel str)`.

---

## 3. Standard Library (API)

### 3.1. Module `core` — auto-import

All `core` functions are available without a prefix. Redefining a `core` function name in user code is an error (linter code `core-shadowing`).

| Function | Signature | Description | Example |
|----------|-----------|-------------|---------|
| `+` | `(Func int int int)` or `(Func float float float)` | Addition. Cannot mix int and float. | `(+ 2 3)` → `5` |
| `-` | `(Func int int int)` or `(Func float float float)` | Subtraction. | `(- 5 2)` → `3` |
| `*` | `(Func int int int)` or `(Func float float float)` | Multiplication. | `(* 3 4)` → `12` |
| `/` | `(Func int int int)` | Integer division. | `(/ 7 2)` → `3` |
| `/` | `(Func float float float)` | Float division. | `(/ 7.0 2.0)` → `3.5` |
| `%` | `(Func int int int)` | Remainder. | `(% 7 2)` → `1` |
| `=` | `(Func a a bool)` | Equality. Works for all types except `Func`. For opaque types (socket, task, channel) — identity comparison. | `(= 1 1)` → `true` |
| `!=` | `(Func a a bool)` | Inequality. | `(!= 1 2)` → `true` |
| `<` | `(Func int int bool)` or `(Func float float bool)` or `(Func str str bool)` | Less than. | `(< 1 2)` → `true` |
| `>` | same as `<` | Greater than. | `(> 2 1)` → `true` |
| `<=` | same as `<` | Less than or equal. | `(<= 2 2)` → `true` |
| `>=` | same as `<` | Greater than or equal. | `(>= 2 2)` → `true` |
| `not` | `(Func bool bool)` | Logical NOT. | `(not true)` → `false` |
| `print` | `(Func str unit)` | Print string to stdout with newline. | `(print "hello")` |
| `string-append` | `(Func str ... str)` | Concatenate 1+ strings. | `(string-append "a" "b")` → `"ab"` |
| `string-length` | `(Func str int)` | String length in Unicode code points. | `(string-length "abc")` → `3` |
| `string-split` | `(Func str str (List str))` | Split string by delimiter. | `(string-split "a,b,c" ",")` → `(list "a" "b" "c")` |
| `int->str` | `(Func int str)` | Convert int → str. | `(int->str 42)` → `"42"` |
| `float->str` | `(Func float str)` | Convert float → str. | `(float->str 3.14)` → `"3.14"` |
| `str->int` | `(Func str (Option int))` | Parse int. | `(str->int "42")` → `(some 42)` |
| `str->float` | `(Func str (Option float))` | Parse float. | `(str->float "3.14")` → `(some 3.14)` |
| `list` | `(Func a ... (List a))` | List constructor. | `(list 1 2 3)` |
| `tuple` | `(Func a b ... (Tuple a b ...))` | Tuple constructor. | `(tuple 1 "a")` |
| `map` | `(Func (Tuple k v) ... (Map k v))` | Map constructor. | `(map (tuple "a" 1))` |
| `range` | `(Func int int (List int))` | Range [start, end). | `(range 0 3)` → `(list 0 1 2)` |
| `assert` | `(Func bool unit)` | Check condition. If `false` — runtime panic. | `(assert (= 1 1))` |
| `sleep` | `(Func int unit)` | Sleep for N milliseconds (synchronous). | `(sleep 1000)` |
| `exit` | `(Func int unit)` | Exit program with code. | `(exit 0)` |
| `cons` | `(Func a (List a) (List a))` | Prepend to list. | `(cons 0 (list 1 2))` → `(list 0 1 2)` |
| `head` | `(Func (List a) (Option a))` | First element. | `(head (list 1 2))` → `(some 1)` |
| `tail` | `(Func (List a) (Option (List a)))` | Tail of list. | `(tail (list 1 2))` → `(some (list 2))` |
| `length` | `(Func (List a) int)` | List length. | `(length (list 1 2 3))` → `3` |
| `nth` | `(Func (List a) int (Option a))` | Element by index. | `(nth (list 1 2) 0)` → `(some 1)` |
| `empty?` | `(Func (List a) bool)` | Emptiness check. | `(empty? (list))` → `true` |
| `list-map` | `(Func (List a) (Func a b) (List b))` | Transformation. | `(list-map (list 1 2) (lambda (x) (* x 2)))` |
| `list-filter` | `(Func (List a) (Func a bool) (List a))` | Filtering. | `(list-filter (list 1 2 3) (lambda (x) (> x 1)))` |
| `list-fold` | `(Func (List a) b (Func b a b) b)` | Fold. | `(list-fold (list 1 2 3) 0 (lambda (acc x) (+ acc x)))` → `6` |
| `map-get` | `(Func (Map k v) k (Option v))` | Get from Map. | `(map-get m "key")` |
| `map-set` | `(Func (Map k v) k v (Map k v))` | Write to Map. | `(map-set m "key" 42)` |
| `map-keys` | `(Func (Map k v) (List k))` | Map keys. | `(map-keys m)` |
| `map-values` | `(Func (Map k v) (List v))` | Map values. | `(map-values m)` |
| `map-has?` | `(Func (Map k v) k bool)` | Key check. | `(map-has? m "key")` |
| `tensor` | `(Func (List int) (List float) Tensor)` | Create tensor. | `(tensor (list 2 3) (list 1.0 2.0 ...))` |
| `tensor-shape` | `(Func Tensor (List int))` | Tensor shape. | `(tensor-shape t)` |
| `tensor-add` | `(Func Tensor Tensor (Result Tensor str))` | Tensor addition. | `(tensor-add t1 t2)` |
| `tensor-mul` | `(Func Tensor Tensor (Result Tensor str))` | Tensor multiplication. | `(tensor-mul t1 t2)` |
| `tensor-transpose` | `(Func Tensor Tensor)` | Transposition. | `(tensor-transpose t)` |

**Note:** `and` and `or` are not included in the table above because they are special built-in forms with short-circuiting (see section 1.3).

### 3.2. Module `senko` (math)

Prefix: `math/`. Import: `(import (senko math))`.

| Function | Signature | Description | Example |
|----------|-----------|-------------|---------|
| `math/pi` | `float` | Constant π. | `math/pi` → `3.1415926535...` |
| `math/e` | `float` | Constant e. | `math/e` → `2.7182818284...` |
| `math/sqrt` | `(Func float float)` | Square root. | `(math/sqrt 25.0)` → `5.0` |
| `math/sin` | `(Func float float)` | Sine (radians). | `(math/sin math/pi)` → `0.0` |
| `math/cos` | `(Func float float)` | Cosine (radians). | `(math/cos 0.0)` → `1.0` |
| `math/tan` | `(Func float float)` | Tangent. | `(math/tan 0.0)` → `0.0` |
| `math/log` | `(Func float float)` | Natural logarithm. | `(math/log math/e)` → `1.0` |
| `math/log10` | `(Func float float)` | Base-10 logarithm. | `(math/log10 100.0)` → `2.0` |
| `math/pow` | `(Func float float float)` | Exponentiation. | `(math/pow 2.0 3.0)` → `8.0` |
| `math/abs` | `(Func float float)` | Absolute value. | `(math/abs -3.0)` → `3.0` |
| `math/floor` | `(Func float float)` | Floor. | `(math/floor 3.7)` → `3.0` |
| `math/ceil` | `(Func float float)` | Ceiling. | `(math/ceil 3.2)` → `4.0` |
| `math/round` | `(Func float float)` | Round to nearest. | `(math/round 3.5)` → `4.0` |
| `math/min` | `(Func float float float)` | Minimum of two. | `(math/min 1.0 2.0)` → `1.0` |
| `math/max` | `(Func float float float)` | Maximum of two. | `(math/max 1.0 2.0)` → `2.0` |

### 3.3. Module `texas` (net)

Prefix: `net/`. Import: `(import (texas net))` or `(import (texas net) :as net)`.

| Function | Signature | Description | Example |
|----------|-----------|-------------|---------|
| `net/tcp-connect` | `(Func str int (Result socket str))` | TCP connection. host, port. | `(net/tcp-connect "example.com" 80)` |
| `net/tcp-listen` | `(Func int (Result socket str))` | TCP server, listen on port. | `(net/tcp-listen 8080)` |
| `net/accept` | `(Func socket (Result socket str))` | Accept connection (server). | `(net/accept sock)` |
| `net/send` | `(Func socket str (Result int str))` | Send string. Returns bytes sent. | `(net/send sock "GET / HTTP/1.0\r\n")` |
| `net/recv` | `(Func socket int (Result str str))` | Receive up to N bytes. | `(net/recv sock 4096)` |
| `net/recv-line` | `(Func socket (Result str str))` | Receive string until `\n`. | `(net/recv-line sock)` |
| `net/close` | `(Func socket unit)` | Close socket. | `(net/close sock)` |
| `net/udp-bind` | `(Func int (Result socket str))` | UDP socket. | `(net/udp-bind 53)` |
| `net/udp-send-to` | `(Func socket str int str (Result int str))` | UDP send. | `(net/udp-send-to sock "host" 53 "data")` |
| `net/udp-recv-from` | `(Func socket int (Result (Tuple str int str) str))` | UDP receive. Returns `(host, port, data)`. | `(net/udp-recv-from sock 1024)` |

**Type `socket`:** Opaque type. Cannot be created directly from Lupus, only through `texas` functions.

### 3.4. Module `kaltsit` (file)

Prefix: `file/`. Import: `(import (kaltsit file))` or `(import (kaltsit file) :as file)`.

| Function | Signature | Description | Example |
|----------|-----------|-------------|---------|
| `file/read-file` | `(Func str (Result str str))` | Read entire file into a string. | `(file/read-file "data.txt")` |
| `file/write-file` | `(Func str str (Result unit str))` | Write string to file (overwrite). | `(file/write-file "out.txt" "hello")` |
| `file/append-file` | `(Func str str (Result unit str))` | Append to file. | `(file/append-file "log.txt" "line\n")` |
| `file/exists?` | `(Func str bool)` | Existence check. | `(file/exists? "data.txt")` |
| `file/is-dir?` | `(Func str bool)` | Is directory. | `(file/is-dir? "/tmp")` |
| `file/list-dir` | `(Func str (Result (List str) str))` | List files in directory. | `(file/list-dir "/tmp")` |
| `file/mkdir` | `(Func str (Result unit str))` | Create directory. | `(file/mkdir "newdir")` |
| `file/delete` | `(Func str (Result unit str))` | Delete file. | `(file/delete "old.txt")` |
| `file/size` | `(Func str (Result int str))` | File size in bytes. | `(file/size "data.txt")` |

### 3.5. Module `amiya` (async)

Prefix: `async/`. Import: `(import (amiya async))` or `(import (amiya async) :as async)`.

| Function | Signature | Description | Example |
|----------|-----------|-------------|---------|
| `async/spawn` | `(Func (Func unit) task)` | Run function in a separate thread/task. **The argument function must return `unit`.** | `(async/spawn (lambda () (print "hi")))` |
| `async/sleep` | `(Func int unit)` | Asynchronous sleep (does not block other tasks). | `(async/sleep 1000)` |
| `async/channel` | `(Func (channel a))` | Create a typed channel. | `(async/channel)` |
| `async/send` | `(Func (channel a) a unit)` | Send to channel. | `(async/send ch 42)` |
| `async/recv` | `(Func (channel a) a)` | **Blocking** receive from channel. | `(async/recv ch)` |
| `async/recv-timeout` | `(Func (channel a) int (Option a))` | Receive with timeout (ms). `none` on timeout. | `(async/recv-timeout ch 1000)` |
| `async/wait` | `(Func task unit)` | Wait for task completion. | `(async/wait t)` |
| `async/wait-all` | `(Func (List task) unit)` | Wait for all tasks. | `(async/wait-all (list t1 t2))` |

**Types `task` and `channel`:** Opaque types. Created only through `amiya`. The `channel` type is a built-in parameterized opaque type: `(channel str)`.

**Semantics:** In the Python prototype, `spawn` uses `threading.Thread`. Channels are implemented via `queue.Queue`. `async/recv` blocks the thread until data arrives (no busy-wait). `async/recv-timeout` uses `queue.get(timeout=ms/1000)`.

### 3.6. Module `w` (test)

The `assert` form is a **built-in special form** of the language (not a `core` module function) that causes a runtime panic on `false`. The `test` form is a **special form** of the language (built into the parser), not a function. Importing module `w` is not required to use `(test ...)` or `(assert ...)`. However, the functions `test/assert-eq`, `test/assert-true`, `test/assert-false` are available after `(import (w test))` or via the `test/` prefix.

| Function | Signature | Description | Example |
|----------|-----------|-------------|---------|
| `test` | Special form | Test declaration (see section 5). | `(test "name" (assert ...))` |
| `test/assert-eq` | `(Func a a unit)` | Equality assert with diff output. | `(test/assert-eq 2 (+ 1 1))` |
| `test/assert-true` | `(Func bool unit)` | Assert true. | `(test/assert-true (> 2 1))` |
| `test/assert-false` | `(Func bool unit)` | Assert false. | `(test/assert-false (= 1 2))` |
| `test/run` | `(Func (List str) int)` | Run tests by name. | `(test/run (list "add-works"))` |
| `test/run-all` | `(Func int)` | Run all tests in the file. | `(test/run-all)` |

---

## 4. FFI Specification

### 4.1. General Principles

- FFI allows implementing standard library modules in the host language (Python in v0.1).
- Lupus code cannot call Python directly. Instead, Python functions are **registered** as Lupus symbols through the FFI mechanism.
- The FFI directive is **declarative**: it tells the interpreter where to find the implementation.
- The directive must be at the **beginning of the module file** (before any `define` or `import`).
- **All calls to Python functions through FFI are wrapped in `try...except`** (see section 4.5). Unhandled Python exceptions must not crash the Lupus interpreter.

### 4.2. FFI Directive

```lupus
(#lupus ffi python "<module.path>")
```

- `python` — host language (in v0.1 only `python`).
- `<module.path>` — Python import path (e.g., `lupus_modules.senko`).
- If the file is a **user script**, not a module, the FFI directive is forbidden (linter error `invalid-directive`).

### 4.3. Python Module Requirements

A Python module must export a dictionary `__lupus_exports__`:

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

Entry format: `{"lupus_name": ("type_string", callable)}`

**Requirements for callable:**
1. Accepts as many arguments as specified in the type.
2. Returns a value matching the type.
3. If the function returns `Result`, it must return a tuple `("success", value)` or `("failure", error_msg)`.
4. If the function returns `Option`, it must return `None` (converted to `none`) or any value (converted to `(some value)`).
5. For constants (like `pi`), the callable is a thunk (zero-argument function), invoked once at module load time.

### 4.4. FFI Loading Process

1. The interpreter encounters `(import (senko math))`.
2. Resolves the module path (see section 4.6).
3. If the directive `(#lupus ffi python "lupus_modules.senko")` is found, executes `importlib.import_module("lupus_modules.senko")`.
4. Reads `__lupus_exports__`.
5. For each key, creates an internal Lupus `Func` object with:
   - `name`: `math/<key>`
   - `type`: parsed type from the string
   - `impl`: Python callable (wrapped in a type-conversion adapter)
6. Registers in the `math` module environment.

### 4.5. Error Handling in FFI

All calls to Python functions through FFI **must** be wrapped in `try...except`:

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

If a Python function crashes with an unhandled exception:
- The Lupus interpreter **must not** crash.
- A JSON error with code `ffi-runtime-error` must be generated.
- For functions returning `Result`, the exception may be translated into `(failure "...")`.
- For functions not returning `Result`/`Option`, a runtime panic is generated.

### 4.6. Module Name Resolution

For `(import (senko math))`:
1. The interpreter looks for `stdlib/senko/math.lupus` (or `senko/math.lupus` in `LUPUS_PATH`).
2. If the file is found, parses it as a Lupus module.
3. If the file contains an FFI directive, loads the Python implementation.
4. All `define-public` from the file and all FFI exports become available under the `math/` prefix.

### 4.7. Full Module Binding Example for `senko`

**Lupus module file:** `stdlib/senko/math.lupus`
```lupus
;; stdlib/senko/math.lupus
(#lupus ffi python "lupus_modules.senko")

;; Additional pure Lupus functions can be added here:
(define-public (deg->rad deg) -> float
  (* math/pi (/ deg 180.0)))
```

**Python implementation file:** `lupus_modules/senko.py`
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

### 4.8. FFI Errors

| Error Code | Condition |
|------------|-----------|
| `ffi-module-not-found` | Python module not found |
| `ffi-export-missing` | Key not found in `__lupus_exports__` |
| `ffi-type-mismatch` | Python return value does not match the declared type |
| `ffi-arity-mismatch` | Python function accepted a different number of arguments |
| `ffi-runtime-error` | Python exception during function execution (ZeroDivisionError, etc.) |

---

## 5. Test Format

### 5.1. Test Declaration

Tests are declared inside `.lupus` files using the special `test` form:

```lupus
(test "unique-test-name"
  expr1
  expr2
  ...)
```

- `"unique-test-name"` — string identifier. Must be unique within the file. Duplication is a linter error `test-name-duplicate`.
- `expr1, expr2, ...` — test body (sequence of expressions). Usually contains `assert`.
- Tests **do not run** during normal program execution (`lupus run`). They run only during `lupus test <file>`.
- Tests **do not affect** program output during normal execution.

### 5.2. Running Tests

```bash
lupus test file.lupus          # run all tests in the file
lupus test file.lupus --name "add-works"  # run a specific test
lupus test dir/                # recursively run all .lupus files
```

### 5.3. Output Format

The interpreter outputs JSON Lines (JSONL):

```json
{"type": "test-start", "name": "add-works", "file": "calc.lupus"}
{"type": "test-pass", "name": "add-works", "file": "calc.lupus", "duration_ms": 0.5}
{"type": "test-fail", "name": "safe-divide-zero", "file": "calc.lupus", "error": {"code": "assert-failed", "location": {"line": 15, "col": 3}, "message": "Assertion failed: (= (safe-divide 10 0) none)"}, "duration_ms": 1.2}
{"type": "test-summary", "total": 5, "passed": 4, "failed": 1, "file": "calc.lupus"}
```

### 5.4. Test Rules

1. **Isolation:** Each test runs in a **fresh environment**. `define` and `define-mutable` from one test are not visible to other tests. `define-public` from the main file code are visible to all tests.
2. **Order:** Tests execute in the order of declaration in the file.
3. **Failure:** If `assert` fails, the test immediately aborts (fail-fast). Other tests continue running.
4. **Side effects:** `print` inside a test is suppressed by default (output only with `--verbose` flag).

---

## 6. Error Format (JSON)

### 6.1. General Structure

All errors, warnings, and runtime panics are output in strict JSON:

```json
{
  "severity": "error" | "warning" | "info",
  "phase": "lex" | "parse" | "type" | "lint" | "runtime" | "ffi",
  "code": "unique-code",
  "message": "Human-readable description",
  "location": {
    "file": "path/to/file.lupus",
    "line": 12,
    "col": 5,
    "span": {"start": 120, "end": 135}
  },
  "hint": "Possible fix or explanation",
  "context": {
    "line_text": "  (define x \"hello\")",
    "token": "x"
  }
}
```

### 6.2. Full Error Catalog

| Code | Phase | Message | Example |
|------|-------|---------|---------|
| `unknown-token` | lex | Unknown token | `@` |
| `unclosed-string` | lex | Unclosed string | `"hello` |
| `unclosed-comment` | lex | Unclosed block comment | `#| ...` |
| `unexpected-token` | parse | Unexpected token | `(define 1 2)` — 1 is not IDENTIFIER |
| `missing-rparen` | parse | Missing closing parenthesis | `(define x 10` |
| `type-mismatch` | type | Type mismatch | `(+ 1 "a")` |
| `unknown-identifier` | type | Unknown identifier | `(foo 1)` — foo is not defined |
| `unknown-module` | type | Unknown module | `(import (unknown mod))` |
| `unknown-module-symbol` | type | Unknown module symbol | `(math/unknown 1)` |
| `arity-mismatch` | type | Wrong number of arguments | `(+ 1)` — expects 2 |
| `missing-return-type` | type | Missing return type annotation on public function | `(define-public (foo x) x)` — hard error |
| `missing-param-type` | type | Missing parameter type annotation on public function | `(define-public (foo x) -> int x)` — hard error |
| `immutable-assignment` | type | Attempt to set! an immutable variable | `(set! x 1)` where x is from `define` |
| `duplicate-definition` | lint | Duplicate definition | two `(define x ...)` |
| `unused-variable` | lint | Unused variable | `(define y 10)` with no use of y |
| `core-shadowing` | lint | Redefinition of a core function | `(define + 1)` |
| `test-name-duplicate` | lint | Duplicate test name | two `(test "foo" ...)` |
| `test-in-function` | lint | Test declared inside a function | `(define (f) (test "x" ...))` |
| `divide-by-zero` | runtime | Division by zero | `(/ 1 0)` |
| `assert-failed` | runtime | Assert returned false | `(assert false)` |
| `index-out-of-bounds` | runtime | Index out of range | `(nth (list 1) 5)` |
| `match-non-exhaustive` | type | Non-exhaustive pattern matching | `(match (some 1) ((none) 0))` — missing `some` |
| `match-redundant` | lint | Redundant pattern | Pattern after `else` or `_` |
| `invalid-directive` | lint | Invalid directive | `(#lupus unknown)` |
| `duplicate-struct-field` | type | Duplicate field name in struct | `(defstruct p (x int) (x float))` |
| `import-collision` | lint | Name collision on `:all` import | `(import (senko math) :all)` when `pi` already exists |
| `ffi-module-not-found` | ffi | Python module not found | `(#lupus ffi python "missing")` |
| `ffi-type-mismatch` | ffi | FFI type mismatch | Python function returned `str` instead of `float` |
| `ffi-arity-mismatch` | ffi | Wrong FFI arity | Python function accepted 3 instead of 2 |
| `ffi-runtime-error` | ffi | Exception in Python function | Division by zero in Python module |
| `value-restriction` | type | Value restriction violation | Polymorphic mutable without explicit annotation |

---

## 7. AST Serialization (JSON)

### 7.1. Purpose

AST must be serializable to JSON for:
- Passing between layers (Parser → Typechecker → Interpreter).
- Saving to a dataset for LLM training.
- Reading by a Rust version in stage 2.

### 7.2. Node Format

Every AST node is an object with mandatory fields:

```json
{
  "kind": "node_type",
  "span": {"file": "f.lupus", "start": 120, "end": 135, "line": 5, "col": 2},
  "data": { ... }
}
```

### 7.3. Node Types

| `kind` | `data` Fields | Description |
|--------|---------------|-------------|
| `Program` | `toplevels: [Node]` | Root |
| `Define` | `name: str`, `mutable: bool`, `public: bool`, `constant: bool`, `value: Node`, `type_annotation: Type \| null` | Definition |
| `SetBang` | `name: str`, `value: Node` | Mutable assignment |
| `Lambda` | `params: [Param]`, `body: [Node]`, `return_type: Type \| null` | Anonymous function |
| `Param` | `name: str`, `type_annotation: Type \| null` | Function parameter |
| `If` | `condition: Node`, `then_branch: [Node]`, `else_branch: [Node]` | Conditional |
| `Cond` | `clauses: [(condition: Node, body: [Node])]` | Multi-branch conditional |
| `Match` | `expr: Node`, `clauses: [(pattern: Pattern, body: [Node])]` | Pattern matching |
| `PatternVar` | `name: str` | Pattern variable |
| `PatternWildcard` | `{}` | Wildcard pattern `_` |
| `PatternLiteralNone` | `{}` | Pattern for `none` |
| `PatternConstructor` | `constructor: str`, `args: [Pattern]` | Constructor pattern |
| `PatternTuple` | `args: [Pattern]` | Tuple pattern `(Tuple a b c)` |
| `IfLet` | `binding: (name: str, expr: Node)`, `then_branch: [Node]`, `else_branch: [Node]` | If-let |
| `While` | `condition: Node`, `body: [Node]` | While loop |
| `For` | `var: str`, `iter: Node`, `body: [Node]` | For loop |
| `Loop` | `body: [Node]` | Infinite loop |
| `Import` | `package: str`, `module: str`, `alias: str \| null`, `import_all: bool` | Import |
| `DefStruct` | `name: str`, `type_params: [str]`, `fields: [(name: str, type: Type)]` | Struct |
| `Test` | `name: str`, `body: [Node]` | Test |
| `Assert` | `expr: Node` | Assert |
| `Directive` | `name: str`, `args: [str]` | Directive |
| `Call` | `func: Node`, `args: [Node]` | Function call |
| `Identifier` | `name: str` | Identifier |
| `QualifiedName` | `module: str`, `name: str` | Qualified name |
| `LiteralInt` | `value: int` | Integer |
| `LiteralFloat` | `value: float` | Float |
| `LiteralBool` | `value: bool` | Bool |
| `LiteralStr` | `value: str` | String |
| `LiteralNone` | `{}` | none |
| `LiteralUnit` | `{}` | unit |
| `Type` | `kind: str`, `params: [Type]` | Type expression |

### 7.4. Serialization Example

**Source code:**
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

### 7.5. Serialization Requirements

- **Deterministic:** Field order in JSON is fixed (kind, span, data).
- **Span is mandatory:** Every node contains a span for precise diagnostics.
- **Type annotations:** If a type is not explicitly specified, `type_annotation` and `return_type` are `null`.
- **No comments:** Comments are not included in the AST (ignored at the lexing stage).
- **Body as array:** All bodies (functions, branches, loops) are serialized as arrays of nodes `[Node]`.

---

## 8. Implementation Recommendations (Python)

### 8.1. Layered Architecture

```
┌─────────────────────────────────────────┐
│  CLI (lupus run / lupus test / lupus check) │
├─────────────────────────────────────────┤
│  Frontend                               │
│  ├── Lexer (lark/ply or hand-written)   │
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

### 8.2. Lexer

**Recommended library:** `lark` (with EBNF grammar from section 1) or a hand-written lexer using `re`.

**Requirements:**
- Unicode support for strings and identifiers.
- Position tracking (line, col, start, end) for every token.
- Comments are turned into a `COMMENT` token and dropped by the parser, or ignored by the lexer.
- `none` and `unit` are lexed as separate tokens `LITERAL_NONE` and `LITERAL_UNIT`, not as `IDENTIFIER` or `KEYWORD`.
- `true`/`false` are lexed as `BOOLEAN`.

**Hand-written lexer example (concept):**
```python
import re

TOKEN_SPEC = [
    ('COMMENT_LINE',  r';;[^\n]*'),
    ('COMMENT_BLOCK', r'#\|.*?\|#'),
    ('FLOAT',         r'\d+\.\d+([eE][-+]?\d+)?'),
    ('INTEGER',       r'\d+'),
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

### 8.3. Parser

**Recommended library:** `lark` with LALR parsing.

**Why lark:**
- Direct EBNF support.
- AST generation via `Transformer`.
- Good error diagnostics.

**Alternative:** Hand-written recursive descent (easier to control but more code).

**Key parser points:**
- `body` is parsed as a sequence of `expr` until the closing parenthesis of the current level.
- `define-public` requires `func_header` (name in parentheses with parameters) and `body`. The parser allows omitting annotations in `func_header` (per EBNF), but the type checker must generate hard errors `missing-return-type` and `missing-param-type` for any `define-public` without a full signature.
- Parameters in `define-public` may be annotated `(a int)` or bare (`a`). If bare, the type is inferred, but the type checker emits error `missing-param-type`.
- `local_define` differs from `toplevel define` only in context (the parser may use the same rules).
- `test` is allowed only at the top level.
- `defstruct` supports generics: `(defstruct (Node a) ...)`. Struct fields must have unique names; duplication is error `duplicate-struct-field`.

### 8.4. Type Checker

**Algorithm:** Simplified Hindley-Milner with limited polymorphism.

**Key decisions:**
- **Type variables:** `a`, `b`, `t1`, `t2` — for inference.
- **Unification:** Standard unification algorithm with occurs check.
- **Type environment (Gamma):** Dictionary `name -> type scheme`.
- **Polymorphism:** Let-polymorphism (generalization of types at `define` and `define-const`).
- **Value Restriction:** For `define-mutable`, polymorphism is **forbidden**. The type of a mutable variable is not generalized (monomorphic). If the type cannot be uniquely inferred, an explicit annotation is required.
- **Limitations:** No higher-order polymorphism for user-defined types in v0.1.

**`set!` behavior and type safety:**
- The `set!` form **does not change** the variable's type and **does not generalize** it.
- If a variable was declared as `int`, attempting `(set! x "str")` causes a hard `type-mismatch` error because the expected type `int` does not unify with `str`.
- `set!` searches for the variable in the current and parent environments; modifies the first mutable binding found. If the variable is found but not mutable — error `immutable-assignment`.

**Pseudocode:**
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
        # All expressions in body except the last must be unit
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
        # Check that the variable is mutable and the type matches
        var_type = env.lookup(expr.name)
        val_type = infer(expr.value, env)
        unify(var_type, val_type)
        return Type("unit")
```

**Error requirements:**
- If unification fails — emit `type-mismatch` with both sides of the unification.
- If `define-public` lacks a full annotation (return type or any parameter without a type) — hard errors `missing-return-type` or `missing-param-type`.
- If `set!` is applied to a `define` (immutable) — `immutable-assignment`.
- If `define-mutable` has a polymorphic type without annotation — `value-restriction`.

### 8.5. Interpreter

**Strategy:** Tree-walk interpreter (AST traversal).

**Environment:**
- Hierarchical dictionary with a parent link.
- `define` creates an entry in the current environment.
- `lambda` captures the current environment (closure).
- `set!` searches for the variable in the current and parent environments, modifies the first mutable binding found.

**Values:**
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
class VOpaque(Value):   # for socket, task, channel
    pass
```

**Function calls:**
- Built-in (core): Python function accepting `List[Value]`, returning `Value`.
- User-defined: Create a new environment, bind parameters, execute body (sequentially, result is the last expression).
- FFI: Adapter converting `Value` ↔ Python types.

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
        # Reverse conversion with type checking
        ...
```

### 8.7. Module `amiya` (async) — Implementation

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
        return self.q.get()  # blocking

    def recv_timeout(self, timeout_ms):
        try:
            return self.q.get(timeout=timeout_ms / 1000.0)
        except queue.Empty:
            return None
```

### 8.8. CLI

```bash
lupus run <file.lupus> [args...]     # execute program
lupus test <file.lupus>               # run tests
lupus check <file.lupus>              # lint + types (no execution)
lupus ast <file.lupus>                # output AST as JSON
lupus eval <expr>                     # evaluate single expression (REPL mode)
```

### 8.9. Dependencies (requirements.txt)

```
lark>=1.1.0
click>=8.0.0
```

---

## 9. Program Examples

### 9.1. Circle Area Calculator

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

### 9.2. HTTP Client (GET Request)

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

### 9.3. Async Timer with Channels

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

### 9.4. CSV File Processing

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

;; Run
(match (sum-lines "numbers.txt")
  ((success total) (print (string-append "Sum: " (int->str total))))
  ((failure err) (print (string-append "Error: " err))))

(test "parse-int-lines"
  (define result (parse-int-lines "1\n2\n3"))
  (assert (= (length result) 3)))
```

### 9.5. Math Unit Testing

```lupus
;; math_test.lupus
(import (senko math))
(import (w test))

(define-public (factorial (n int)) -> int
  (if (<= n 1)
    1
    (* n (factorial (- n 1)))))

(define-public (fibonacci (n int)) -> int
  (if (<= n 1)
    n
    (+ (fibonacci (- n 1)) (fibonacci (- n 2)))))

;; --- Tests ---
(test "factorial-0"
  (test/assert-eq 1 (factorial 0)))

(test "factorial-5"
  (test/assert-eq 120 (factorial 5)))

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

### 9.6. Data Structures, Map, and Generics

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

## 10. LLM Checklist

### 10.1. Files the Code Must Generate

| # | File | Description | Priority |
|---|------|-------------|----------|
| 1 | `lupus/lexer.py` | Tokenizer (hand-written or lark) | Mandatory |
| 2 | `lupus/parser.py` | Parser building a JSON-serializable AST | Mandatory |
| 3 | `lupus/ast_nodes.py` | AST node classes with `.to_json()` method | Mandatory |
| 4 | `lupus/types.py` | Type definitions, unification, type environment | Mandatory |
| 5 | `lupus/typechecker.py` | Type inference (Hindley-Milner), annotation checks | Mandatory |
| 6 | `lupus/linter.py` | Style checks, unused variables, duplication | Mandatory |
| 7 | `lupus/interpreter.py` | Tree-walk interpreter, environments, closures | Mandatory |
| 8 | `lupus/values.py` | Runtime value representations | Mandatory |
| 9 | `lupus/environment.py` | Hierarchical environments (scopes) | Mandatory |
| 10 | `lupus/ffi.py` | Python module loading, function wrapping, exception handling | Mandatory |
| 11 | `lupus/errors.py` | JSON error formatting | Mandatory |
| 12 | `lupus/cli.py` | Entry point: `lupus run`, `lupus test`, `lupus check`, `lupus ast` | Mandatory |
| 13 | `lupus/core_builtins.py` | Implementation of all `core` functions (+, -, *, list, map, assert, ...) | Mandatory |
| 14 | `lupus_modules/senko.py` | FFI implementation of the math module | Mandatory |
| 15 | `lupus_modules/texas.py` | FFI implementation of the net module (via socket) | Mandatory |
| 16 | `lupus_modules/kaltsit.py` | FFI implementation of the file module (via os, pathlib) | Mandatory |
| 17 | `lupus_modules/amiya.py` | FFI implementation of the async module (via threading, queue) | Mandatory |
| 18 | `lupus_modules/w.py` | FFI implementation of the test module (or built into interpreter) | Mandatory |
| 19 | `stdlib/core.lupus` | Core module definitions and documentation (if partially in Lupus) | Optional |
| 20 | `tests/test_lexer.py` | Lexer unit tests | Mandatory |
| 21 | `tests/test_parser.py` | Parser unit tests | Mandatory |
| 22 | `tests/test_typechecker.py` | Type checker unit tests | Mandatory |
| 23 | `tests/test_interpreter.py` | Interpreter unit tests | Mandatory |
| 24 | `tests/test_ffi.py` | FFI unit tests | Mandatory |
| 25 | `tests/integration/` | Integration tests: all examples from section 9 | Mandatory |
| 26 | `docs/spec.md` | Copy of this specification | Mandatory |
| 27 | `docs/grammar.ebnf` | Formal grammar | Mandatory |
| 28 | `docs/api.md` | Standard library API documentation | Mandatory |
| 29 | `docs/tutorial.md` | LLM tutorial (how to write Lupus) | Mandatory |
| 30 | `Makefile` / `pyproject.toml` | Build, install, test runner | Mandatory |

### 10.2. Code Quality Requirements

1. **Test coverage:** At least 80% line coverage for `lexer.py`, `parser.py`, `typechecker.py`, `interpreter.py`.
2. **JSON errors:** All errors (including runtime panics) must be output in the format of section 6.
3. **AST:** The `.to_json()` method must be deterministic and match section 7.
4. **FFI:** Every Python module (`lupus_modules/*.py`) must contain `__lupus_exports__`. All calls are wrapped in `try...except`.
5. **CLI:** Support for flags `--json` (JSON-only output), `--verbose`, `--no-lint`.
6. **Documentation:** Every public function has a docstring with description and types.

### 10.3. Generation Order (Recommendation for LLM)

1. **First** AST and lexer (foundation).
2. **Then** parser + parser tests.
3. **Then** type system + type checker + tests.
4. **Then** runtime (values, environment) + interpreter + tests.
5. **Then** FFI + standard library modules.
6. **Then** CLI + integration tests.
7. **Finally** documentation.

### 10.4. Acceptance Criteria (Definition of Done)

- [ ] All examples from section 9 execute without errors (`lupus run` and `lupus test`).
- [ ] The `lupus check` command passes without errors for all `.lupus` files in `examples/`.
- [ ] `lupus ast example.lupus` outputs valid JSON that validates against the schema in section 7.
- [ ] FFI modules `senko`, `texas`, `kaltsit`, `amiya`, `w` load and work.
- [ ] The interpreter correctly handles all errors from the catalog in section 6.
- [ ] Test coverage ≥ 80% for core files.
- [ ] Python exceptions in FFI modules do not crash the interpreter (emit JSON error).

---

## Appendix A. Glossary

| Term | Description |
|------|-------------|
| **AST** | Abstract Syntax Tree. |
| **EBNF** | Extended Backus-Naur Form — grammar notation. |
| **FFI** | Foreign Function Interface — mechanism for calling host functions. |
| **Hindley-Milner** | Type inference algorithm with polymorphism. |
| **LALR** | Look-Ahead LR — parsing algorithm (used in lark). |
| **Opaque type** | Type whose internal structure is hidden from the language (socket, task). |
| **Prelude / Core** | Automatically imported set of functions. |
| **REPL** | Read-Eval-Print Loop — interactive mode. |
| **Span** | Range of positions in source code (file, line, column, bytes). |
| **Unit** | Type with the single value `unit`, used for side-effect functions. |
| **Value Restriction** | Restriction forbidding polymorphism for mutable variables in HM. |

---

*Document prepared for LLMs. All sections are self-contained and contain sufficient information for implementing the Lupus V1.0 language in Python.*