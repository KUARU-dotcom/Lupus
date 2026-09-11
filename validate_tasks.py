#!/usr/bin/env python3
"""Validate the Rust Lupus interpreter against tasks.json (100 tasks)."""
import json, subprocess, sys

BIN = "./target/debug/lupus"
TMP = "/tmp/task_run.lupus"

SOLUTIONS = {
    1: "(print (int->str (+ 7 15)))",
    2: "(print (int->str (- 100 37)))",
    3: "(print (int->str (* 6 9)))",
    4: "(print (int->str (/ 17 4)))",
    5: "(print (int->str (% 17 5)))",
    6: "(print (int->str (* 13 13)))",
    7: "(print (int->str (* 4 (* 4 4))))",
    8: "(define-mutable p 1) (define-mutable i 0) (while (< i 10) (set! p (* p 2)) (set! i (+ i 1))) (print (int->str p))",
    9: "(define (pow b e) (if (= e 0) 1 (* b (pow b (- e 1))))) (print (int->str (pow 3 5)))",
    10: "(if (> 17 42) (print (int->str 17)) (print (int->str 42)))",
    11: "(if (= (% 4 2) 0) (print \"even\") (print \"odd\"))",
    12: "(if (= (% 7 2) 0) (print \"even\") (print \"odd\"))",
    13: "(if (= (% 15 3) 0) (print \"yes\") (print \"no\"))",
    14: "(if (= (% 10 7) 0) (print \"yes\") (print \"no\"))",
    15: "(if (and (> 5 3) (< 10 20)) (print \"ok\") (print \"fail\"))",
    16: "(if (or (> 5 10) (< 3 7)) (print \"ok\") (print \"fail\"))",
    17: "(if (not (= 5 3)) (print \"ok\") (print \"fail\"))",
    18: "(if (> 42 0) (print \"positive\") (if (< 42 0) (print \"negative\") (print \"zero\")))",
    19: "(if (> 0 0) (print \"positive\") (if (< 0 0) (print \"negative\") (print \"zero\")))",
    20: "(if (> (- 5) 0) (print \"positive\") (if (< (- 5) 0) (print \"negative\") (print \"zero\")))",
    21: "(define-mutable i 1) (while (<= i 5) (print (int->str i)) (set! i (+ i 1)))",
    22: "(define-mutable i 5) (while (>= i 1) (print (int->str i)) (set! i (- i 1)))",
    23: "(define-mutable s 0) (define-mutable i 1) (while (<= i 10) (set! s (+ s i)) (set! i (+ i 1))) (print (int->str s))",
    24: "(define-mutable p 1) (define-mutable i 1) (while (<= i 5) (set! p (* p i)) (set! i (+ i 1))) (print (int->str p))",
    25: "(define-mutable i 2) (while (<= i 10) (print (int->str i)) (set! i (+ i 2)))",
    26: "(define-mutable i 1) (while (<= i 9) (print (int->str i)) (set! i (+ i 2)))",
    27: "(define-mutable s 0) (define-mutable i 2) (while (<= i 20) (set! s (+ s i)) (set! i (+ i 2))) (print (int->str s))",
    28: "(define-mutable s 0) (define-mutable i 1) (while (<= i 9) (set! s (+ s i)) (set! i (+ i 2))) (print (int->str s))",
    29: "(define-mutable i 1) (while (<= i 5) (print (int->str (* i i))) (set! i (+ i 1)))",
    30: "(define-mutable i 1) (while (<= i 5) (print (int->str (* 3 i))) (set! i (+ i 1)))",


    31: "(define-mutable p 2) (define-mutable i 1) (while (<= i 8) (print (int->str p)) (set! p (* p 2)) (set! i (+ i 1)))",
    32: "(print (int->str (+ 1 (+ 4 (+ 9 16)))))",
    33: "(define-mutable c 0) (define-mutable i 1) (while (<= i 20) (if (= (% i 4) 0) (set! c (+ c 1)) 0) (set! i (+ i 1))) (print (int->str c))",
    34: "(define lst (list 3 7 2 9 1 5)) (define-mutable m 0) (define-mutable i 0) (while (< i (length lst)) (if (> (nth lst i) m) (set! m (nth lst i)) 0) (set! i (+ i 1))) (print (int->str m))",
    35: "(define-mutable n 12345) (define-mutable s 0) (while (> n 0) (set! s (+ s (% n 10))) (set! n (/ n 10))) (print (int->str s))",
    36: "(define (fact n) (if (= n 0) 1 (* n (fact (- n 1))))) (print (int->str (fact 5)))",
    37: "(define (fact n) (if (= n 0) 1 (* n (fact (- n 1))))) (print (int->str (fact 7)))",
    38: "(define (fib n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (print (int->str (fib 7)))",
    39: "(define (fib n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (print (int->str (fib 9)))",
    40: "(define (sum n) (if (= n 0) 0 (+ n (sum (- n 1))))) (print (int->str (sum 10)))",
    41: "(define (pow b e) (if (= e 0) 1 (* b (pow b (- e 1))))) (print (int->str (pow 2 8)))",
    42: "(define (gcd a b) (if (= b 0) a (gcd b (% a b)))) (print (int->str (gcd 48 18)))",
    43: "(define (gcd a b) (if (= b 0) a (gcd b (% a b)))) (print (int->str (gcd 100 75)))",
    44: "(define (ds n) (if (= n 0) 0 (+ (% n 10) (ds (/ n 10))))) (print (int->str (ds 9876)))",
    45: "(define (count n) (if (> n 0) (begin (print (int->str n)) (count (- n 1))) 0)) (count 5)",
    46: "(define (count n) (if (> n 0) (begin (count (- n 1)) (print (int->str n))) 0)) (count 4)",
    47: "(define (trib n) (if (= n 0) 0 (if (= n 1) 1 (if (= n 2) 1 (+ (trib (- n 1)) (+ (trib (- n 2)) (trib (- n 3)))))))) (print (int->str (trib 7)))",
    48: "(define (pow b e) (if (= e 0) 1 (* b (pow b (- e 1))))) (print (int->str (pow 4 4)))",
    49: "(define (se n) (if (= n 0) 0 (+ n (se (- n 2))))) (print (int->str (se 10)))",
    50: "(define (find-max lst i m) (if (= i (length lst)) m (if (> (nth lst i) m) (find-max lst (+ i 1) (nth lst i)) (find-max lst (+ i 1) m)))) (print (int->str (find-max (list 3 1 4 1 5 9 2 6) 0 (- 999999))))",
    51: "(print (int->str (length (list 1 2 3 4 5))))",
    52: "(print (int->str (nth (list 10 20 30) 0)))",
    53: "(print (int->str (nth (list 10 20 30) 2)))",
    54: "(print (int->str (nth (list 5 10 15 20 25) 2)))",
    55: "(define (sl lst i) (if (= i (length lst)) 0 (+ (nth lst i) (sl lst (+ i 1))))) (print (int->str (sl (list 3 1 4 1 5 9) 0)))",
    56: "(define (mm lst i m) (if (= i (length lst)) m (if (> (nth lst i) m) (mm lst (+ i 1) (nth lst i)) (mm lst (+ i 1) m)))) (print (int->str (mm (list 3 1 4 1 5 9 2 6) 0 (- 999999))))",
    57: "(define (mn lst i m) (if (= i (length lst)) m (if (< (nth lst i) m) (mn lst (+ i 1) (nth lst i)) (mn lst (+ i 1) m)))) (print (int->str (mn (list 3 1 4 1 5 9 2 6) 0 999999)))",
    58: "(define (rec-length lst i) (if (= i (length lst)) i (rec-length lst (+ i 1)))) (print (int->str (rec-length (list 1 2 3 4 5 6 7) 0)))",
    59: "(define (cnt lst i) (if (= i (length lst)) 0 (+ (if (> (nth lst i) 4) 1 0) (cnt lst (+ i 1))))) (print (int->str (cnt (list 3 7 2 9 1 5 8) 0)))",
    60: "(define (has lst i) (if (= i (length lst)) 0 (if (= (nth lst i) 7) 1 (has lst (+ i 1))))) (if (= (has (list 3 1 4 1 5 9) 0) 1) (print \"yes\") (print \"no\"))",
    61: "(define (has lst i) (if (= i (length lst)) 0 (if (= (nth lst i) 5) 1 (has lst (+ i 1))))) (if (= (has (list 3 1 4 1 5 9) 0) 1) (print \"yes\") (print \"no\"))",
    62: "(define (pr lst i) (if (= i (length lst)) 1 (* (nth lst i) (pr lst (+ i 1))))) (print (int->str (pr (list 1 2 3 4 5) 0)))",
    63: "(define (idx lst i) (if (= (nth lst i) 9) i (idx lst (+ i 1)))) (print (int->str (idx (list 3 7 2 9 1) 0)))",
    64: "(define l (list 1 4 9 16 25)) (print (int->str (nth l 3)))",
    65: "(define (sl lst i) (if (= i (length lst)) 0 (+ (nth lst i) (sl lst (+ i 1))))) (print (int->str (+ (sl (list 1 2 3) 0) (sl (list 4 5 6) 0))))",
    66: "(print (string-append \"Hello\" \" World\"))",
    67: "(print (string-append \"one\" \"-\" \"two\" \"-\" \"three\"))",
    68: "(print (int->str 42))",
    69: "(print (string-append \"Result: \" (int->str 99)))",
    70: "(define-mutable i 0) (while (< i 4) (print \"ha\") (set! i (+ i 1)))",
    71: "(define (greet name) (print (string-append \"Hello, \" name \"!\"))) (greet \"Lupus\")",
    72: "(print (int->str (+ 3 (+ 4 5))))",
    73: "(define (fact n) (if (= n 0) 1 (* n (fact (- n 1))))) (print (string-append \"Result: \" (int->str (fact 6))))",
    74: "(print \"Step 1\") (print \"Step 2\") (print \"Step 3\")",
    75: "(define-mutable i 1) (while (<= i 10) (if (= (% i 3) 0) (print \"Fizz\") (print (int->str i))) (set! i (+ i 1)))",
    76: "(define (isprime n) (define-mutable i 2) (define-mutable res 1) (while (< i n) (if (= (% n i) 0) (set! res 0) 0) (set! i (+ i 1))) res) (if (= (isprime 7) 1) (print \"prime\") (print \"not prime\"))",
    77: "(define (isprime n) (define-mutable i 2) (define-mutable res 1) (while (< i n) (if (= (% n i) 0) (set! res 0) 0) (set! i (+ i 1))) res) (if (= (isprime 9) 1) (print \"prime\") (print \"not prime\"))",
    78: "(define (ack m n) (if (= m 0) (+ n 1) (if (= n 0) (ack (- m 1) 1) (ack (- m 1) (ack m (- n 1)))))) (print (int->str (ack 2 3)))",
    79: "(define-mutable n 6) (define-mutable c 0) (while (!= n 1) (if (= (% n 2) 0) (set! n (/ n 2)) (set! n (+ (* 3 n) 1))) (set! c (+ c 1))) (print (int->str c))",
    80: "(define (make-counter) (define-mutable n 0) (lambda () (set! n (+ n 1)) n)) (define c (make-counter)) (print (int->str (c))) (print (int->str (c))) (print (int->str (c)))",
    81: "(define d (lambda (x) (* x 2))) (print (int->str (d 21)))",
    82: "(define (dbl x) (* x 2)) (define l (list 1 2 3 4 5)) (define-mutable i 0) (while (< i (length l)) (print (int->str (dbl (nth l i)))) (set! i (+ i 1)))",
    83: "(define-mutable i 1) (while (<= i 10) (if (= (% i 3) 0) (print (int->str i)) 0) (set! i (+ i 1)))",
    84: "(define (sum lst i acc) (if (= i (length lst)) acc (sum lst (+ i 1) (+ acc (nth lst i))))) (print (int->str (sum (list 1 2 3 4 5) 0 0)))",
    85: "(define (is-even n) (if (= n 0) 1 (is-odd (- n 1)))) (define (is-odd n) (if (= n 0) 0 (is-even (- n 1)))) (if (= (is-even 4) 1) (print \"even\") (print \"odd\"))",
    86: "(print \"T-3\") (print \"T-2\") (print \"T-1\") (print \"Launch!\")",
    87: "(define (fib n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (print (int->str (fib 10)))",
    88: "(define-mutable i 5) (while (<= i 50) (print (int->str i)) (set! i (+ i 5)))",
    89: "(define (fact n) (if (= n 0) 1 (* n (fact (- n 1))))) (print (int->str (+ (fact 1) (+ (fact 2) (+ (fact 3) (fact 4))))))",
    90: "(define (luc n) (if (= n 0) 2 (if (= n 1) 1 (+ (luc (- n 1)) (luc (- n 2)))))) (print (int->str (luc 7)))",
    91: "(define (bin n k) (if (or (= k 0) (= k n)) 1 (+ (bin (- n 1) (- k 1)) (bin (- n 1) k)))) (print (int->str (bin 5 2)))",
    92: "(define-mutable n 12345) (define-mutable c 0) (while (> n 0) (set! c (+ c 1)) (set! n (/ n 10))) (print (int->str c))",
    93: "(define (rev n) (define-mutable r 0) (define-mutable m n) (while (> m 0) (set! r (+ (* r 10) (% m 10))) (set! m (/ m 10))) r) (define x 121) (if (= x (rev x)) (print \"yes\") (print \"no\"))",
    94: "(define (rev n) (define-mutable r 0) (define-mutable m n) (while (> m 0) (set! r (+ (* r 10) (% m 10))) (set! m (/ m 10))) r) (define x 123) (if (= x (rev x)) (print \"yes\") (print \"no\"))",
    95: "(define (m3 a b c) (if (< a b) (if (< a c) a c) (if (< b c) b c))) (print (int->str (m3 17 3 42)))",
    96: "(define (sum lst i) (if (= i (length lst)) 0 (+ (nth lst i) (sum lst (+ i 1))))) (define l (list 10 20 30 40)) (print (int->str (/ (sum l 0) (length l))))",
    97: "(define-mutable s 0) (define-mutable i 1) (while (<= i 7) (set! s (+ s i)) (set! i (+ i 1))) (print (int->str s))",
    98: "(define (isprime n) (define-mutable i 2) (define-mutable res 1) (while (< i n) (if (= (% n i) 0) (set! res 0) 0) (set! i (+ i 1))) res) (define-mutable s 0) (define-mutable i 2) (while (<= i 20) (if (= (isprime i) 1) (set! s (+ s i)) 0) (set! i (+ i 1))) (print (int->str s))",
    99: "(define-mutable n 12345) (define-mutable r 0) (while (> n 0) (set! r (+ (* r 10) (% n 10))) (set! n (/ n 10))) (print (int->str r))",
    100: "(define (pad n) (if (< n 3) 1 (+ (pad (- n 2)) (pad (- n 3))))) (print (int->str (pad 9)))",

}

def run_lupus(code):
    with open(TMP, "w", encoding="utf-8") as f:
        f.write(code)
    r = subprocess.run([BIN, TMP], capture_output=True, text=True)
    return r.stdout.strip()


def main():
    tasks = json.load(open("tasks.json"))
    # Нормализуем ключи решений к строкам (dict содержит смесь int/str ключей).
    sol = {str(k): v for k, v in SOLUTIONS.items()}
    passed, failed = 0, []
    for t in tasks:
        tid = str(t["id"])
        code = sol.get(tid, "")
        if not code:
            failed.append((tid, t["name"], "НЕТ РЕШЕНИЯ", "", ""))
            continue
        got = run_lupus(code)
        exp = t["expected"].strip()
        if got == exp:
            passed += 1
        else:
            failed.append((tid, t["name"], exp, got, code))
    print(f"PASSED: {passed}/{len(tasks)}")
    for tid, name, exp, got, code in failed:
        print(f"  FAIL #{tid} [{name}] exp='{exp}' got='{got}'")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

