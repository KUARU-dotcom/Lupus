#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
generate_lupus_dataset.py — генератор ОБУЧАЮЩЕГО датасета по языку Lupus
(Lisp-подобный, префиксная нотация) для LoRA-дообучения LLM.

Только одна "рука" — Lupus (никакой Python-руки). Решения генерируются LLM
через OpenAI-compatible API и валидируются локальным интерпретатором
lupus_proto.py. Задача попадает в датасет ТОЛЬКО если Lupus-решение прошло
валидацию (expected считается эталонной Python-функцией ВНУТРИ шаблона).

Ключевые особенности:
  * 50+ параметрических шаблонов, структурно отличных от tasks.json
    (композиции 2–3 операций, вложенные циклы, взаимная рекурсия,
    замыкания-счётчики, аккумуляторы, реверс цифр, палиндромы, строки,
    списки, edge-cases: ноль, отрицательные, пустые/одноэлементные списки).
  * Рантайм-проверка разнообразия: промпт нормализуется (числа -> N) и
    исключаются совпадения с нормализованными промптами tasks.json.
  * Стратификация по 7 категориям и тирам easy/medium/hard, --seed для
    детерминизма.
  * Параллелизм: concurrent.futures.ThreadPoolExecutor (--workers по умолч. 8),
    потокобезопасные структуры и периодический чекпоинт.
  * Self-repair: при ошибке валидации LLM получает текст ошибки (до 2 ретраев),
    трейсы пишутся в repair_traces.jsonl.
  * Чекпоинт каждые 100 задач в dataset_lupus.json, поддержка --resume.
  * Безопасная работа с API: content может быть None -> берём "" и считаем
    сбоем, не падаем с AttributeError. Ретраи с экспоненциальным бэкоффом.

Запуск:
    export LUPUS_API_BASE=https://api.example.com/v1
    export LUPUS_API_KEY=sk-...
    export LUPUS_MODEL=deepseek-v4-flash
    python generate_lupus_dataset.py --count 3000 --hold 300 --seed 42 --workers 8
    python generate_lupus_dataset.py --count 3000 --hold 300 --seed 42 --limit 20 --resume

Выходные файлы:
    train_lupus_bare.jsonl  — {"instruction","output"} (короткий system + промпт)
    train_lupus_sheet.jsonl — тот же output, instruction = system + шпаргалка
    hold_lupus.json         — hold-задачи с expected (для оценки)
    stats.json              — pass@1, доля repairs, разбивка по cat/difficulty
    dataset_lupus.json      — чекпоинт обработанных задач
    repair_traces.jsonl     — трейсы успешных self-repair
"""

from __future__ import annotations

import argparse
import json
import math
import os
import random
import re
import subprocess
import sys
import tempfile
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Set, Tuple

try:
    import requests
except ImportError:  # pragma: no cover
    requests = None
    sys.stderr.write("WARNING: 'requests' не установлен — установите: pip install requests\n")

# ============================================================================
# КОНСТАНТЫ
# ============================================================================

THIS_DIR = Path(__file__).resolve().parent
LUPUS_PROTO = THIS_DIR / "lupus_proto.py"
CHEATSHEET_PATH = THIS_DIR / "lupus_cheatsheet.txt"
TASKS_JSON_PATH = THIS_DIR / "tasks.json"
DATASET_PATH = THIS_DIR / "dataset_lupus.json"

CATEGORIES: List[str] = ["arithmetic", "logic", "loops", "recursion", "lists", "strings", "combined"]
DIFFICULTIES: List[str] = ["easy", "medium", "hard"]

DEFAULT_MODEL = "deepseek-v4-flash"
MAX_REPAIRS = 2            # до 2 ретраев self-repair
API_RETRIES = 4            # ретраи API при 429/5xx/timeout
CHECKPOINT_EVERY = 100     # чекпоинт каждые 100 задач
VALIDATE_TIMEOUT = 30      # секунд на запуск интерпретатора

SYSTEM_BARE = ("You are a programmer. Write ONLY Lupus code. "
               "No markdown, no explanation, no code blocks.")

# Веса тиров для стратификации (пересчитываются с учётом доступных тиров категории).
DIFF_WEIGHTS: Dict[str, float] = {"easy": 0.5, "medium": 0.3, "hard": 0.2}

# Регулярка для нормализации чисел в промпте.
_NUM_RE = re.compile(r"-?\d+")


def normalize_prompt(text: str) -> str:
    """Нормализовать промпт: все числа заменяются на 'N' (для рантайм-проверки
    разнообразия, чтобы отсечь клоны tasks.json с другими числами)."""
    return _NUM_RE.sub("N", text).strip()


def load_benchmark_normalized(path: Path = TASKS_JSON_PATH) -> Set[str]:
    """Загрузить tasks.json и вернуть множество нормализованных промптов
    бенчмарка — их нельзя повторять в обучающем датасете (защита от подгонки)."""
    norm: Set[str] = set()
    if not path.exists():
        return norm
    data = json.loads(path.read_text(encoding="utf-8"))
    for item in data:
        prompt = item.get("prompt", "")
        if prompt:
            norm.add(normalize_prompt(prompt))
    return norm

# ============================================================================
# ЭТАЛОННЫЕ МАТЕМАТИЧЕСКИЕ ФУНКЦИИ (ОБЩИЕ ДЛЯ ШАБЛОНОВ)
# ============================================================================

def _multi_str(values: Any) -> str:
    """Многострочный expected из итерируемого набора значений."""
    return "\n".join(str(v) for v in values)


def _prod(xs: Any) -> int:
    """Произведение последовательности целых чисел."""
    r = 1
    for x in xs:
        r *= x
    return r


def _is_prime(x: int) -> bool:
    """Проверка простоты числа."""
    return x > 1 and all(x % d != 0 for d in range(2, int(x ** 0.5) + 1))


def _ack(m: int, n: int) -> int:
    """Функция Аккермана (эталонная реализация внутри шаблона)."""
    if m == 0:
        return n + 1
    if n == 0:
        return _ack(m - 1, 1)
    return _ack(m - 1, _ack(m, n - 1))


def _collatz_steps(n: int) -> int:
    """Количество шагов гипотезы Коллатца до достижения 1."""
    steps = 0
    while n != 1:
        n = n // 2 if n % 2 == 0 else 3 * n + 1
        steps += 1
    return steps


def _reverse_int(n: int) -> int:
    """Реверс цифр числа через % и / (без строк)."""
    r = 0
    while n > 0:
        r = r * 10 + n % 10
        n //= 10
    return r


def _fib(n: int) -> int:
    """n-е число Фибоначчи (F(0)=0, F(1)=1)."""
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a


def _trib(n: int) -> int:
    """n-е число Трибоначчи (T(0)=0,T(1)=1,T(2)=1)."""
    t = [0, 1, 1]
    for _ in range(n):
        t.append(t[-1] + t[-2] + t[-3])
    return t[n]


def _lucas(n: int) -> int:
    """n-е число Люка (L(0)=2, L(1)=1)."""
    a, b = 2, 1
    for _ in range(n):
        a, b = b, a + b
    return a


def _rand_list(rng: random.Random, lo: int = 1, hi: int = 99) -> List[int]:
    """Случайный список из 3–6 целых чисел."""
    return [rng.randint(lo, hi) for _ in range(rng.randint(3, 6))]


def _rand_str_list(rng: random.Random, n: int) -> List[str]:
    """Случайный список из n слов без пробелов."""
    return [rng.choice(["red", "blue", "green", "fox", "moon", "star", "sun"])
            for _ in range(n)]


# ============================================================================
# ШАБЛОНЫ ЗАДАЧ: t_xxx(rng) -> (name, prompt, expected, difficulty)
#   expected вычисляется ЭТАЛОННОЙ Python-функцией ВНУТРИ шаблона.
#   Формулировки промптов сознательно отличаются от бенчмарка tasks.json.
# ============================================================================

# ----------------------------- ARITHMETIC -----------------------------------
def t_add_mul(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: a + b*c."""
    a, b, c = rng.randint(2, 25), rng.randint(2, 25), rng.randint(2, 25)
    res = a + b * c
    return (f"add_mul_{a}_{b}_{c}",
            f"Begin with {a}, then add to it the product of {b} and {c}. "
            f"Output the resulting value.",
            str(res), "easy")


def t_mul_add(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: a*b + c."""
    a, b, c = rng.randint(3, 20), rng.randint(3, 20), rng.randint(3, 60)
    res = a * b + c
    return (f"mul_add_{a}_{b}_{c}",
            f"Multiply {a} and {b}, then increase the outcome by {c}. "
            f"Print what you get.",
            str(res), "easy")


def t_sub_mul(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: a - b*c (может быть отрицательным)."""
    a = rng.randint(5, 60)
    b, c = rng.randint(3, 15), rng.randint(3, 15)
    res = a - b * c
    return (f"sub_mul_{a}_{b}_{c}",
            f"Take {a} and subtract from it the value of {b} multiplied by {c}. "
            f"Print the final number.",
            str(res), "medium" if res < 0 else "easy")


def t_paren_sum_mul(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: (a+b) * c."""
    a, b, c = rng.randint(3, 30), rng.randint(3, 30), rng.randint(2, 12)
    res = (a + b) * c
    return (f"paren_sum_mul_{a}_{b}_{c}",
            f"Sum {a} and {b}, multiply the total by {c}, and print the answer.",
            str(res), "easy")


def t_paren_diff_mul(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: (a-b) * c."""
    a, b = rng.randint(10, 60), rng.randint(2, 9)
    c = rng.randint(2, 12)
    res = (a - b) * c
    return (f"paren_diff_mul_{a}_{b}_{c}",
            f"Subtract {b} from {a}, multiply the difference by {c}, and output "
            f"the result.",
            str(res), "medium" if res < 0 else "easy")


def t_double_paren(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: (a+b) * (c-d)."""
    a, b, c, d = (rng.randint(4, 40), rng.randint(4, 40),
                  rng.randint(10, 50), rng.randint(2, 9))
    res = (a + b) * (c - d)
    return (f"double_paren_{a}_{b}_{c}_{d}",
            f"Combine: add {a} to {b}, subtract {d} from {c}, and multiply the "
            f"two results together. Print the number you obtain.",
            str(res), "hard")


def t_three_term_line(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: a + b - c."""
    a, b = rng.randint(5, 60), rng.randint(5, 60)
    c = rng.randint(2, 40)
    res = a + b - c
    return (f"three_term_{a}_{b}_{c}",
            f"Evaluate {a} plus {b} minus {c} and report the final value.",
            str(res), "easy")


def t_add_then_div(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: (a+b) // c (целочисленное деление)."""
    a, b = rng.randint(5, 50), rng.randint(5, 50)
    c = rng.randint(2, 9)
    res = (a + b) // c
    return (f"add_then_div_{a}_{b}_{c}",
            f"First sum {a} and {b}, then divide that total by {c} using integer "
            f"division and print the quotient.",
            str(res), "medium")


def t_mul_then_div(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: (a*b) // c."""
    a, b, c = rng.randint(3, 20), rng.randint(3, 20), rng.randint(2, 10)
    res = (a * b) // c
    return (f"mul_then_div_{a}_{b}_{c}",
            f"Multiply {a} by {b}, then divide the product by {c} (integer "
            f"division) and print the answer.",
            str(res), "medium")


def t_mod_add(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: a % b + c."""
    a, b, c = rng.randint(15, 120), rng.randint(3, 15), rng.randint(2, 50)
    res = a % b + c
    return (f"mod_add_{a}_{b}_{c}",
            f"Find the remainder of {a} divided by {b}, then add {c} to it and "
            f"print the result.",
            str(res), "easy")


def t_neg_mul(rng: random.Random) -> Tuple[str, str, str, str]:
    """Отрицательное значение: (- a) * b."""
    a, b = rng.randint(3, 20), rng.randint(3, 20)
    res = -a * b
    return (f"neg_mul_{a}_{b}",
            f"Take the negation of {a} and multiply it by {b}. Print the "
            f"(possibly negative) product.",
            str(res), "medium")


def t_neg_add(rng: random.Random) -> Tuple[str, str, str, str]:
    """Отрицательное слагаемое: (- a) + b."""
    a, b = rng.randint(3, 30), rng.randint(3, 50)
    res = b - a
    return (f"neg_add_{a}_{b}",
            f"Add {b} to the negative of {a} and print the outcome.",
            str(res), "medium" if res < 0 else "easy")


def t_zero_result(rng: random.Random) -> Tuple[str, str, str, str]:
    """Edge-case: результат равен нулю."""
    a, b = rng.randint(4, 25), rng.randint(4, 25)
    return (f"zero_result_{a}_{b}",
            f"Compute the product of {a} and {b}, then subtract from it the "
            f"product of {b} and {a}. Output the resulting number.",
            "0", "easy")


def t_zero_via_paren(rng: random.Random) -> Tuple[str, str, str, str]:
    """Edge-case: ноль через скобки."""
    a, b = rng.randint(5, 30), rng.randint(5, 30)
    return (f"zero_paren_{a}_{b}",
            f"Add {a} and {b}, then subtract the value you get by adding {b} "
            f"and {a}. Print the outcome.",
            "0", "easy")


def t_large_prod3(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция из трёх множителей: a*b*c."""
    a, b, c = rng.randint(5, 25), rng.randint(5, 25), rng.randint(5, 25)
    res = a * b * c
    return (f"prod3_{a}_{b}_{c}",
            f"Multiply three numbers together: {a}, {b} and {c}. Print the "
            f"final product.",
            str(res), "hard")


def t_diff_of_products(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: a*b - c*d."""
    a, b, c, d = (rng.randint(2, 20), rng.randint(2, 20),
                  rng.randint(2, 20), rng.randint(2, 20))
    res = a * b - c * d
    return (f"diff_products_{a}_{b}_{c}_{d}",
            f"Subtract the product of {c} and {d} from the product of {a} and "
            f"{b}, then print the result.",
            str(res), "medium" if res < 0 else "easy")


def t_sum_square_add(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция: a^2 + b."""
    a, b = rng.randint(3, 15), rng.randint(3, 70)
    res = a * a + b
    return (f"sum_sq_{a}_{b}",
            f"Square {a}, then add {b} to the square and print the answer.",
            str(res), "easy")


# ------------------------------- LOGIC --------------------------------------
def t_and_check(rng: random.Random) -> Tuple[str, str, str, str]:
    """Логика И: два сравнения."""
    a, b = rng.randint(1, 50), rng.randint(1, 50)
    c, d = rng.randint(1, 50), rng.randint(1, 50)
    ok = (a > b) and (c < d)
    return (f"and_check_{a}_{b}_{c}_{d}",
            f"Print 'TRUE' if both {a} > {b} and {c} < {d} hold, otherwise print "
            f"'FALSE'.",
            "TRUE" if ok else "FALSE", "easy")


def t_or_check(rng: random.Random) -> Tuple[str, str, str, str]:
    """Логика ИЛИ: два сравнения."""
    a, b = rng.randint(1, 50), rng.randint(1, 50)
    c, d = rng.randint(1, 50), rng.randint(1, 50)
    ok = (a > b) or (c < d)
    return (f"or_check_{a}_{b}_{c}_{d}",
            f"Print 'TRUE' if {a} > {b} OR {c} < {d} is satisfied, otherwise "
            f"print 'FALSE'.",
            "TRUE" if ok else "FALSE", "easy")


def t_not_equal(rng: random.Random) -> Tuple[str, str, str, str]:
    """Проверка неравенства."""
    a, b = rng.randint(1, 60), rng.randint(1, 60)
    return (f"not_equal_{a}_{b}",
            f"Print 'TRUE' when {a} differs from {b}, else print 'FALSE'.",
            "TRUE" if a != b else "FALSE", "easy")


def t_div_both(rng: random.Random) -> Tuple[str, str, str, str]:
    """Делится на оба числа."""
    n = rng.randint(12, 200)
    a = rng.choice([2, 3, 4])
    b = rng.choice([3, 5, 7])
    ok = (n % a == 0) and (n % b == 0)
    return (f"div_both_{n}_{a}_{b}",
            f"Check whether {n} can be split evenly by both {a} and {b}. Print "
            f"'Match' if it can, otherwise 'NoMatch'.",
            "Match" if ok else "NoMatch", "medium")


def t_div_by_11(rng: random.Random) -> Tuple[str, str, str, str]:
    """Делимость на 11 (отличается от бенчмарка — там делимость на 3 и 7)."""
    n = rng.randint(11, 132)
    return (f"div11_{n}",
            f"Decide if {n} is a multiple of 11. Print '1' when it is, otherwise "
            f"'0'.",
            "1" if n % 11 == 0 else "0", "easy")


def t_in_interval(rng: random.Random) -> Tuple[str, str, str, str]:
    """Попадание в интервал [lo, hi]."""
    lo = rng.randint(1, 40)
    hi = lo + rng.randint(8, 50)
    n = rng.randint(lo - 6, hi + 6)
    inside = lo <= n <= hi
    return (f"in_interval_{lo}_{hi}_{n}",
            f"Tell whether {n} lies inside the closed range from {lo} to {hi} "
            f"(both endpoints included). Print 'Inside' or 'Outside'.",
            "Inside" if inside else "Outside", "medium")


def t_pos_even(rng: random.Random) -> Tuple[str, str, str, str]:
    """Положительное и чётное."""
    n = rng.randint(-15, 30)
    ok = (n > 0) and (n % 2 == 0)
    return (f"pos_even_{n}",
            f"Check if {n} is positive and at the same time even. Output 'TRUE' "
            f"or 'FALSE'.",
            "TRUE" if ok else "FALSE", "easy")


def t_even_label(rng: random.Random) -> Tuple[str, str, str, str]:
    """Чётность с собственными метками."""
    n = rng.randint(0, 60)
    return (f"even_label_{n}",
            f"It is known a number is classified as class 'E' when it is even "
            f"and class 'O' when it is odd. Classify {n} and print its class.",
            "E" if n % 2 == 0 else "O", "easy")


def t_div_or_gt(rng: random.Random) -> Tuple[str, str, str, str]:
    """Делимость ИЛИ больше порога."""
    n = rng.randint(1, 100)
    ok = (n % 3 == 0) or (n > 50)
    return (f"div_or_gt_{n}",
            f"Print '1' if {n} is divisible by 3 or if it exceeds 50; print '0' "
            f"otherwise.",
            "1" if ok else "0", "easy")


def t_three_cond_all(rng: random.Random) -> Tuple[str, str, str, str]:
    """Три условия (a<b<c<d)."""
    a, b, c, d = (rng.randint(1, 20), rng.randint(1, 20), rng.randint(1, 20),
                  rng.randint(1, 26))
    ok = (a < b) and (b < c) and (c < d)
    return (f"three_cond_{a}_{b}_{c}_{d}",
            f"Print 'All' if the chain {a} < {b} < {c} < {d} is entirely true, "
            f"else print 'Broken'.",
            "All" if ok else "Broken", "medium")


def t_sign_word(rng: random.Random) -> Tuple[str, str, str, str]:
    """Знак числа с метками plus/minus/nil."""
    n = rng.randint(-12, 12)
    return (f"sign_word_{n}",
            f"Print 'plus' when {n} is greater than zero, 'minus' when it is "
            f"less than zero, and 'nil' when it equals zero.",
            "plus" if n > 0 else ("minus" if n < 0 else "nil"), "easy")


def t_xor_logic(rng: random.Random) -> Tuple[str, str, str, str]:
    """Ровно одно из двух условий (XOR)."""
    a, b = rng.randint(1, 40), rng.randint(1, 40)
    c, d = rng.randint(1, 40), rng.randint(1, 40)
    p, q = (a > b), (c < d)
    return (f"xor_logic_{a}_{b}_{c}_{d}",
            f"Exactly one of the two facts \"{a} > {b}\" and \"{c} < {d}\" is "
            f"true. Print '1' if exactly one holds, else '0'.",
            "1" if p != q else "0", "medium")


def t_same_remainder(rng: random.Random) -> Tuple[str, str, str, str]:
    """Одинаковые остатки от деления."""
    a, b = rng.randint(10, 80), rng.randint(10, 80)
    k = rng.randint(3, 10)
    same = (a % k) == (b % k)
    return (f"same_rem_{a}_{b}_{k}",
            f"Both {a} and {b} are divided by {k}. Print 'Same' if the two "
            f"remainders coincide, otherwise 'Diff'.",
            "Same" if same else "Diff", "medium")


# ------------------------------- LOOPS --------------------------------------
def t_sum_interval(rng: random.Random) -> Tuple[str, str, str, str]:
    """Сумма чисел от lo до hi (аккумулятор в цикле)."""
    lo = rng.randint(1, 10)
    hi = lo + rng.randint(6, 25)
    res = sum(range(lo, hi + 1))
    return (f"sum_interval_{lo}_{hi}",
            f"Add up every whole number from {lo} through {hi} (inclusive) and "
            f"print the accumulated total.",
            str(res), "medium")


def t_product_interval(rng: random.Random) -> Tuple[str, str, str, str]:
    """Произведение чисел от lo до hi (аккумулятор-произведение)."""
    lo = rng.randint(2, 5)
    hi = lo + rng.randint(2, 3)
    res = _prod(range(lo, hi + 1))
    return (f"prod_interval_{lo}_{hi}",
            f"Multiply all the integers from {lo} up to {hi} (inclusive) together "
            f"and show the resulting product.",
            str(res), "medium")


def t_count_multiples(rng: random.Random) -> Tuple[str, str, str, str]:
    """Подсчёт кратных k в интервале [lo, hi]."""
    lo = rng.randint(1, 30)
    hi = lo + rng.randint(10, 40)
    k = rng.randint(3, 9)
    cnt = sum(1 for x in range(lo, hi + 1) if x % k == 0)
    return (f"count_multiples_{lo}_{hi}_{k}",
            f"Walk through every integer from {lo} to {hi} and keep a tally of "
            f"how many are multiples of {k}. Output that tally.",
            str(cnt), "medium")


def t_nested_sum_product(rng: random.Random) -> Tuple[str, str, str, str]:
    """Вложенный цикл: сумма i*j для i in 1..m, j in 1..n."""
    m, n = rng.randint(3, 8), rng.randint(3, 8)
    res = sum(i * j for i in range(1, m + 1) for j in range(1, n + 1))
    return (f"nested_sum_product_{m}_{n}",
            f"Use two nested loops: for every i from 1 to {m} and every j from "
            f"1 to {n} add the product i*j to a running total. Print the total.",
            str(res), "hard")


def t_nested_count_pairs(rng: random.Random) -> Tuple[str, str, str, str]:
    """Вложенный цикл: число пар (i,j) с (i+j) кратным k."""
    m, n = rng.randint(3, 10), rng.randint(3, 10)
    k = rng.randint(3, 6)
    cnt = sum(1 for i in range(1, m + 1) for j in range(1, n + 1)
              if (i + j) % k == 0)
    return (f"nested_count_pairs_{m}_{n}_{k}",
            f"With nested loops go over i in 1..{m} and j in 1..{n}. Count the "
            f"pairs whose sum i+j is divisible by {k}. Print the count.",
            str(cnt), "hard")


def t_fib_loop(rng: random.Random) -> Tuple[str, str, str, str]:
    """Фибоначчи через ЦИКЛ (в бенчмарке — рекурсия)."""
    n = rng.randint(8, 20)
    return (f"fib_loop_{n}",
            f"Compute the {n}th Fibonacci term, where term 0 = 0 and term 1 = 1, "
            f"using an iterative loop. Print it.",
            str(_fib(n)), "medium")


def t_fact_loop(rng: random.Random) -> Tuple[str, str, str, str]:
    """Факториал через ЦИКЛ (в бенчмарке — рекурсия)."""
    n = rng.randint(4, 9)
    return (f"fact_loop_{n}",
            f"Find factorial({n}) — the product of every integer from 1 to {n} — "
            f"using a loop with an accumulator. Print the result.",
            str(_prod(range(1, n + 1))), "medium")


def t_pow_loop(rng: random.Random) -> Tuple[str, str, str, str]:
    """Возведение в степень через цикл."""
    b, e = rng.randint(2, 8), rng.randint(3, 6)
    return (f"pow_loop_{b}_{e}",
            f"Raise {b} to the {e}th power by repeatedly multiplying in a loop. "
            f"Print the value of {b}^{e}.",
            str(b ** e), "medium")


def t_squares_sum_interval(rng: random.Random) -> Tuple[str, str, str, str]:
    """Сумма квадратов чисел от lo до hi."""
    lo = rng.randint(1, 5)
    hi = lo + rng.randint(3, 6)
    res = sum(x * x for x in range(lo, hi + 1))
    return (f"squares_sum_{lo}_{hi}",
            f"Loop from {lo} to {hi}, squaring each number, and accumulate the "
            f"sum of those squares. Print the sum.",
            str(res), "medium")


def t_sum_evens_interval(rng: random.Random) -> Tuple[str, str, str, str]:
    """Сумма чётных чисел в интервале."""
    lo = rng.randint(1, 15)
    hi = lo + rng.randint(8, 25)
    res = sum(x for x in range(lo, hi + 1) if x % 2 == 0)
    return (f"sum_evens_interval_{lo}_{hi}",
            f"From {lo} to {hi} collect only the even numbers and fold them into "
            f"a running sum, then print the sum.",
            str(res), "medium")


def t_count_primes_interval(rng: random.Random) -> Tuple[str, str, str, str]:
    """Подсчёт простых чисел в интервале."""
    a, b = rng.randint(1, 5), rng.randint(25, 70)
    cnt = sum(1 for x in range(a, b + 1) if _is_prime(x))
    return (f"count_primes_{a}_{b}",
            f"Scan the integers from {a} to {b} and count how many of them are "
            f"prime. Print the count.",
            str(cnt), "hard")


def t_digit_product_loop(rng: random.Random) -> Tuple[str, str, str, str]:
    """Произведение цифр числа через цикл."""
    n = rng.randint(22, 9999)
    ds = [int(ch) for ch in str(n)]
    res = _prod(ds) if all(d != 0 for d in ds) else 0
    return (f"digit_product_{n}",
            f"Take the digits of {n} one by one (using remainder and division) "
            f"and multiply them together. Print the product.",
            str(res), "medium")


def t_accum_powers2(rng: random.Random) -> Tuple[str, str, str, str]:
    """Сумма степеней двойки: 2^0 + 2^1 + ... + 2^k (однозначная формулировка)."""
    k = rng.randint(3, 6)
    terms = ", ".join(str(2 ** i) for i in range(0, k + 1))
    res = sum(2 ** i for i in range(0, k + 1))
    return (f"accum_powers2_{k}",
            f"Add the powers of two from 2^0 up to and including 2^{k} — that "
            f"is, compute {terms} — and print the resulting total.",
            str(res), "medium")


def t_alternating_sum(rng: random.Random) -> Tuple[str, str, str, str]:
    """Знакочередующаяся сумма: 1-2+3-4+..."""
    n = rng.randint(5, 20)
    res = sum((1 if i % 2 == 1 else -1) * i for i in range(1, n + 1))
    return (f"alternating_sum_{n}",
            f"Sum the sequence 1 - 2 + 3 - 4 + ... up to term {n} (odd terms are "
            f"added, even terms are subtracted). Print the total.",
            str(res), "hard")


def t_even_countdown_print(rng: random.Random) -> Tuple[str, str, str, str]:
    """Печать чётных чисел в обратном порядке."""
    n = rng.randint(8, 20)
    vals = [x for x in range(n, 0, -1) if x % 2 == 0]
    return (f"even_countdown_{n}",
            f"Print every even number from {n} down to 2, each on its own line, "
            f"in decreasing order.",
            _multi_str(vals), "easy")


# ----------------------------- RECURSION ------------------------------------
def t_sum_rec_interval(rng: random.Random) -> Tuple[str, str, str, str]:
    """Рекурсивная сумма чисел от lo до hi."""
    lo = rng.randint(1, 8)
    hi = lo + rng.randint(5, 20)
    res = sum(range(lo, hi + 1))
    return (f"sum_rec_{lo}_{hi}",
            f"Write a recursive routine that adds the integers from {lo} through "
            f"{hi}. Call it and print the sum.",
            str(res), "medium")


def t_pow_rec(rng: random.Random) -> Tuple[str, str, str, str]:
    """Рекурсивное возведение в степень."""
    b, e = rng.randint(2, 9), rng.randint(3, 6)
    return (f"pow_rec_{b}_{e}",
            f"Define a recursive function to raise {b} to the power {e} and "
            f"print the outcome of {b}^{e}.",
            str(b ** e), "medium")


def t_fib_rec(rng: random.Random) -> Tuple[str, str, str, str]:
    """Рекурсивное число Фибоначчи (в бенчмарке константы F(7),F(9))."""
    n = rng.randint(8, 14)
    return (f"fib_rec_{n}",
            f"Using recursion compute the term F({n}) of the Fibonacci sequence "
            f"with F(0)=0 and F(1)=1. Output the value.",
            str(_fib(n)), "medium")


def t_gcd_rec(rng: random.Random) -> Tuple[str, str, str, str]:
    """Рекурсивный НОД (в бенчмарке (48,18) и (100,75))."""
    a, b = rng.randint(30, 160), rng.randint(20, 120)
    import math
    return (f"gcd_rec_{a}_{b}",
            f"Find the greatest common divisor of {a} and {b} with a recursive "
            f"Euclidean algorithm and print it.",
            str(math.gcd(a, b)), "medium")


def t_trib_rec(rng: random.Random) -> Tuple[str, str, str, str]:
    """Рекурсивное число Трибоначчи (в бенчмарке T(7))."""
    n = rng.randint(6, 12)
    return (f"trib_rec_{n}",
            f"Recursively evaluate the Tribonacci value T({n}) given T(0)=0, "
            f"T(1)=1, T(2)=1 and T(n)=T(n-1)+T(n-2)+T(n-3). Print it.",
            str(_trib(n)), "medium")


def t_collatz_rec(rng: random.Random) -> Tuple[str, str, str, str]:
    """Коллатц через рекурсию (в бенчмарке старт с 6)."""
    n = rng.randint(5, 25)
    return (f"collatz_rec_{n}",
            f"Starting at {n}, iteratively apply: halve even numbers, triple and "
            f"add one for odd numbers, until reaching 1. Count the steps taken, "
            f"using a recursive routine, and print that count.",
            str(_collatz_steps(n)), "hard")


def t_digit_product_rec(rng: random.Random) -> Tuple[str, str, str, str]:
    """Рекурсивное произведение цифр числа."""
    n = rng.randint(23, 9999)
    while "0" in str(n):
        n = rng.randint(23, 9999)
    ds = [int(ch) for ch in str(n)]
    return (f"digit_product_rec_{n}",
            f"Write a recursive function that multiplies all the digits of {n} "
            f"together and print the product.",
            str(_prod(ds)), "medium")


def t_count_evens_rec(rng: random.Random) -> Tuple[str, str, str, str]:
    """Рекурсивный подсчёт чётных элементов списка."""
    lst = _rand_list(rng)
    cnt = sum(1 for x in lst if x % 2 == 0)
    txt = ", ".join(str(x) for x in lst)
    return (f"count_evens_rec_{txt}",
            f"Recursively walk through the list [{txt}] and count how many of "
            f"its entries are even. Print the count.",
            str(cnt), "medium")


def t_list_contains_rec(rng: random.Random) -> Tuple[str, str, str, str]:
    """Рекурсивная проверка принадлежности элемента списку."""
    lst = _rand_list(rng, 5, 50)
    v = rng.choice([lst[rng.randint(0, len(lst) - 1)], rng.randint(5, 60)])
    ok = v in lst
    txt = ", ".join(str(x) for x in lst)
    return (f"contains_rec_{txt}_{v}",
            f"Using recursion decide whether the value {v} appears inside the "
            f"list [{txt}]. Print 'Present' or 'Absent'.",
            "Present" if ok else "Absent", "hard")


def t_reverse_digits_rec(rng: random.Random) -> Tuple[str, str, str, str]:
    """Рекурсивный реверс цифр числа."""
    n = rng.randint(102, 98765)
    return (f"reverse_rec_{n}",
            f"Reversing digits via repeated remainder/modulo and division, build "
            f"a recursive function that reads {n} from right to left. Print the "
            f"reversed number.",
            str(_reverse_int(n)), "hard")


def t_ackermann_rec(rng: random.Random) -> Tuple[str, str, str, str]:
    """Функция Аккермана (в бенчмарке (2,3)=9)."""
    m, n = rng.choice([(2, 2), (3, 1), (3, 2), (2, 4), (1, 5)])
    val = _ack(m, n)
    return (f"ackermann_{m}_{n}",
            f"Compute Ackermann({m},{n}) where A(0,n)=n+1, A(m,0)=A(m-1,1) and "
            f"A(m,n)=A(m-1,A(m,n-1)). Print the result.",
            str(val), "hard")


def t_mutual_even_odd(rng: random.Random) -> Tuple[str, str, str, str]:
    """Взаимная рекурсия is-even/is-odd (в бенчмарке is-even(4))."""
    n = rng.choice([7, 9, 11, 13, 6, 12])
    return (f"mutual_even_odd_{n}",
            f"Implement two functions is-even and is-odd that call each other "
            f"recursively. Call is-even on {n} and print 'Even' if true and "
            f"'Odd' if false.",
            "Even" if n % 2 == 0 else "Odd", "hard")


def t_lucas_rec(rng: random.Random) -> Tuple[str, str, str, str]:
    """Числа Люка (в бенчмарке L(7))."""
    n = rng.randint(6, 12)
    return (f"lucas_rec_{n}",
            f"Using recursion compute the Lucas number L({n}) with L(0)=2 and "
            f"L(1)=1, each L(k)=L(k-1)+L(k-2). Print the value.",
            str(_lucas(n)), "medium")


# ------------------------------- LISTS --------------------------------------
def t_list_sum(rng: random.Random) -> Tuple[str, str, str, str]:
    """Сумма элементов списка."""
    lst = _rand_list(rng, 1, 40)
    txt = ", ".join(str(x) for x in lst)
    return (f"list_sum_{txt}",
            f"Given the list [{txt}], sum all of its elements and print the total.",
            str(sum(lst)), "medium")


def t_list_max(rng: random.Random) -> Tuple[str, str, str, str]:
    """Максимум списка."""
    lst = _rand_list(rng)
    txt = ", ".join(str(x) for x in lst)
    return (f"list_max_{txt}",
            f"Among the values in the list [{txt}] identify the biggest one and "
            f"print it.",
            str(max(lst)), "medium")


def t_list_len(rng: random.Random) -> Tuple[str, str, str, str]:
    """Длина списка."""
    lst = _rand_list(rng, 10, 99)
    txt = ", ".join(str(x) for x in lst)
    return (f"list_len_{txt}",
            f"Report the number of entries stored in the list [{txt}].",
            str(len(lst)), "easy")


def t_list_first_times_last(rng: random.Random) -> Tuple[str, str, str, str]:
    """Произведение первого и последнего элемента."""
    lst = _rand_list(rng, 2, 20)
    txt = ", ".join(str(x) for x in lst)
    return (f"first_times_last_{txt}",
            f"In the list [{txt}] take the first element and the last element and "
            f"multiply them together. Print the product.",
            str(lst[0] * lst[-1]), "easy")


def t_list_sum_evens(rng: random.Random) -> Tuple[str, str, str, str]:
    """Сумма чётных элементов списка."""
    lst = _rand_list(rng, 1, 50)
    res = sum(x for x in lst if x % 2 == 0)
    txt = ", ".join(str(x) for x in lst)
    return (f"list_sum_evens_{txt}",
            f"From the list [{txt}] add together only the even entries and print "
            f"that sum.",
            str(res), "medium")


def t_list_count_odds(rng: random.Random) -> Tuple[str, str, str, str]:
    """Подсчёт нечётных элементов списка."""
    lst = _rand_list(rng)
    cnt = sum(1 for x in lst if x % 2 == 1)
    txt = ", ".join(str(x) for x in lst)
    return (f"list_count_odds_{txt}",
            f"Count how many of the numbers in the list [{txt}] are odd and "
            f"print the count.",
            str(cnt), "medium")


def t_list_min_index(rng: random.Random) -> Tuple[str, str, str, str]:
    """Индекс минимального элемента списка."""
    lst = _rand_list(rng, 5, 99)
    idx = lst.index(min(lst))
    txt = ", ".join(str(x) for x in lst)
    return (f"list_min_index_{txt}",
            f"Locate the position (0-based index) of the smallest value in the "
            f"list [{txt}]. Print that index.",
            str(idx), "hard")


def t_list_nth_sum_even_index(rng: random.Random) -> Tuple[str, str, str, str]:
    """Сумма элементов на чётных индексах."""
    lst = _rand_list(rng, 1, 40)
    res = sum(lst[i] for i in range(0, len(lst), 2))
    txt = ", ".join(str(x) for x in lst)
    return (f"list_nth_sum_even_idx_{txt}",
            f"In the list [{txt}] add the elements that sit at even indices "
            f"(0, 2, 4, ...) and print the total.",
            str(res), "medium")


def t_list_empty_len(rng: random.Random) -> Tuple[str, str, str, str]:
    """Edge-case: пустой список."""
    return (f"empty_list",
            f"Create an empty list (a list with no elements) and print how many "
            f"items it contains.",
            "0", "easy")


def t_list_empty_sum(rng: random.Random) -> Tuple[str, str, str, str]:
    """Edge-case: сумма элементов пустого списка."""
    return (f"empty_list_sum",
            f"Build an empty list and then add up all of its elements. Print the "
            f"resulting sum.",
            "0", "easy")


def t_list_single_len(rng: random.Random) -> Tuple[str, str, str, str]:
    """Edge-case: одноэлементный список."""
    v = rng.randint(5, 90)
    return (f"single_list_len_{v}",
            f"Create a list holding exactly one number, {v}, and print the length "
            f"of that list.",
            "1", "easy")


def t_list_single_value(rng: random.Random) -> Tuple[str, str, str, str]:
    """Edge-case: вывод единственного элемента одноэлементного списка."""
    v = rng.randint(5, 90)
    return (f"single_list_val_{v}",
            f"Create a single-element list [{v}], read back its only entry, and "
            f"print that entry.",
            str(v), "easy")


def t_list_avg(rng: random.Random) -> Tuple[str, str, str, str]:
    """Целочисленное среднее списка."""
    lst = _rand_list(rng, 10, 90)
    res = sum(lst) // len(lst)
    txt = ", ".join(str(x) for x in lst)
    return (f"list_avg_{txt}",
            f"Compute the integer average (sum divided by count) of the list "
            f"[{txt}] and print it.",
            str(res), "medium")


def t_list_filter_gt(rng: random.Random) -> Tuple[str, str, str, str]:
    """Фильтр: печать элементов больше n."""
    lst = _rand_list(rng, 1, 50)
    n = rng.randint(10, 40)
    kept = [x for x in lst if x > n]
    txt = ", ".join(str(x) for x in lst)
    return (f"list_filter_gt_{txt}_{n}",
            f"Go through the list [{txt}] and print, one per line, every element "
            f"that is strictly greater than {n}.",
            _multi_str(kept), "medium")


def t_list_sum_squares(rng: random.Random) -> Tuple[str, str, str, str]:
    """Сумма квадратов элементов списка."""
    lst = _rand_list(rng, 1, 12)
    res = sum(x * x for x in lst)
    txt = ", ".join(str(x) for x in lst)
    return (f"list_sum_squares_{txt}",
            f"Square each entry of the list [{txt}] and sum all the squares "
            f"together. Print the total.",
            str(res), "medium")


def t_list_count_gt(rng: random.Random) -> Tuple[str, str, str, str]:
    """Подсчёт элементов больше n."""
    lst = _rand_list(rng, 1, 60)
    n = rng.randint(10, 50)
    cnt = sum(1 for x in lst if x > n)
    txt = ", ".join(str(x) for x in lst)
    return (f"list_count_gt_{txt}_{n}",
            f"Tell how many entries of the list [{txt}] are larger than {n} and "
            f"print that count.",
            str(cnt), "medium")


# ------------------------------- STRINGS ------------------------------------
def t_concat_two_words(rng: random.Random) -> Tuple[str, str, str, str]:
    """Конкатенация двух строк."""
    a = rng.choice(["Sun", "Night", "Fire", "Ocean", "Snow", "Wind"])
    b = rng.choice(["Shine", "Fall", "Blaze", "Wave", "Dust", "Call"])
    return (f"concat_two_{a}_{b}",
            f"Join the string '{a}' together with the string '{b}' and print the "
            f"single combined line.",
            a + b, "easy")


def t_concat_three(rng: random.Random) -> Tuple[str, str, str, str]:
    """Конкатенация трёх строк."""
    parts = _rand_str_list(rng, 3)
    return (f"concat_three_{'_'.join(parts)}",
            f"Concatenate the three strings '{parts[0]}', '{parts[1]}' and "
            f"'{parts[2]}' into one piece of text and print it.",
            "".join(parts), "easy")


def t_concat_four(rng: random.Random) -> Tuple[str, str, str, str]:
    """Конкатенация четырёх строк."""
    p = _rand_str_list(rng, 4)
    return (f"concat_four_{'_'.join(p)}",
            f"Fuse the four tokens '{p[0]}', '{p[1]}', '{p[2]}' and '{p[3]}' in "
            f"that order into a single string and print it.",
            "".join(p), "easy")


def t_str_plus_num(rng: random.Random) -> Tuple[str, str, str, str]:
    """Строка + число."""
    n = rng.randint(5, 999)
    word = rng.choice(["Count", "Score", "Value", "Total"])
    return (f"str_num_{word}_{n}",
            f"Print the text '{word}: ' immediately followed by the number {n} "
            f"on one line.",
            f"{word}: {n}", "easy")


def t_str_repeat(rng: random.Random) -> Tuple[str, str, str, str]:
    """Повтор строки на отдельных строках."""
    word = rng.choice(["la", "zo", "ta", "bo", "mi"])
    k = rng.randint(2, 5)
    return (f"str_repeat_{word}_{k}",
            f"Print the word '{word}' exactly {k} times, each occurrence on its "
            f"own line.",
            _multi_str([word] * k), "easy")


def t_str_join_numbers(rng: random.Random) -> Tuple[str, str, str, str]:
    """Числа через разделитель в одной строке."""
    n = rng.randint(3, 6)
    sep = rng.choice(["-", ":", "_"])
    s = sep.join(str(i) for i in range(1, n + 1))
    return (f"str_join_{n}_{sep}",
            f"Produce a single line containing the numbers 1 through {n} joined "
            f"by the character '{sep}' (e.g. 1{sep}2{sep}3). Print that line.",
            s, "medium")


def t_str_greet(rng: random.Random) -> Tuple[str, str, str, str]:
    """Функция greet с собственным именем."""
    name = rng.choice(["Nova", "Astra", "Rex", "Kira", "Blaze", "Nix"])
    return (f"str_greet_{name}",
            f"Define a function greet(person) that prints 'Welcome, ' followed by "
            f"the person's name followed by '!'. Call it with '{name}'.",
            f"Welcome, {name}!", "easy")


def t_str_countdown(rng: random.Random) -> Tuple[str, str, str, str]:
    """Строковый обратный отсчёт."""
    n = rng.randint(3, 5)
    tag = rng.choice(["Phase-", "Round-", "Stage-"])
    lines = [f"{tag}{i}" for i in range(n, 0, -1)]
    return (f"str_countdown_{tag}_{n}",
            f"Print the lines '{tag}{n}', '{tag}{n-1}', ..., '{tag}1' — each on a "
            f"separate line, in that exact order.",
            _multi_str(lines), "easy")


def t_str_equality(rng: random.Random) -> Tuple[str, str, str, str]:
    """Сравнение двух ЯВНО заданных строк (использует '=').

    Обе строки полностью описаны в промпте (прямой/обратный порядок слов),
    поэтому expected 'equal'/'different' однозначно выводим из условия."""
    words = ["red", "blue", "green", "fox", "moon", "star", "sun"]
    a, b = rng.sample(words, 2)      # разные слова => a+b != b+a
    left = a + b
    same = rng.random() < 0.5
    if same:
        right = a + b
        second_desc = f"again in the same order: '{a}' then '{b}'"
    else:
        right = b + a
        second_desc = f"in the reverse order: '{b}' then '{a}'"
    assert (left == right) == same
    return (f"str_equality_{a}_{b}",
            f"Assemble a first string by writing '{a}' then '{b}'. Assemble a "
            f"second string {second_desc}. Compare the two strings and print "
            f"'equal' if they are identical, otherwise 'different'.",
            "equal" if same else "different", "medium")


def t_str_prefix_suffix(rng: random.Random) -> Tuple[str, str, str, str]:
    """Конкатенация префикса + середины + суффикса."""
    pre = rng.choice(["start_", "prefix_", "head_"])
    mid = rng.choice(["core", "mid", "body"])
    suf = rng.choice(["_end", "_tail", "_finish"])
    return (f"str_pre_suf_{pre}_{mid}_{suf}",
            f"Glue together the prefix '{pre}', the middle part '{mid}' and the "
            f"suffix '{suf}', in that order, and print the final string.",
            pre + mid + suf, "easy")


def t_str_num_sum(rng: random.Random) -> Tuple[str, str, str, str]:
    """Строка + числовой результат суммы."""
    a, b = rng.randint(3, 40), rng.randint(3, 40)
    res = a + b
    return (f"str_num_sum_{a}_{b}",
            f"Add {a} and {b}, then print the label 'Sum = ' followed by the "
            f"result on one line.",
            f"Sum = {res}", "easy")


def t_str_multi_values(rng: random.Random) -> Tuple[str, str, str, str]:
    """Несколько строк 'key: value'."""
    k = rng.randint(2, 4)
    vals = [rng.randint(10, 99) for _ in range(k)]
    prefix = rng.choice(["v", "item", "part"])
    lines = [f"{prefix}{i}: {vals[i - 1]}" for i in range(1, k + 1)]
    return (f"str_multi_{k}_{prefix}",
            f"Print {k} lines in the form '{prefix}1: <val>', '{prefix}2: <val>', "
            f"... '{prefix}{k}: <val>' where the values are {', '.join(str(v) for v in vals)} "
            f"respectively. One line per entry.",
            _multi_str(lines), "medium")


# ------------------------------ COMBINED ------------------------------------
def t_closure_counter(rng: random.Random) -> Tuple[str, str, str, str]:
    """Замыкание-счётчик (в бенчмарке — 3 вызова)."""
    k = rng.randint(4, 6)
    return (f"closure_counter_{k}",
            f"Build a counter with a closure: a function that increments an "
            f"internal mutable counter on every call and returns the new value. "
            f"Call it {k} times and print each returned value, one per line.",
            _multi_str(range(1, k + 1)), "hard")


def t_closure_step(rng: random.Random) -> Tuple[str, str, str, str]:
    """Замыкание-счётчик с шагом 2."""
    k = rng.randint(3, 4)
    vals = [2 * (i + 1) for i in range(k)]
    return (f"closure_step_{k}",
            f"Make a closure-based counter whose internal value starts at 0 and "
            f"grows by 2 with each call. Invoke it {k} times and print the values "
            f"returned, each on a new line.",
            _multi_str(vals), "hard")


def t_lambda_triple(rng: random.Random) -> Tuple[str, str, str, str]:
    """Лямбда: утроение."""
    v = rng.randint(5, 40)
    return (f"lambda_triple_{v}",
            f"Create an anonymous function that triples its input, assign it to a "
            f"name, then apply it to {v} and print the result.",
            str(v * 3), "easy")


def t_lambda_compose(rng: random.Random) -> Tuple[str, str, str, str]:
    """Композиция двух лямбд."""
    a = rng.randint(5, 40)
    res = (a + 3) * 2
    return (f"lambda_compose_{a}",
            f"Define an anonymous function that adds 3 and another one that "
            f"doubles. Pipe the value {a} through both (add 3, then double) and "
            f"print the final number.",
            str(res), "medium")


def t_map_square(rng: random.Random) -> Tuple[str, str, str, str]:
    """map: квадраты элементов списка."""
    lst = _rand_list(rng, 1, 9)
    vals = [x * x for x in lst]
    txt = ", ".join(str(x) for x in lst)
    return (f"map_square_{txt}",
            f"Apply a 'squaring' function to every element of the list [{txt}] "
            f"and print each transformed value on its own line.",
            _multi_str(vals), "medium")


def t_map_increment(rng: random.Random) -> Tuple[str, str, str, str]:
    """map: прибавление 1 к элементам списка."""
    lst = _rand_list(rng, 10, 50)
    vals = [x + 1 for x in lst]
    txt = ", ".join(str(x) for x in lst)
    return (f"map_inc_{txt}",
            f"Take the list [{txt}] and print, line by line, the result of adding "
            f"1 to each element.",
            _multi_str(vals), "easy")


def t_fold_sum(rng: random.Random) -> Tuple[str, str, str, str]:
    """Накопление (fold): сумма списка через аккумулятор."""
    lst = _rand_list(rng, 1, 30)
    txt = ", ".join(str(x) for x in lst)
    return (f"fold_sum_{txt}",
            f"Reduce the list [{txt}] by folding: start from 0 and for each "
            f"element add it to the running accumulator. Print the final "
            f"accumulator value.",
            str(sum(lst)), "medium")


def t_reverse_digits(rng: random.Random) -> Tuple[str, str, str, str]:
    """Реверс цифр через % и / (в бенчмарке 12345->54321)."""
    n = rng.randint(102, 98765)
    return (f"reverse_digits_{n}",
            f"Reverse the order of the digits of {n} using only remainder and "
            f"integer division (no strings). Print the reversed number.",
            str(_reverse_int(n)), "medium")


def t_palindrome_num(rng: random.Random) -> Tuple[str, str, str, str]:
    """Проверка палиндрома числа."""
    if rng.random() < 0.5:
        half = str(rng.randint(10, 999))
        n = int(half + half[::-1])
    else:
        n = rng.randint(201, 99999)
        while str(n) == str(n)[::-1]:
            n = rng.randint(201, 99999)
    return (f"palindrome_{n}",
            f"Determine whether the number {n} reads the same forwards and "
            f"backwards. Print 'yes' or 'no'.",
            "yes" if str(n) == str(n)[::-1] else "no", "medium")


def t_reverse_add(rng: random.Random) -> Tuple[str, str, str, str]:
    """Реверс и сложение с исходным числом."""
    n = rng.randint(102, 9999)
    res = n + _reverse_int(n)
    return (f"reverse_add_{n}",
            f"Take the number {n}, reverse its digits, and add the reversed value "
            f"back to {n}. Print the sum.",
            str(res), "hard")


def t_perfect_square(rng: random.Random) -> Tuple[str, str, str, str]:
    """Проверка, является ли число полным квадратом."""
    root = rng.randint(4, 20)
    if rng.random() < 0.5:
        n = root * root
    else:
        n = root * root + rng.choice([1, 2, 3])
    is_sq = int(n ** 0.5) ** 2 == n
    return (f"perfect_square_{n}",
            f"Check whether {n} is a perfect square (some integer squared). Print "
            f"'square' or 'nonsquare'.",
            "square" if is_sq else "nonsquare", "medium")


def t_prime_label(rng: random.Random) -> Tuple[str, str, str, str]:
    """Проверка простоты с метками prime/composite."""
    n = rng.randint(5, 40)
    return (f"prime_label_{n}",
            f"Classify {n} as 'prime' if it has no divisors other than 1 and "
            f"itself, otherwise 'composite'. Print the label.",
            "prime" if _is_prime(n) else "composite", "medium")


def t_max_of_four(rng: random.Random) -> Tuple[str, str, str, str]:
    """Максимум четырёх чисел."""
    nums = [rng.randint(1, 90) for _ in range(4)]
    return (f"max_of_four_{'_'.join(str(x) for x in nums)}",
            f"Given four numbers {nums[0]}, {nums[1]}, {nums[2]} and {nums[3]}, "
            f"report the largest of them.",
            str(max(nums)), "easy")


def t_sum_of_multiples(rng: random.Random) -> Tuple[str, str, str, str]:
    """Сумма кратных k до n."""
    k, n = rng.randint(3, 9), rng.randint(20, 90)
    res = sum(x for x in range(k, n + 1, k))
    return (f"sum_multiples_{k}_{n}",
            f"Add together all the multiples of {k} between 1 and {n} (inclusive) "
            f"and print the total.",
            str(res), "medium")


# ============================================================================
# РЕЕСТР ШАБЛОНОВ ПО КАТЕГОРИЯМ И ТИРАМ
# ============================================================================

# Каждому шаблону соответствует фиксированный тир сложности, поэтому
# стратификацию по easy/medium/hard можно выполнить детерминированно.
_TEMPLATES: Dict[str, Dict[str, List[Callable[[random.Random], Tuple[str, str, str, str]]]]] = {
    "arithmetic": {
        "easy": [t_add_mul, t_mul_add, t_paren_sum_mul, t_three_term_line, t_mod_add,
                 t_zero_result, t_zero_via_paren, t_sum_square_add, t_sub_mul,
                 t_paren_diff_mul, t_diff_of_products, t_neg_add],
        "medium": [t_add_then_div, t_mul_then_div, t_neg_mul],
        "hard": [t_double_paren, t_large_prod3],
    },
    "logic": {
        "easy": [t_and_check, t_or_check, t_not_equal, t_div_by_11, t_pos_even,
                 t_even_label, t_div_or_gt, t_sign_word],
        "medium": [t_div_both, t_in_interval, t_three_cond_all, t_xor_logic,
                   t_same_remainder],
        "hard": [],
    },
    "loops": {
        "easy": [t_even_countdown_print],
        "medium": [t_sum_interval, t_product_interval, t_count_multiples, t_fib_loop,
                   t_fact_loop, t_pow_loop, t_squares_sum_interval, t_sum_evens_interval,
                   t_digit_product_loop, t_accum_powers2],
        "hard": [t_nested_sum_product, t_nested_count_pairs, t_count_primes_interval,
                 t_alternating_sum],
    },
    "recursion": {
        "easy": [],
        "medium": [t_sum_rec_interval, t_pow_rec, t_fib_rec, t_gcd_rec, t_trib_rec,
                   t_digit_product_rec, t_count_evens_rec, t_lucas_rec],
        "hard": [t_collatz_rec, t_list_contains_rec, t_reverse_digits_rec,
                 t_ackermann_rec, t_mutual_even_odd],
    },
    "lists": {
        "easy": [t_list_len, t_list_first_times_last, t_list_empty_len, t_list_empty_sum,
                 t_list_single_len, t_list_single_value],
        "medium": [t_list_sum, t_list_max, t_list_sum_evens, t_list_count_odds,
                   t_list_nth_sum_even_index, t_list_avg, t_list_filter_gt,
                   t_list_sum_squares, t_list_count_gt],
        "hard": [t_list_min_index],
    },
    "strings": {
        "easy": [t_concat_two_words, t_concat_three, t_concat_four, t_str_plus_num,
                 t_str_repeat, t_str_greet, t_str_countdown, t_str_prefix_suffix,
                 t_str_num_sum],
        "medium": [t_str_join_numbers, t_str_equality, t_str_multi_values],
        "hard": [],
    },
    "combined": {
        "easy": [t_lambda_triple, t_map_increment, t_max_of_four],
        "medium": [t_lambda_compose, t_map_square, t_fold_sum, t_reverse_digits,
                   t_palindrome_num, t_perfect_square, t_prime_label, t_sum_of_multiples],
        "hard": [t_closure_counter, t_closure_step, t_reverse_add],
    },
}


def _count_templates() -> int:
    """Общее число шаблонов (для вывода в лог)."""
    return sum(len(lst) for cat in _TEMPLATES.values() for lst in cat.values())


# ============================================================================
# ГЕНЕРАЦИЯ ЗАДАЧ СО СТРАТИФИКАЦИЕЙ
# ============================================================================

def _split_bucket(n: int, available: List[str]) -> Dict[str, int]:
    """Распределить n задач по доступным тирам пропорционально DIFF_WEIGHTS.
    Остаток раздаётся по наибольшим дробным частям (метод наибольших остатков)."""
    total_w = sum(DIFF_WEIGHTS[d] for d in available)
    frac = {d: n * DIFF_WEIGHTS[d] / total_w for d in available}
    base = {d: int(frac[d]) for d in available}
    rem = n - sum(base.values())
    order = sorted(available, key=lambda d: frac[d] - int(frac[d]), reverse=True)
    i = 0
    while rem > 0:
        base[order[i % len(order)]] += 1
        rem -= 1
        i += 1
    return base


def generate_tasks(count: int, hold: int, seed: int,
                   benchmark_norm: Set[str]) -> Tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
    """Сгенерировать train (count) и hold (hold) задачи со стратификацией по
    7 категориям и тирам easy/medium/hard. Возвращает (train_tasks, hold_tasks).

    Разнообразие: промпты нормализуются (числа -> N) и исключаются КОЛЛИЗИИ
    с нормализованными промптами бенчмарка tasks.json (защита от клонов).
    Уникальность внутри набора обеспечивается по имени шаблона, которое
    кодирует все параметры (числа/слова), поэтому тройки уникальны."""
    rng = random.Random(seed)
    tasks: List[Dict[str, Any]] = []
    used_names: Set[str] = set()

    def make(cat: str, diff: str) -> Optional[Dict[str, Any]]:
        """Создать одну уникальную задачу в категории cat тира diff."""
        pool = _TEMPLATES[cat][diff]
        for _ in range(200):
            name, prompt, expected, tdiff = rng.choice(pool)(rng)
            # На всякий случай проверяем фактический тир шаблона.
            if tdiff != diff:
                continue
            # Защита от клонов бенчмарка: нормализованный промпт сверяем
            # с нормализованными промптами tasks.json.
            if normalize_prompt(prompt) in benchmark_norm:
                continue
            if name in used_names:
                continue
            used_names.add(name)
            return {"id": 0, "cat": cat, "name": name, "prompt": prompt,
                    "expected": expected, "difficulty": diff}
        return None

    # Стратификация: quota по категориям.
    base_cat, rem_cat = divmod(count + hold, len(CATEGORIES))
    cat_quota: Dict[str, int] = {}
    for ci, cat in enumerate(CATEGORIES):
        cat_quota[cat] = base_cat + (1 if ci < rem_cat else 0)

    # Тир-стратификация: на первом проходе на каждой категории квота
    # распределяется по тирам весами. Если конкретный тир насыщается (все его
    # шаблоны уже исчерпаны уникальными промптами), запросы автоматически
    # перераспределяются на другие тиры той же категории — так квота категории
    # заполняется гарантированно, а стратификация остаётся сбалансированной.
    task_id = 1
    for cat in CATEGORIES:
        available = [d for d in DIFFICULTIES if _TEMPLATES[cat][d]]
        target = _split_bucket(cat_quota[cat], available)
        remaining: Dict[str, int] = dict(target)
        produced = 0
        global_attempts = 0
        while produced < cat_quota[cat]:
            global_attempts += 1
            if global_attempts > 30000:
                raise RuntimeError(f"Не удалось заполнить квоту категории {cat}: "
                                   f"не хватает разнообразия шаблонов")
            # Выбираем тир с вероятностью, пропорциональной оставшейся квоте.
            active = [d for d in available if remaining.get(d, 0) > 0]
            if not active:
                active = list(available)
            weights = [max(remaining.get(d, 0), 1) for d in active]
            pick = rng.choices(active, weights=weights, k=1)[0]
            t = make(cat, pick)
            if t is None:
                # Тир насыщен — уменьшаем его квоту, чтобы чаще выбирать другие.
                remaining[pick] = max(0, remaining.get(pick, 0) - 1)
                continue
            t["id"] = task_id
            task_id += 1
            t["split"] = "pending"
            tasks.append(t)
            produced += 1
            if remaining.get(pick, 0) > 0:
                remaining[pick] -= 1

    # Разделятор: держим порядок стабильным, помечаем hold в конце.
    # Число hold задач берём из общего пула равномерно по категориям.
    hold_ids: Set[int] = set()
    if hold > 0:
        by_cat_ids: Dict[str, List[int]] = {c: [] for c in CATEGORIES}
        for t in tasks:
            by_cat_ids[t["cat"]].append(t["id"])
        base_h, rem_h = divmod(hold, len(CATEGORIES))
        for ci, cat in enumerate(CATEGORIES):
            picks = base_h + (1 if ci < rem_h else 0)
            chosen = rng.sample(by_cat_ids[cat], min(picks, len(by_cat_ids[cat])))
            hold_ids.update(chosen)

    train: List[Dict[str, Any]] = []
    hold_tasks: List[Dict[str, Any]] = []
    for t in tasks:
        if t["id"] in hold_ids:
            t["split"] = "hold"
            hold_tasks.append(t)
        else:
            t["split"] = "train"
            train.append(t)

    return train, hold_tasks


# ============================================================================
# РЕКОРД: задачи + результат решения
# ============================================================================

def empty_record(task: Dict[str, Any]) -> Dict[str, Any]:
    """Пустой рекорд для задачи (до генерации решения)."""
    return {"id": task["id"], "cat": task["cat"], "name": task["name"],
            "prompt": task["prompt"], "expected": task["expected"],
            "difficulty": task["difficulty"], "split": task["split"],
            "code": None, "passed": False, "pass1": False, "repaired": False,
            "attempts": 0, "error": ""}


# ============================================================================
# КЛИЕНТ LLM (OpenAI-compatible) + ВАЛИДАЦИЯ + SELF-REPAIR
# ============================================================================

def load_cheatsheet(path: Path = CHEATSHEET_PATH) -> str:
    """Прочитать текст шпаргалки Lupus для системного промпта учителя."""
    return Path(path).read_text(encoding="utf-8")


class LLMClient:
    """Клиент к OpenAI-compatible API через requests с ретраями и бэкоффом."""

    def __init__(self, base_url: str, api_key: str, model: str) -> None:
        if requests is None:
            raise RuntimeError("Пакет 'requests' не установлен. Установите: pip install requests")
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.model = model

    def call(self, system: str, user: str) -> str:
        """Один вызов модели с ретраями при 429/5xx/timeout (до API_RETRIES).

        БЕЗОПАСНО: `content` в ответе может быть None — берём msg.get("content")
        or "" и считаем такой ответ сбоем, НЕ падаем с AttributeError."""
        url = f"{self.base_url}/chat/completions"
        payload: Dict[str, Any] = {
            "model": self.model,
            "messages": [{"role": "system", "content": system},
                         {"role": "user", "content": user}],
            "temperature": 0.2,
            "max_tokens": 900,
        }
        headers = {"Authorization": f"Bearer {self.api_key}",
                   "Content-Type": "application/json"}
        last_err: Optional[str] = "unknown"
        for attempt in range(API_RETRIES):
            try:
                resp = requests.post(url, json=payload, headers=headers, timeout=120)
                if resp.status_code == 429 or resp.status_code >= 500:
                    last_err = f"HTTP {resp.status_code}"
                    time.sleep(2 ** attempt)
                    continue
                resp.raise_for_status()
                data = resp.json()
            except (requests.exceptions.Timeout, requests.exceptions.ConnectionError,
                    requests.exceptions.RequestException) as exc:
                last_err = str(exc)
                time.sleep(2 ** attempt)
                continue
            # Аккуратно извлекаем content: могут быть None/пустые поля.
            content: Optional[str] = None
            if isinstance(data, dict):
                choices = data.get("choices") or []
                if choices:
                    msg = choices[0].get("message") or {}
                    content = msg.get("content")  # может быть None — безопасно
            content = (content or "").strip()      # None -> "" и считаем сбоем
            if content:
                return content
            last_err = "пустой/отсутствующий content в ответе API"
            time.sleep(2 ** attempt)
        raise RuntimeError(f"Не удалось получить ответ LLM после {API_RETRIES} "
                           f"попыток: {last_err}")


def clean_code(code: str) -> str:
    """Убрать markdown-фенсы ```, если модель их добавила, и хвостовые пробелы."""
    code = code.strip()
    if code.startswith("```"):
        lines = code.splitlines()
        if lines and lines[0].strip().startswith("```"):
            lines = lines[1:]
        if lines and lines[-1].strip().startswith("```"):
            lines = lines[:-1]
        code = "\n".join(lines).strip()
    return code


def run_lupus(code: str, tmpdir: Path, task_id: int) -> Tuple[bool, str, str]:
    """Выполнить Lupus-код локальным интерпретатором lupus_proto.py
    в подпроцессе. Returns: (returncode==0, stdout, stderr_или_ошибка)."""
    fname = tmpdir / f"task_{task_id}.lupus"
    fname.write_text(code, encoding="utf-8")
    try:
        proc = subprocess.run([sys.executable, str(LUPUS_PROTO), str(fname)],
                              capture_output=True, text=True, timeout=VALIDATE_TIMEOUT)
    except subprocess.TimeoutExpired:
        return False, "", "timeout — программа не завершилась за отведённое время"
    # Интерпретатор печатает ошибки в stdout и завершается с кодом 1.
    return proc.returncode == 0, proc.stdout, proc.stderr


def solve_task(client: LLMClient, task: Dict[str, Any], system_teacher: str,
               tmpdir: Path, traces: List[Dict[str, Any]],
               traces_lock: threading.Lock) -> Dict[str, Any]:
    """Сгенерировать Lupus-решение для задачи с валидацией и self-repair
    (до MAX_REPAIRS ретраев). Возвращает рекорд Dict.

    Потокобезопасна: каждый вызов работает со своим рекордом; запись трейсов
    в общий список выполняется под блокировкой traces_lock."""
    rec = empty_record(task)
    bad_code: Optional[str] = None
    last_error: str = ""

    for attempt in range(MAX_REPAIRS + 1):
        user = task["prompt"]
        if attempt > 0:
            # Отправляем текст ошибки И ОЖИДАЕМЫЙ ВЫВОД обратно модели: без
            # expected ремонт идёт вслепую и не может понять причину
            # «output mismatch».
            user = (f"{task['prompt']}\n\nYour previous Lupus program produced "
                    f"the wrong output.\nExpected output:\n{task['expected']}\n"
                    f"What your program produced / the error:\n{last_error}\n"
                    f"Return the corrected code.")
        try:
            raw = client.call(system_teacher, user)
        except Exception as exc:
            # Сбой вызова LLM (в т.ч. пустой content) — считаем попытку неудачной,
            # не роняем весь прогон.
            rec["attempts"] += 1
            rec["error"] = f"LLM call failed: {exc}"
            last_error = f"LLM call failed: {exc}"
            time.sleep(0.1)
            continue

        if not isinstance(raw, str) or not raw.strip():
            rec["attempts"] += 1
            rec["error"] = "LLM вернул пустой ответ"
            last_error = "empty response"
            time.sleep(0.1)
            continue

        code = clean_code(raw)
        rec["attempts"] += 1
        ok, out, err = run_lupus(code, tmpdir, task["id"])

        if ok and out.strip() == task["expected"].strip():
            rec["code"] = code
            rec["passed"] = True
            if attempt == 0:
                rec["pass1"] = True
            else:
                rec["repaired"] = True
                # Трейс успешного self-repair.
                with traces_lock:
                    traces.append({"task_id": task["id"], "cat": task["cat"],
                                   "prompt": task["prompt"],
                                   "bad_code": bad_code, "error": last_error,
                                   "fixed_code": code})
            break

        # Фиксируем ошибку для следующей итерации self-repair.
        bad_code = code
        last_error = (err or out).strip() or "output mismatch"
        rec["error"] = last_error
        time.sleep(0.1)

    return rec


# ============================================================================
# ЧЕКПОИНТ, ЭКСПОРТ, СТАТИСТИКА
# ============================================================================

def load_checkpoint(path: Path = DATASET_PATH) -> Dict[int, Dict[str, Any]]:
    """Загрузить чекпоинт dataset_lupus.json -> dict по task id (для --resume)."""
    result: Dict[int, Dict[str, Any]] = {}
    if not path.exists():
        return result
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return result
    for rec in data:
        if isinstance(rec, dict) and "id" in rec:
            result[int(rec["id"])] = rec
    return result


def write_checkpoint(train: List[Dict[str, Any]], results: Dict[int, Dict[str, Any]],
                     hold_tasks: List[Dict[str, Any]], traces: List[Dict[str, Any]],
                     args: argparse.Namespace, cheatsheet: str) -> None:
    """Записать все итоговые файлы (чекпоинт / финальный экспорт)."""
    # 1) dataset_lupus.json — чекпоинт всех train-задач с рекордами.
    payload = []
    for t in train:
        rec = results.get(t["id"])
        payload.append(rec if rec is not None else empty_record(t))
    DATASET_PATH.write_text(json.dumps(payload, ensure_ascii=False, indent=2),
                            encoding="utf-8")

    # 2) train_lupus_bare.jsonl — короткий system + промпт.
    write_train_bare(train, results)
    # 3) train_lupus_sheet.jsonl — system + шпаргалка + промпт.
    write_train_sheet(train, results, cheatsheet)
    # 4) hold_lupus.json — hold-задачи с expected.
    write_hold(hold_tasks)
    # 5) repair_traces.jsonl.
    write_traces(traces)
    # 6) stats.json.
    write_stats(train, results, hold_tasks, args)


def _passed_train(train: List[Dict[str, Any]], results: Dict[int, Dict[str, Any]]
                  ) -> List[Dict[str, Any]]:
    """Только train-задачи, чьи Lupus-решения прошли валидацию."""
    out: List[Dict[str, Any]] = []
    for t in train:
        rec = results.get(t["id"])
        if rec and rec.get("passed") and rec.get("code"):
            out.append(rec)
    return out


def write_train_bare(train: List[Dict[str, Any]], results: Dict[int, Dict[str, Any]]) -> None:
    """train_lupus_bare.jsonl: instruction = короткий system + промпт,
    output = Lupus-код."""
    lines: List[str] = []
    for rec in _passed_train(train, results):
        instruction = f"{SYSTEM_BARE}\n\n{rec['prompt']}"
        lines.append(json.dumps({"instruction": instruction, "output": rec["code"]},
                                ensure_ascii=False))
    (THIS_DIR / "train_lupus_bare.jsonl").write_text(
        "\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")


def write_train_sheet(train: List[Dict[str, Any]], results: Dict[int, Dict[str, Any]],
                      cheatsheet: str) -> None:
    """train_lupus_sheet.jsonl: instruction = system + шпаргалка + промпт,
    output = тот же Lupus-код."""
    lines: List[str] = []
    for rec in _passed_train(train, results):
        instruction = f"{SYSTEM_BARE}\n\n{cheatsheet}\n\n{rec['prompt']}"
        lines.append(json.dumps({"instruction": instruction, "output": rec["code"]},
                                ensure_ascii=False))
    (THIS_DIR / "train_lupus_sheet.jsonl").write_text(
        "\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")


def write_hold(hold_tasks: List[Dict[str, Any]]) -> None:
    """hold_lupus.json — hold-задачи с expected (для оценки без подгонки)."""
    items = [{"id": t["id"], "cat": t["cat"], "name": t["name"],
              "prompt": t["prompt"], "expected": t["expected"],
              "difficulty": t["difficulty"]} for t in hold_tasks]
    (THIS_DIR / "hold_lupus.json").write_text(
        json.dumps(items, ensure_ascii=False, indent=2), encoding="utf-8")


def write_traces(traces: List[Dict[str, Any]]) -> None:
    """repair_traces.jsonl — трейсы успешных self-repair."""
    lines = [json.dumps(t, ensure_ascii=False) for t in traces]
    (THIS_DIR / "repair_traces.jsonl").write_text(
        "\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")


def _bucket_stats(recs: List[Dict[str, Any]], key: str, order: List[str]) -> Dict[str, Any]:
    """Разбивка pass@1/pass/repair по значению key (cat или difficulty)."""
    out: Dict[str, Any] = {}
    for val in order:
        bucket = [r for r in recs if r.get(key) == val]
        if not bucket:
            continue
        n = len(bucket)
        pass1 = sum(1 for r in bucket if r.get("pass1"))
        passed = sum(1 for r in bucket if r.get("passed"))
        repairs = sum(1 for r in bucket if r.get("repaired"))
        out[val] = {
            "count": n,
            "pass_at_1": round(pass1 / n, 4),
            "pass_rate": round(passed / n, 4),
            "repair_share": round(repairs / n, 4),
        }
    return out


def write_stats(train: List[Dict[str, Any]], results: Dict[int, Dict[str, Any]],
                hold_tasks: List[Dict[str, Any]], args: argparse.Namespace) -> None:
    """stats.json: pass@1, доля repairs, разбивка по категориям/сложности."""
    recs = [r for t in train if (r := results.get(t["id"])) is not None]
    attempted = [r for r in recs if r.get("attempts", 0) > 0]
    n = len(attempted)
    passed = [r for r in attempted if r.get("passed")]
    pass1 = sum(1 for r in attempted if r.get("pass1"))
    repairs = sum(1 for r in attempted if r.get("repaired"))

    stats: Dict[str, Any] = {
        "config": {
            "count_train": len(train),
            "count_hold": len(hold_tasks),
            "model": args.model,
            "workers": args.workers,
            "seed": args.seed,
            "templates": _count_templates(),
        },
        "train": {
            "total": len(train),
            "attempted": n,
            "passed": len(passed),
            "dataset_size": len(_passed_train(train, results)),
            "pass_rate": round(len(passed) / n, 4) if n else 0.0,
        },
        "pass_at_1": {
            "count": pass1,
            "rate": round(pass1 / n, 4) if n else 0.0,
        },
        "repairs": {
            "count": repairs,
            "share": round(repairs / n, 4) if n else 0.0,
        },
        "by_cat": _bucket_stats(attempted, "cat", CATEGORIES),
        "by_difficulty": _bucket_stats(attempted, "difficulty", DIFFICULTIES),
        "hold_count": len(hold_tasks),
    }
    (THIS_DIR / "stats.json").write_text(
        json.dumps(stats, ensure_ascii=False, indent=2), encoding="utf-8")


def print_summary(train: List[Dict[str, Any]], results: Dict[int, Dict[str, Any]],
                  hold_tasks: List[Dict[str, Any]], args: argparse.Namespace) -> None:
    """Вывести итоговую сводку прогона в консоль."""
    recs = [r for t in train if (r := results.get(t["id"])) is not None]
    attempted = [r for r in recs if r.get("attempts", 0) > 0]
    n = len(attempted)
    passed = sum(1 for r in attempted if r.get("passed"))
    pass1 = sum(1 for r in attempted if r.get("pass1"))
    repairs = sum(1 for r in attempted if r.get("repaired"))
    print("\n" + "=" * 72)
    print("ИТОГОВАЯ СВОДКА (Lupus)")
    print("=" * 72)
    if n == 0:
        print("Нет обработанных задач.")
        return
    print(f"Train задач: {len(train)} | Hold: {len(hold_tasks)} | "
          f"Обработано: {n}")
    print(f"Прошло валидацию (датасет): {passed} ({passed / n:.1%})")
    print(f"pass@1  : {pass1} ({pass1 / n:.1%})")
    print(f"repairs : {repairs} (доля {repairs / n:.1%})")
    print(f"Шаблонов: {_count_templates()} | workers: {args.workers} | "
          f"model: {args.model}")
    print("=" * 72)


# ============================================================================
# ОСНОВНОЙ КОНВЕЙЕР (ПАРАЛЛЕЛЬНЫЙ)
# ============================================================================

def run_pipeline(args: argparse.Namespace) -> None:
    """Полный конвейер: генерация задач -> LLM-решения (параллельно) ->
    валидация/self-repair -> чекпоинты -> экспорт."""
    benchmark_norm = load_benchmark_normalized()
    train, hold_tasks = generate_tasks(args.count, args.hold, args.seed,
                                       benchmark_norm)
    print(f"[1/3] Шаблонов: {_count_templates()} | Train: {len(train)} | "
          f"Hold: {len(hold_tasks)} (seed={args.seed})")

    if not LUPUS_PROTO.exists():
        sys.stderr.write(f"ОШИБКА: интерпретатор не найден: {LUPUS_PROTO}\n")
        sys.exit(2)
    cheatsheet = load_cheatsheet()
    system_teacher = f"{cheatsheet}\n\n{SYSTEM_BARE}"

    client = LLMClient(args.api_base, args.api_key, args.model)

    # Resume: подхватываем уже обработанные задачи.
    resumed = load_checkpoint() if args.resume else {}
    results: Dict[int, Dict[str, Any]] = {}
    pending: List[Dict[str, Any]] = []
    for t in train:
        rec = resumed.get(t["id"])
        # Возобновляем только задачи, которые РЕАЛЬНО попытались решить
        # (attempts > 0). Пустые рекорды (например, из-за --limit или прерывания)
        # не считаются готовыми — их нужно догенерировать.
        if rec is not None and rec.get("attempts", 0) > 0:
            # На всякий случай возвращаем актуальные поля задачи.
            rec.setdefault("split", t["split"])
            rec.setdefault("difficulty", t["difficulty"])
            results[t["id"]] = rec
        else:
            pending.append(t)

    print(f"[2/3] Модель: {args.model} | workers: {args.workers} | "
          f"Уже обработано: {len(results)} | Новых: {len(pending)}")
    if args.limit and 0 < args.limit < len(pending):
        pending = pending[:args.limit]
        print(f"      --limit={args.limit} — обработаем только первые "
              f"{len(pending)} новых задач.")

    traces: List[Dict[str, Any]] = []
    traces_lock = threading.Lock()
    total_done = len(results)
    checkpoint_marker = total_done - (total_done % CHECKPOINT_EVERY)

    # Параллельная обработка через ThreadPoolExecutor.
    with tempfile.TemporaryDirectory(prefix="lupus_gen_") as td:
        tmpdir = Path(td)
        with ThreadPoolExecutor(max_workers=args.workers) as executor:
            futures = {executor.submit(solve_task, client, t, system_teacher,
                                       tmpdir, traces, traces_lock): t
                       for t in pending}
            for fut in as_completed(futures):
                t = futures[fut]
                try:
                    rec = fut.result()
                except Exception as exc:  # защита от падений воркера
                    rec = empty_record(t)
                    rec["error"] = f"worker exception: {exc}"
                results[t["id"]] = rec
                total_done += 1
                if total_done % 50 == 0 or total_done == len(train):
                    print(f"      ...обработано {total_done}/{len(train)} "
                          f"(passed={sum(1 for r in results.values() if r.get('passed'))})")
                # Периодический чекпоинт каждые CHECKPOINT_EVERY задач.
                if total_done - checkpoint_marker >= CHECKPOINT_EVERY:
                    checkpoint_marker = total_done - (total_done % CHECKPOINT_EVERY)
                    write_checkpoint(train, results, hold_tasks, traces, args,
                                     cheatsheet)
                    print(f"      ...чекпоинт сохранён ({total_done} задач)")

    print("[3/3] Финальный экспорт...")
    write_checkpoint(train, results, hold_tasks, traces, args, cheatsheet)
    print_summary(train, results, hold_tasks, args)


def parse_args(argv: Optional[List[str]] = None) -> argparse.Namespace:
    """Разобрать аргументы командной строки."""
    parser = argparse.ArgumentParser(
        description="Генератор ОБУЧАЮЩЕГО датасета по языку Lupus для LoRA.")
    parser.add_argument("--count", type=int, default=3000,
                        help="Число train-задач (default: 3000)")
    parser.add_argument("--hold", type=int, default=300,
                        help="Число hold-задач (default: 300)")
    parser.add_argument("--seed", type=int, default=42,
                        help="Зерно RNG для детерминизма (default: 42)")
    parser.add_argument("--workers", type=int, default=8,
                        help="Число потоков ThreadPoolExecutor (default: 8)")
    parser.add_argument("--resume", action="store_true",
                        help="Продолжить с чекпоинта dataset_lupus.json")
    parser.add_argument("--limit", type=int, default=0,
                        help="Ограничить число НОВЫХ задач (0 = без лимита; "
                             "полезно для теста)")
    parser.add_argument("--api-base", default=os.environ.get("LUPUS_API_BASE", ""),
                        help="Базовый URL OpenAI-compatible API (env LUPUS_API_BASE)")
    parser.add_argument("--api-key", default=os.environ.get("LUPUS_API_KEY", ""),
                        help="API-ключ (env LUPUS_API_KEY)")
    parser.add_argument("--model", default=os.environ.get("LUPUS_MODEL", DEFAULT_MODEL),
                        help=f"Модель (env LUPUS_MODEL, default: {DEFAULT_MODEL})")
    return parser.parse_args(argv)


def main(argv: Optional[List[str]] = None) -> None:
    """Точка входа."""
    args = parse_args(argv)
    if not args.api_base or not args.api_key:
        sys.stderr.write("ОШИБКА: задайте LUPUS_API_BASE и LUPUS_API_KEY "
                         "(или флаги --api-base/--api-key).\n")
        sys.exit(2)
    try:
        run_pipeline(args)
    except KeyboardInterrupt:
        print("\nПрервано пользователем. Чекпоинт можно продолжить через --resume.")
        sys.exit(130)


if __name__ == "__main__":
    main()
