#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Генератор отдельных раннеров под каждую модель. Запуск: python make_runners.py"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(HERE, "run_experiment.py"), encoding="utf-8") as f:
    BASE = f.read()

CONFIGS = [
    dict(file="run_experiment_qwen35_4b.py",
         url="http://localhost:1234/v1/chat/completions",
         key="sk-lm-kJIgUah0:sI0VU9qdjO2tv8lZaRis",
         model="qwen3.5-4b", max_tokens=700,
         results="results_qwen35_4b.json",
         check="Проверяем LM Studio (qwen3.5-4b)"),
    dict(file="run_experiment_qwen35_9b.py",
         url="http://localhost:1234/v1/chat/completions",
         key="sk-lm-kJIgUah0:sI0VU9qdjO2tv8lZaRis",
         model="qwen3.5-9b", max_tokens=700,
         results="results_qwen35_9b.json",
         check="Проверяем LM Studio (qwen3.5-9b)"),
    dict(file="run_experiment_qwen38_4b.py",
         url="http://localhost:1234/v1/chat/completions",
         key="sk-lm-kJIgUah0:sI0VU9qdjO2tv8lZaRis",
         model="qwen3.8-4b-distill", max_tokens=700,
         results="results_qwen38_4b.json",
         check="Проверяем LM Studio (qwen3.8-4b-distill)"),
    dict(file="run_experiment_deepseek.py",
         url="https://knyazevai.work/v1/chat/completions",
         key="",   # ← СЮДА после генерации вставишь ключ knyazevai.work
         model="deepseek-v4-flash", max_tokens=2000,
         results="results_deepseek_v4_flash.json",
         check="Проверяем knyazevai.work (deepseek-v4-flash)"),
]

def generate(c):
    hits = {k: 0 for k in ("url", "max", "res", "model", "hdr", "check")}
    out = []
    for line in BASE.splitlines(keepends=True):
        s = line.strip()
        if s.startswith("API_URL"):
            hits["url"] += 1
            line = f'API_URL      = "{c["url"]}"\nAPI_KEY      = "{c["key"]}"\n'
        elif s.startswith("MAX_TOKENS"):
            hits["max"] += 1
            line = f'MAX_TOKENS   = {c["max_tokens"]}\n'
        elif s.startswith("RESULTS_FILE"):
            hits["res"] += 1
            line = f'RESULTS_FILE = os.path.join(os.path.dirname(__file__), "{c["results"]}")\n'
        elif '"model": "local-model"' in s:
            hits["model"] += 1
            line = line.replace('"local-model"', f'"{c["model"]}"')
        elif s.startswith('headers={"Content-Type": "application/json"},'):
            hits["hdr"] += 1
            line = ('                headers={"Content-Type": "application/json", '
                    '"Authorization": f"Bearer {API_KEY}"},\n')
        elif "Проверяем LM Studio" in s:
            hits["check"] += 1
            line = line.replace("Проверяем LM Studio", c["check"])
        out.append(line)
    missing = [k for k, v in hits.items() if v != 1]
    if missing:
        raise RuntimeError(f'[{c["file"]}] не найдены места замены: {missing}')
    return "".join(out)

for c in CONFIGS:
    with open(os.path.join(HERE, c["file"]), "w", encoding="utf-8") as f:
        f.write(generate(c))
    print(f"✅ создан {c['file']}")
print("\nГотово! Не забудь вставить ключ в run_experiment_deepseek.py (поле API_KEY).")
