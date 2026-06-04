# LUPUS v1.2 — Final Programming Language Specification (Corrected, Final)

**Version:** 1.2 (Final Corrected Specification)  
**Date:** 2026-06-02  
**Status:** Ready for Implementation  
**Target audience:** GPT-5.5 Mimi, interpreter developers, compiler architects  

---

## Table of Contents

1. [EBNF Grammar](#1-ebnf-grammar)
2. [Built-in Types and Operations](#2-built-in-types-and-operations)
3. [Standard Library (API)](#3-standard-library-api)
4. [FFI Specification](#4-ffi-specification)
5. [Test Format](#5-test-format)
6. [Error Format (JSON)](#6-error-format-json)
7. [AST Serialization (JSON)](#7-ast-serialization-json)
8. [Implementation Guidelines (Python)](#8-implementation-guidelines-python)
9. [Program Examples](#9-program-examples)
10. [Checklist for GPT-5.5 Mimi](#10-checklist-for-gpt-55-mimi)

---

## 1. EBNF Grammar

### 1.1. Lexical rules

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

IDENTIFIER    = LETTER , { LETTER | DIGIT | "_" | "-" } ;
              (* Note: "/" is NOT part of IDENTIFIER. Qualified names are parsed at parser stage. *)

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

(* --- Symbols --- *)
LPAREN        = "(" ;
RPAREN        = ")" ;
ARROW         = "->" ;
SLASH         = "/" ;

(* --- Helpers --- *)
LETTER        = "a" .. "z" | "A" .. "Z" ;
UPPERCASE_LETTER = "A" .. "Z" ;
DIGIT         = "0" .. "9" ;
HEX_DIGIT     = DIGIT | "a" .. "f" | "A" .. "F" ;
```

### 1.2. Syntax rules

```ebnf
(* === Parser === *)

(* --- Top level --- *)

define        = LPAREN , "define" , IDENTIFIER , expr , RPAREN ;
              (* (define x 10) *)

define_mutable = LPAREN , "define-mutable" , IDENTIFIER , expr , RPAREN ;
              (* (define-mutable y 20) *)

define_const  = LPAREN , "define-const" , IDENTIFIER , expr , RPAREN ;
              (* (define-const pi 3.1416) — immutable, compile-time constant *)

define_public = LPAREN , "define-public" , func_header , body , RPAREN ;
              (* (define-public (add (a int) (b int)) -> int (+ a b)) *)
              (* Return type annotation is required. *)
              (* Parameters may be annotated: (a int) or not: a *)
              (* If a parameter has no annotation, type is inferred, but for public functions *)
              (* the type checker may require an explicit annotation (warn). *)

func_header   = LPAREN , IDENTIFIER , { param } , RPAREN , [ "->" , type_expr ] ;
              (* Function name + parameters + optional return type *)

param         = IDENTIFIER
              | LPAREN , IDENTIFIER , type_expr , RPAREN ;
              (* annotated parameter: (a int) *)

set_bang      = LPAREN , "set!" , IDENTIFIER , expr , RPAREN ;
              (* (set! x (+ x 1)) *)

lambda        = LPAREN , "lambda" , LPAREN , { param } , RPAREN , [ "->" , type_expr ] , body , RPAREN ;
              (* (lambda (x) (* x x)) *)
              (* (lambda ((x int)) -> int (* x x)) *)

(* --- Body (expression sequence) --- *)

body          = { expr } ;
              (* One or more expressions. Body result is the value of the last expression. *)
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
              | LPAREN , constructor , { pattern } , RPAREN ;
              (* (some value), (success data), (point x y) *)
              | LPAREN , "Tuple" , { pattern } , RPAREN ;
              (* (Tuple a b c) — tuple pattern matching *)

constructor   = IDENTIFIER ;
              (* none, some, success, failure, point, etc. *)

if_let        = LPAREN , "if-let" , LPAREN , IDENTIFIER , expr , RPAREN , expr , expr , RPAREN ;
              (* (if-let (value expr) then-expr else-expr) *)

(* --- Loops --- *)

while_expr    = LPAREN , "while" , expr , body , RPAREN ;
              (* (while (< i 10) (print i) (set! i (+ i 1))) *)

for_expr      = LPAREN , "for" , IDENTIFIER , "in" , expr , body , RPAREN ;
              (* (for i in (range 0 10) (print i)) *)

loop_expr     = LPAREN , "loop" , body , RPAREN ;
              (* (loop (print "tick") (sleep 1000)) *)

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
              (* All control constructs and local define are expressions *)

call_expr     = LPAREN , operator , { expr } , RPAREN
              | LPAREN , expr , { expr } , RPAREN ;
              (* Function call: (func args...) or (+ 1 2) *)

literal       = INTEGER | FLOAT | BOOLEAN | STRING | LITERAL_NONE | LITERAL_UNIT ;
              (* none and unit are separate tokens, not IDENTIFIER *)

qualified_name = IDENTIFIER , SLASH , IDENTIFIER ;
              (* math/sqrt, net/tcp-connect *)
              (* Available only for imported modules. *)

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
              | IDENTIFIER ; (* any user function or constructor *)

(* --- Types --- *)

type_expr     = "int" | "float" | "bool" | "str" | "unit"
              | IDENTIFIER    (* User types: point, person, socket, task, channel *)
              | TYPE_NAME     (* Generic parameters: a, b, T *)
              | LPAREN , "List" , type_expr , RPAREN
              | LPAREN , "Tuple" , { type_expr } , RPAREN
              | LPAREN , "Map" , type_expr , type_expr , RPAREN
              | LPAREN , "Tensor" , RPAREN
              | LPAREN , "Func" , { type_expr } , type_expr , RPAREN
              | LPAREN , "Option" , type_expr , RPAREN
              | LPAREN , "Result" , type_expr , type_expr , RPAREN
              | LPAREN , IDENTIFIER , { type_expr } , RPAREN ;
              (* User generics: (Node a), (Tree a b) *)
```

### 1.3. Grammar notes

- **Prefix notation:** All operators and function calls are strictly prefix. No infix syntax. The AST is unambiguous.
- **Indentation:** Have no syntactic meaning. Used only for readability.
- **Qualified names:** Available only for imported modules. Format: `<module-alias>/<identifier>`. `/` cannot appear in a plain IDENTIFIER.
- **Body (sequence):** Function bodies, `if` branches, `match`, and loops are a sequence of 1+ expressions. All expressions in a body are evaluated sequentially; the result is the value of the last expression. All expressions except the last must have type `unit` (checked by the type checker).
- **Local define:** `define` and `define-mutable` may be used at top level or inside any `body`. Scope runs from the definition point to the end of the current `body`.
- **Variadic functions:** Only built-in constructors `list`, `tuple`, `map`, `string-append`, and `+` (for 2+ arguments) accept a variable number of arguments. All user functions have fixed arity defined at declaration.
- **Tests:** The `test` form is allowed only at top level (`toplevel`), not inside functions.
- **Generics in defstruct:** `(defstruct (Node a) ...)` creates a parameterized type. Usage: `(Node int)`.

---

## 2. Built-in Types and Operations

### 2.1. Scalar types

| Type | Literal | Operations | Notes |
|-----|---------|----------|------------|
| `int` | `42`, `-7` | `+`, `-`, `*`, `/` (integer division), `%` (remainder), `=`, `!=`, `<`, `>`, `<=`, `>=` | 64-bit signed integer. Division by 0 — runtime panic (`error: divide-by-zero`). |
| `float` | `3.14`, `-0.5` | `+`, `-`, `*`, `/`, `=`, `!=`, `<`, `>`, `<=`, `>=` | 64-bit IEEE 754. Division by 0 — `inf` / `-inf` (IEEE behavior), not an error. |
| `bool` | `true`, `false` | `and`, `or`, `not`, `=` | Short-circuit (`and`/`or`) required. |
| `str` | `"hello"` | `string-append` (concatenation), `=`, `!=`, `<` (lexicographic), `int->str`, `str->int`, `float->str`, `str->float`, `string-split`, `string-length` | UTF-8. Immutable. |
| `unit` | `unit` | No operations | Type with single value `unit`. Used for side-effect functions. |

### 2.2. Composite types

#### `List` — homogeneous list

```lupus
(List int)        ;; type
(list 1 2 3)      ;; constructor
```

| Operation | Signature | Description |
|----------|-----------|----------|
| `list` | `(Func a ... (List a))` | Constructor. Accepts 0+ elements of one type. |
| `cons` | `(Func a (List a) (List a))` | Prepends element. |
| `head` | `(Func (List a) (Option a))` | First element or `none`. |
| `tail` | `(Func (List a) (Option (List a)))` | List without head or `none`. |
| `length` | `(Func (List a) int)` | Element count. |
| `nth` | `(Func (List a) int (Option a))` | Element by index (0-based) or `none`. |
| `empty?` | `(Func (List a) bool)` | Emptiness check. |
| `list-map` | `(Func (List a) (Func a b) (List b))` | Transform each element. |
| `list-filter` | `(Func (List a) (Func a bool) (List a))` | Filter. |
| `list-fold` | `(Func (List a) b (Func b a b) b)` | Fold (left). `(list-fold xs init (lambda (acc x) ...))` |

#### `Tuple` — heterogeneous fixed-length tuple

```lupus
(Tuple int str bool)   ;; type
(tuple 1 "a" true)     ;; constructor
```

**Element access:** In v0.1 tuple element access is **only via pattern matching** (including pattern `(Tuple a b c)`). Dynamic indexing (`tuple-nth`) is not supported because static typing cannot infer element type from a dynamic index.

```lupus
(define t (tuple 1 "hello" true))
(match t
  ((Tuple a b c) (print b)))   ;; b has type str
```

| Operation | Signature | Description |
|----------|-----------|----------|
| `tuple` | `(Func a b ... (Tuple a b ...))` | Constructor. Arity fixed by type. |

#### `Map` — associative array

```lupus
(Map str int)          ;; type
(map (tuple "a" 1) (tuple "b" 2))  ;; constructor from list of pairs
```

| Operation | Signature | Description |
|----------|-----------|----------|
| `map` | `(Func (Tuple k v) ... (Map k v))` | Constructor from (tuple key value) pairs. |
| `map-get` | `(Func (Map k v) k (Option v))` | Get value by key. |
| `map-set` | `(Func (Map k v) k v (Map k v))` | Returns new Map (immutable). |
| `map-keys` | `(Func (Map k v) (List k))` | List of keys. |
| `map-values` | `(Func (Map k v) (List v))` | List of values. |
| `map-has?` | `(Func (Map k v) k bool)` | Key membership check. |

#### `Tensor` — multidimensional array (for ML)

```lupus
(Tensor)               ;; type without parameters in v0.1
```

| Operation | Signature | Description |
|----------|-----------|----------|
| `tensor` | `(Func (List int) (List float) Tensor)` | Create from shape and flat data. |
| `tensor-shape` | `(Func Tensor (List int))` | Dimensions. |
| `tensor-add` | `(Func Tensor Tensor (Result Tensor str))` | Element-wise addition (shape check). |
| `tensor-mul` | `(Func Tensor Tensor (Result Tensor str))` | Matrix multiplication. |
| `tensor-transpose` | `(Func Tensor Tensor)` | Transpose. |

#### `Func` — function type

```lupus
(Func int int int)     ;; (int, int) -> int
```

Functions are first-class values. Closures are supported via lexical environment.

### 2.3. Algebraic types

#### `Option` — optional value

```lupus
(Option int)           ;; type
none                   ;; constructor for any Option
(some 42)              ;; constructor with value
```

| Constructor | Contents | Pattern |
|-------------|------------|---------|
| `none` | nothing | `(none)` |
| `some` | 1 value of type `a` | `((some value))` |

#### `Result` — success or error

```lupus
(Result int str)       ;; success: int, error: str
(success 42)           ;; success constructor
(failure "error msg")  ;; failure constructor
```

| Constructor | Contents | Pattern |
|-------------|------------|---------|
| `success` | 1 value of type `a` | `((success value))` |
| `failure` | 1 value of type `e` | `((failure err))` |

**Rule:** Functions that can fail must return `Option` or `Result`. Use `Result` when error information must be conveyed. Use `Option` when absence alone is enough.

### 2.4. User-defined and opaque types

**User-defined types (structs):** When defining `(defstruct point (x int) (y int))`, name `point` automatically becomes valid in `type_expr`. Constructor `(point 10 20)` and accessors `(point-x p)`, `(point-y p)` are generated automatically.

**User-defined generics:**
```lupus
(defstruct (Node a)
  (value a)
  (left (Option (Node a)))
  (right (Option (Node a))))

(define tree (Node 42 (some (Node 10 none none)) none))
```

**Opaque types:** Types such as `socket`, `task`, `channel` declared in FFI modules are also available in `type_expr` via `IDENTIFIER`. Their internals are hidden; operations are only possible through module functions.

---

## 3. Standard Library (API)

### 3.1. `core` module — auto-import

All `core` functions are available without a prefix. You cannot redefine a `core` function name in user code (linter error `core-shadowing`).

| Function | Signature | Description | Example |
|---------|-----------|----------|--------|
| `+` | `(Func int int int)` or `(Func float float float)` | Addition. Cannot mix int and float. | `(+ 2 3)` → `5` |
| `-` | `(Func int int int)` or `(Func float float float)` | Subtraction. | `(- 5 2)` → `3` |
| `*` | `(Func int int int)` or `(Func float float float)` | Multiplication. | `(* 3 4)` → `12` |
| `/` | `(Func int int int)` | Integer division. | `(/ 7 2)` → `3` |
| `/` | `(Func float float float)` | Float division. | `(/ 7.0 2.0)` → `3.5` |
| `%` | `(Func int int int)` | Remainder. | `(% 7 2)` → `1` |
| `=` | `(Func a a bool)` | Equality. Works for all types except `Func`. | `(= 1 1)` → `true` |
| `!=` | `(Func a a bool)` | Inequality. | `(!= 1 2)` → `true` |
| `<` | `(Func int int bool)` or `(Func float float bool)` or `(Func str str bool)` | Less than. | `(< 1 2)` → `true` |
| `>` | same as `<` | Greater than. | `(> 2 1)` → `true` |
| `<=` | same as `<` | Less than or equal. | `(<= 2 2)` → `true` |
| `>=` | same as `<` | Greater than or equal. | `(>= 2 2)` → `true` |
| `and` | `(Func bool bool bool)` | Logical AND (short-circuit). | `(and true false)` → `false` |
| `or` | `(Func bool bool bool)` | Logical OR (short-circuit). | `(or true false)` → `true` |
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
| `sleep` | `(Func int unit)` | Sleep N milliseconds (synchronous). | `(sleep 1000)` |
| `exit` | `(Func int unit)` | Exit program with code. | `(exit 0)` |
| `cons` | `(Func a (List a) (List a))` | Prepend to list. | `(cons 0 (list 1 2))` → `(list 0 1 2)` |
| `head` | `(Func (List a) (Option a))` | First element. | `(head (list 1 2))` → `(some 1)` |
| `tail` | `(Func (List a) (Option (List a)))` | List tail. | `(tail (list 1 2))` → `(some (list 2))` |
| `length` | `(Func (List a) int)` | List length. | `(length (list 1 2 3))` → `3` |
| `nth` | `(Func (List a) int (Option a))` | Element by index. | `(nth (list 1 2) 0)` → `(some 1)` |
| `empty?` | `(Func (List a) bool)` | Emptiness check. | `(empty? (list))` → `true` |
| `list-map` | `(Func (List a) (Func a b) (List b))` | Transform. | `(list-map (list 1 2) (lambda (x) (* x 2)))` |
| `list-filter` | `(Func (List a) (Func a bool) (List a))` | Filter. | `(list-filter (list 1 2 3) (lambda (x) (> x 1)))` |
| `list-fold` | `(Func (List a) b (Func b a b) b)` | Fold. | `(list-fold (list 1 2 3) 0 (lambda (acc x) (+ acc x)))` → `6` |
| `map-get` | `(Func (Map k v) k (Option v))` | Map lookup. | `(map-get m "key")` |
| `map-set` | `(Func (Map k v) k v (Map k v))` | Map insert/update. | `(map-set m "key" 42)` |
| `map-keys` | `(Func (Map k v) (List k))` | Map keys. | `(map-keys m)` |
| `map-values` | `(Func (Map k v) (List v))` | Map values. | `(map-values m)` |
| `map-has?` | `(Func (Map k v) k bool)` | Key check. | `(map-has? m "key")` |
| `tensor` | `(Func (List int) (List float) Tensor)` | Create tensor. | `(tensor (list 2 3) (list 1.0 2.0 ...))` |
| `tensor-shape` | `(Func Tensor (List int))` | Tensor shape. | `(tensor-shape t)` |
| `tensor-add` | `(Func Tensor Tensor (Result Tensor str))` | Tensor addition. | `(tensor-add t1 t2)` |
| `tensor-mul` | `(Func Tensor Tensor (Result Tensor str))` | Tensor multiplication. | `(tensor-mul t1 t2)` |
| `tensor-transpose` | `(Func Tensor Tensor)` | Transpose. | `(tensor-transpose t)` |

### 3.2. `senko` module (math)

Prefix: `math/`. Import: `(import (senko math))`.

| Function | Signature | Description | Example |
|---------|-----------|----------|--------|
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

### 3.3. `texas` module (net)

Prefix: `net/`. Import: `(import (texas net))` or `(import (texas net) :as net)`.

| Function | Signature | Description | Example |
|---------|-----------|----------|--------|
| `net/tcp-connect` | `(Func str int (Result socket str))` | TCP connection. host, port. | `(net/tcp-connect "example.com" 80)` |
| `net/tcp-listen` | `(Func int (Result socket str))` | TCP server, listen on port. | `(net/tcp-listen 8080)` |
| `net/accept` | `(Func socket (Result socket str))` | Accept connection (server). | `(net/accept sock)` |
| `net/send` | `(Func socket str (Result int str))` | Send string. Returns byte count. | `(net/send sock "GET / HTTP/1.0\r\n")` |
| `net/recv` | `(Func socket int (Result str str))` | Receive up to N bytes. | `(net/recv sock 4096)` |
| `net/recv-line` | `(Func socket (Result str str))` | Receive line up to `\n`. | `(net/recv-line sock)` |
| `net/close` | `(Func socket unit)` | Close socket. | `(net/close sock)` |
| `net/udp-bind` | `(Func int (Result socket str))` | UDP socket. | `(net/udp-bind 53)` |
| `net/udp-send-to` | `(Func socket str int str (Result int str))` | UDP send. | `(net/udp-send-to sock "host" 53 "data")` |
| `net/udp-recv-from` | `(Func socket int (Result (Tuple str int str) str))` | UDP receive. Returns `(host, port, data)`. | `(net/udp-recv-from sock 1024)` |

**Type `socket`:** Opaque type. Cannot be created directly from Lupus; only via `texas` functions.

### 3.4. `kaltsit` module (file)

Prefix: `file/`. Import: `(import (kaltsit file))` or `(import (kaltsit file) :as file)`.

| Function | Signature | Description | Example |
|---------|-----------|----------|--------|
| `file/read-file` | `(Func str (Result str str))` | Read entire file into string. | `(file/read-file "data.txt")` |
| `file/write-file` | `(Func str str (Result unit str))` | Write string to file (overwrite). | `(file/write-file "out.txt" "hello")` |
| `file/append-file` | `(Func str str (Result unit str))` | Append to file. | `(file/append-file "log.txt" "line\n")` |
| `file/exists?` | `(Func str bool)` | Existence check. | `(file/exists? "data.txt")` |
| `file/is-dir?` | `(Func str bool)` | Whether path is directory. | `(file/is-dir? "/tmp")` |
| `file/list-dir` | `(Func str (Result (List str) str))` | List files in directory. | `(file/list-dir "/tmp")` |
| `file/mkdir` | `(Func str (Result unit str))` | Create directory. | `(file/mkdir "newdir")` |
| `file/delete` | `(Func str (Result unit str))` | Delete file. | `(file/delete "old.txt")` |
| `file/size` | `(Func str (Result int str))` | File size in bytes. | `(file/size "data.txt")` |

### 3.5. `amiya` module (async)

Prefix: `async/`. Import: `(import (amiya async))` or `(import (amiya async) :as async)`.

| Function | Signature | Description | Example |
|---------|-----------|----------|--------|
| `async/spawn` | `(Func (Func unit) task)` | Run function in separate thread/task. **Argument function must return `unit`.** | `(async/spawn (lambda () (print "hi")))` |
| `async/sleep` | `(Func int unit)` | Async sleep (does not block other tasks). | `(async/sleep 1000)` |
| `async/channel` | `(Func (channel a))` | Create typed channel. | `(async/channel)` |
| `async/send` | `(Func (channel a) a unit)` | Send to channel. | `(async/send ch 42)` |
| `async/recv` | `(Func (channel a) a)` | **Blocking** receive from channel. | `(async/recv ch)` |
| `async/recv-timeout` | `(Func (channel a) int (Option a))` | Receive with timeout (ms). `none` on timeout. | `(async/recv-timeout ch 1000)` |
| `async/wait` | `(Func task unit)` | Wait for task completion. | `(async/wait t)` |
| `async/wait-all` | `(Func (List task) unit)` | Wait for all tasks. | `(async/wait-all (list t1 t2))` |

**Types `task` and `channel`:** Opaque types. Created only via `amiya`.

**Semantics:** In the Python prototype, `spawn` uses `threading.Thread`. Channels are implemented with `queue.Queue`. `async/recv` blocks the thread until data arrives (no busy-wait). `async/recv-timeout` uses `queue.get(timeout=ms/1000)`.

### 3.6. `w` module (test)

The `test` form is a **special form** of the language (built into the parser), not a function. Importing module `w` is not required to use `(test ...)`. However, `test/assert-eq`, `test/assert-true`, and `test/assert-false` are available after `(import (w test))` or via the `test/` prefix.

| Function | Signature | Description | Example |
|---------|-----------|----------|--------|
| `test` | Special form | Test declaration (see section 5). | `(test "name" (assert ...))` |
| `test/assert-eq` | `(Func a a unit)` | Assert equality with diff output. | `(test/assert-eq 2 (+ 1 1))` |
| `test/assert-true` | `(Func bool unit)` | Assert true. | `(test/assert-true (> 2 1))` |
| `test/assert-false` | `(Func bool unit)` | Assert false. | `(test/assert-false (= 1 2))` |
| `test/run` | `(Func (List str) int)` | Run tests by name. | `(test/run (list "add-works"))` |
| `test/run-all` | `(Func int)` | Run all tests in the file. | `(test/run-all)` |

---

## 4. FFI Specification

### 4.1. General principles

- FFI allows standard library modules to be implemented in the host language (Python in v0.1).
- Lupus code cannot call Python directly. Instead, Python functions are **registered** as Lupus symbols via FFI.
- The FFI directive is **declarative**: it tells the interpreter where to find the implementation.
- The directive must appear **at the beginning of the module file** (before any `define` or `import`).
- **All Python function calls through FFI are wrapped in `try...except`** (see section 4.5). Unhandled Python exceptions must not crash the Lupus interpreter.

### 4.2. FFI directive

```lupus
(#lupus ffi python "<module.path>")
```

- `python` — host language (in v0.1, only `python`).
- `<module.path>` — Python import path (for example, `lupus_modules.senko`).
- If the file is a **user script**, not a module, the FFI directive is forbidden (linter error `invalid-directive`).

### 4.3. Python module requirements

The Python module must export a `__lupus_exports__` dictionary:

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

**Callable requirements:**
1. Accepts as many arguments as specified in the type.
2. Returns a value matching the type.
3. If the function returns `Result`, it must return a tuple `("success", value)` or `("failure", error_msg)`.
4. If the function returns `Option`, it must return `None` (converted to `none`) or any value (converted to `(some value)`).
5. For constants (such as `pi`), the callable is a thunk (zero-argument function) invoked once when the module loads.

### 4.4. FFI loading process

1. The interpreter encounters `(import (senko math))`.
2. Resolves the module path (see section 4.6).
3. If directive `(#lupus ffi python "lupus_modules.senko")` is found, runs `importlib.import_module("lupus_modules.senko")`.
4. Reads `__lupus_exports__`.
5. For each key, creates an internal Lupus `Func` object with:
   - `name`: `math/<key>`
   - `type`: type parsed from the string
   - `impl`: Python callable (wrapped in a type-conversion adapter)
6. Registers in the `math` module environment.

### 4.5. FFI error handling

All Python function calls through FFI **must** be wrapped in `try...except`:

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

If a Python function fails with an unhandled exception:
- The Lupus interpreter **must not** crash.
- A JSON error with code `ffi-runtime-error` must be emitted.
- For functions returning `Result`, the exception may be translated to `(failure "...")`.
- For functions not returning `Result`/`Option`, a runtime panic is generated.

### 4.6. Module name resolution

For `(import (senko math))`:
1. The interpreter looks for `stdlib/senko/math.lupus` (or `senko/math.lupus` on `LUPUS_PATH`).
2. If the file is found, parses it as a Lupus module.
3. If the file has an FFI directive, loads the Python implementation.
4. All `define-public` from the file and all FFI exports become available under the `math/` prefix.

### 4.7. Full `senko` module binding example

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

### 4.8. FFI errors

| Error code | Condition |
|------------|---------|
| `ffi-module-not-found` | Python module not found |
| `ffi-export-missing` | Key in `__lupus_exports__` not found |
| `ffi-type-mismatch` | Python return value does not match declared type |
| `ffi-arity-mismatch` | Python function received wrong argument count |
| `ffi-runtime-error` | Python exception during execution (ZeroDivisionError, etc.) |

---

## 5. Test format

### 5.1. Declaring tests

Tests are declared inside `.lupus` files using the special `test` form:

```lupus
(test "unique-test-name"
  expr1
  expr2
  ...)
```

- `"unique-test-name"` — string identifier. Must be unique within the file. Duplicates trigger linter error `test-name-duplicate`.
- `expr1, expr2, ...` — test body (sequence of expressions). Usually contains `assert`.
- Tests **do not run** during normal program execution (`lupus run`). They run only with `lupus test <file>`.
- Tests **do not affect** program output during normal execution.

### 5.2. Running tests

```bash
lupus test file.lupus          # run all tests in file
lupus test file.lupus --name "add-works"  # run specific test
lupus test dir/                # recursively run all .lupus files
```

### 5.3. Result output

The interpreter prints JSON Lines (JSONL):

```json
{"type": "test-start", "name": "add-works", "file": "calc.lupus"}
{"type": "test-pass", "name": "add-works", "file": "calc.lupus", "duration_ms": 0.5}
{"type": "test-fail", "name": "safe-divide-zero", "file": "calc.lupus", "error": {"code": "assert-failed", "location": {"line": 15, "col": 3}, "message": "Assertion failed: (= (safe-divide 10 0) none)"}, "duration_ms": 1.2}
{"type": "test-summary", "total": 5, "passed": 4, "failed": 1, "file": "calc.lupus"}
```

### 5.4. Test rules

1. **Isolation:** Each test runs in a **fresh environment**. Test `define` and `define-mutable` are not visible to other tests. File `define-public` are visible to all tests.
2. **Order:** Tests run in declaration order in the file.
3. **Failure:** If `assert` fails, the test stops immediately (fail-fast). Other tests continue.
4. **Side effects:** `print` in tests is suppressed by default (shown only with `--verbose`).

---

## 6. Error format (JSON)

### 6.1. General structure

All errors, warnings, and runtime panics are emitted as strict JSON:

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

### 6.2. Full error catalog

| Code | Phase | Message | Example |
|-----|------|-----------|--------|
| `unknown-token` | lex | Unknown token | `@` |
| `unclosed-string` | lex | Unclosed string | `"hello` |
| `unclosed-comment` | lex | Unclosed block comment | `#| ...` |
| `unexpected-token` | parse | Unexpected token | `(define 1 2)` — 1 is not IDENTIFIER |
| `missing-rparen` | parse | Missing closing parenthesis | `(define x 10` |
| `type-mismatch` | type | Type mismatch | `(+ 1 "a")` |
| `unknown-identifier` | type | Unknown identifier | `(foo 1)` — foo is not defined |
| `unknown-module` | type | Unknown module | `(import (unknown mod))` |
| `unknown-module-symbol` | type | Unknown module symbol | `(math/unknown 1)` |
| `arity-mismatch` | type | Wrong number of arguments | `(+ 1)` — expected 2 |
| `missing-return-type` | type | Missing return type on public function | `(define-public (foo x) x)` |
| `missing-param-type` | type | Missing parameter type on public function | `(define-public (foo x) -> int x)` |
| `immutable-assignment` | type | set! on immutable variable | `(set! x 1)` where x is from `define` |
| `duplicate-definition` | lint | Duplicate definition | two `(define x ...)` |
| `unused-variable` | lint | Unused variable | `(define y 10)` without using y |
| `core-shadowing` | lint | Shadowing core function | `(define + 1)` |
| `test-name-duplicate` | lint | Duplicate test name | two `(test "foo" ...)` |
| `test-in-function` | lint | Test declared inside function | `(define (f) (test "x" ...))` |
| `divide-by-zero` | runtime | Division by zero | `(/ 1 0)` |
| `assert-failed` | runtime | Assert returned false | `(assert false)` |
| `index-out-of-bounds` | runtime | Index out of range | `(nth (list 1) 5)` |
| `match-non-exhaustive` | type | Non-exhaustive pattern match | `(match (some 1) ((none) 0))` — missing `some` |
| `match-redundant` | lint | Redundant pattern | Pattern after `else` or `_` |
| `invalid-directive` | lint | Invalid directive | `(#lupus unknown)` |
| `ffi-module-not-found` | ffi | Python module not found | `(#lupus ffi python "missing")` |
| `ffi-type-mismatch` | ffi | FFI type mismatch | Python function returned `str` instead of `float` |
| `ffi-arity-mismatch` | ffi | FFI arity mismatch | Python function took 3 instead of 2 |
| `ffi-runtime-error` | ffi | Exception in Python function | Division by zero in Python module |
| `value-restriction` | type | Value restriction violation | Polymorphic mutable without explicit annotation |

---

## 7. AST serialization (JSON)

### 7.1. Purpose

The AST must serialize to JSON for:
- Passing between layers (Parser → Typechecker → Interpreter).
- Saving to a dataset for LLM training.
- Consumption by the Rust version in phase 2.

### 7.2. Node format

Each AST node is an object with required fields:

```json
{
  "kind": "node_type",
  "span": {"file": "f.lupus", "start": 120, "end": 135, "line": 5, "col": 2},
  "data": { ... }
}
```

### 7.3. Node types

| `kind` | `data` fields | Description |
|--------|-------------|----------|
| `Program` | `toplevels: [Node]` | Root |
| `Define` | `name: str`, `mutable: bool`, `public: bool`, `constant: bool`, `value: Node`, `type_annotation: Type \| null` | Definition |
| `SetBang` | `name: str`, `value: Node` | Mutable assignment |
| `Lambda` | `params: [Param]`, `body: [Node]`, `return_type: Type \| null` | Anonymous function |
| `Param` | `name: str`, `type_annotation: Type \| null` | Function parameter |
| `If` | `condition: Node`, `then_branch: [Node]`, `else_branch: [Node]` | Conditional |
| `Cond` | `clauses: [(condition: Node, body: [Node])]` | Multi-way branch |
| `Match` | `expr: Node`, `clauses: [(pattern: Pattern, body: [Node])]` | Pattern matching |
| `PatternVar` | `name: str` | Variable pattern |
| `PatternWildcard` | `{}` | `_` pattern |
| `PatternConstructor` | `constructor: str`, `args: [Pattern]` | Constructor pattern |
| `PatternTuple` | `args: [Pattern]` | Tuple pattern `(Tuple a b c)` |
| `IfLet` | `binding: (name: str, expr: Node)`, `then_branch: [Node]`, `else_branch: [Node]` | If-let |
| `While` | `condition: Node`, `body: [Node]` | while loop |
| `For` | `var: str`, `iter: Node`, `body: [Node]` | for loop |
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

### 7.4. Serialization example

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

### 7.5. Serialization requirements

- **Deterministic:** JSON field order fixed (kind, span, data).
- **Span required:** Every node has span for precise diagnostics.
- **Type annotations:** If type not explicit, `type_annotation` and `return_type` are `null`.
- **No comments:** Comments not in AST (ignored during lexing).
- **Body as array:** All bodies serialize as `[Node]` arrays.

---

## 8. Implementation guidelines (Python)

### 8.1. Layer architecture

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
- Track positions (line, col, start, end) per token.
- Comments become `COMMENT` token dropped by parser, or ignored by lexer.
- `none` and `unit` lex as `LITERAL_NONE` and `LITERAL_UNIT`, not `IDENTIFIER` or `KEYWORD`.
- `true`/`false` lex as `BOOLEAN`.

**Hand-written lexer example (concept):**
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

### 8.3. Parser

**Recommended library:** `lark` with LALR parsing.

**Why lark:**
- Direct EBNF support.
- AST generation via `Transformer`.
- Good error diagnostics.

**Alternative:** Hand-written recursive descent (easier to control, more code).

**Parser key points:**
- `body` parses as `expr` sequence until closing paren of current level.
- `define-public` requires `func_header` (name in parens with params) and `body`.
- Parameters in `define-public` may be annotated `(a int)` or not (`a`). If not — type is inferred, but the type checker may emit `warn`.
- `local_define` differs from `toplevel define` only by context (parser may use same rules).
- `test` allowed only at top level.
- `defstruct` supports generics: `(defstruct (Node a) ...)`.

### 8.4. Type checker

**Algorithm:** Simplified Hindley-Milner with limited polymorphism.

**Key decisions:**
- **Type variables:** `a`, `b`, `t1`, `t2` — for inference.
- **Unification:** Standard unification with occurs check.
- **Type environment (Gamma):** Map `name -> type scheme`.
- **Polymorphism:** Let-polymorphism (generalize on `define` and `define-const`).
- **Value restriction:** For `define-mutable`, polymorphism is **forbidden**. Mutable types are not generalized (monomorphic). Ambiguous inference requires explicit annotation.
- **Limits:** No higher-order polymorphism for user types in v0.1.

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
        # All body expressions except last must be unit
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
        # Ensure variable is mutable and types match
        var_type = env.lookup(expr.name)
        val_type = infer(expr.value, env)
        unify(var_type, val_type)
        return Type("unit")
```

**Error requirements:**
- If unification fails — emit `type-mismatch` with both sides.
- If `define-public` lacks full annotations — `missing-return-type` or `missing-param-type`.
- If `set!` on `define` (immutable) — `immutable-assignment`.
- If `define-mutable` polymorphic without annotation — `value-restriction`.

### 8.5. Interpreter

**Strategy:** Tree-walk interpreter (AST traversal).

**Environment:**
- Hierarchical dict with parent link.
- `define` creates entry in current environment.
- `lambda` captures current environment (closure).
- `set!` searches current and parent environments, updates first mutable found.

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
- Built-ins (core): Python function, takes `List[Value]`, returns `Value`.
- User-defined: New environment, bind parameters, run body (sequentially; result is last expression).
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
        # Reverse conversion with type check
        ...
```

### 8.7. `amiya` module (async) — implementation

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
lupus run <file.lupus> [args...]     # run program
lupus test <file.lupus>               # run tests
lupus check <file.lupus>              # lint + types (no execution)
lupus ast <file.lupus>                # print AST as JSON
lupus eval <expr>                     # evaluate one expression (REPL mode)
```

### 8.9. Dependencies (requirements.txt)

```
lark>=1.1.0
click>=8.0.0
```

---

## 9. Program examples

### 9.1. Circle area calculator

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

### 9.2. HTTP client (GET request)

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

### 9.3. Async timer with channels

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

### 9.4. CSV file processing

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

### 9.5. Math unit tests

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

;; --- Tests ---
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

### 9.6. Data structures, Map, and generics

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

## 10. Checklist for GPT-5.5 Mimi

### 10.1. Files the generated code must include

| # | File | Description | Priority |
|---|------|----------|-------------|
| 1 | `lupus/lexer.py` | Tokenizer (hand-written or lark) | Required |
| 2 | `lupus/parser.py` | Parser building JSON-serializable AST | Required |
| 3 | `lupus/ast_nodes.py` | AST node classes with `.to_json()` | Required |
| 4 | `lupus/types.py` | Types, unification, type environment | Required |
| 5 | `lupus/typechecker.py` | Type inference (HM), annotation checks | Required |
| 6 | `lupus/linter.py` | Style, unused vars, duplicates | Required |
| 7 | `lupus/interpreter.py` | Tree-walk interpreter, envs, closures | Required |
| 8 | `lupus/values.py` | Runtime values | Required |
| 9 | `lupus/environment.py` | Hierarchical scopes | Required |
| 10 | `lupus/ffi.py` | Load Python modules, wrap functions, handle exceptions | Required |
| 11 | `lupus/errors.py` | JSON error formatting | Required |
| 12 | `lupus/cli.py` | Entry point: `lupus run`, `lupus test`, `lupus check`, `lupus ast` | Required |
| 13 | `lupus/core_builtins.py` | All `core` functions (+, -, *, list, map, assert, ...) | Required |
| 14 | `lupus_modules/senko.py` | FFI module math | Required |
| 15 | `lupus_modules/texas.py` | FFI module net (via socket) | Required |
| 16 | `lupus_modules/kaltsit.py` | FFI module file (via os, pathlib) | Required |
| 17 | `lupus_modules/amiya.py` | FFI module async (via threading, queue) | Required |
| 18 | `lupus_modules/w.py` | FFI module test (or built into interpreter) | Required |
| 19 | `stdlib/core.lupus` | Core module defs/docs (if partly in Lupus) | Optional |
| 20 | `tests/test_lexer.py` | Unit tests lexer | Required |
| 21 | `tests/test_parser.py` | Unit tests parser | Required |
| 22 | `tests/test_typechecker.py` | Unit tests type checker | Required |
| 23 | `tests/test_interpreter.py` | Unit tests interpreter | Required |
| 24 | `tests/test_ffi.py` | Unit tests FFI | Required |
| 25 | `tests/integration/` | Integration tests: all section 9 examples | Required |
| 26 | `docs/spec.md` | Copy of this specification | Required |
| 27 | `docs/grammar.ebnf` | Formal grammar | Required |
| 28 | `docs/api.md` | Standard library API docs | Required |
| 29 | `docs/tutorial.md` | LLM tutorial (how to write Lupus) | Required |
| 30 | `Makefile` / `pyproject.toml` | Build, install, run tests | Required |

### 10.2. Code quality requirements

1. **Test coverage:** At least 80% line coverage for `lexer.py`, `parser.py`, `typechecker.py`, `interpreter.py`.
2. **JSON errors:** All errors (including runtime panic) must use section 6 format.
3. **AST:** `.to_json()` must be deterministic and match section 7.
4. **FFI:** Each `lupus_modules/*.py` must define `__lupus_exports__`. All calls wrapped in `try...except`.
5. **CLI:** Support `--json` (JSON-only), `--verbose`, `--no-lint`.
6. **Documentation:** Every public function has docstring with description and types.

### 10.3. Generation order (recommended for GPT-5.5)

1. **First** AST and lexer (foundation).
2. **Then** parser + parser tests.
3. **Then** type system + type checker + tests.
4. **Then** runtime + interpreter + tests.
5. **Then** FFI + standard library modules.
6. **Then** CLI + integration tests.
7. **Finally** documentation.

### 10.4. Acceptance criteria (Definition of Done)

- [ ] All section 9 examples run without errors (`lupus run` and `lupus test`).
- [ ] `lupus check` passes for all `.lupus` files in `examples/`.
- [ ] `lupus ast example.lupus` prints valid JSON validating against section 7 schema.
- [ ] FFI modules `senko`, `texas`, `kaltsit`, `amiya`, `w` load and work.
- [ ] Interpreter handles all section 6 catalog errors.
- [ ] Test coverage ≥ 80% for core files.
- [ ] Python exceptions in FFI must not crash interpreter (emit JSON error).

---

## Appendix A. Glossary

| Term | Description |
|--------|----------|
| **AST** | Abstract Syntax Tree — abstract syntax tree. |
| **EBNF** | Extended Backus-Naur Form — grammar notation. |
| **FFI** | Foreign Function Interface — host function call mechanism. |
| **Hindley-Milner** | Type inference with polymorphism. |
| **LALR** | Look-Ahead LR — parsing algorithm (used in lark). |
| **Opaque type** | Type whose internals are hidden (socket, task). |
| **Prelude / Core** | Automatically imported functions. |
| **REPL** | Read-Eval-Print Loop — interactive mode. |
| **Span** | Source position range (file, line, column, bytes). |
| **Unit** | Type with single value `unit`, for side-effect functions. |
| **Value Restriction** | Restriction forbidding polymorphism for mutable variables in HM. |

---

*Document prepared for GPT-5.5 Mimi. All sections are self-contained and contain enough information to implement the Lupus v1.2 language in Python.*
