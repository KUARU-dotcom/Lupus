#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Lupus Experiment — Multi-Model OpenAI-Compatible Runner

Пример запуска:
    python run_experiment_Model_custom.py --list-models
    python run_experiment_Model_custom.py --model merged-lupus-2bV2
    python run_experiment_Model_custom.py --all-models
    python run_experiment_Model_custom.py --model merged-lupus-2bV2 --resume
    python run_experiment_Model_custom.py --model merged-python-2b --ids 1,5,10
"""

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from datetime import datetime
from urllib import request as urllib_request
from urllib.error import URLError

# ─── Конфигурация моделей через OpenAI-compatible API ────────────────────────
MODELS_TO_TEST = [
    {
        "name": "Qwen3.8 4B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "max_tokens": 700,
        "disable_thinking": True,
    },
    {
        "name": "Merged Lupus 2B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "max_tokens": 700,
        "disable_thinking": True,
    },
    {
        "name": "Merged Lupus 2Bv2",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "max_tokens": 700,
        "disable_thinking": True,
    },
    {
        "name": "Qwen3.5 2B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "max_tokens": 700,
        "disable_thinking": True,
    },
    {
        "name": "Merged Python 2B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "max_tokens": 700,
        "disable_thinking": True,
    },
    {
        "name": "Bonsai 27B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "max_tokens": 700,
        "disable_thinking": True,
    },
    {
        "name": "Qwen3.5 9B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "max_tokens": 700,
        "disable_thinking": True,
    },
    {
        "name": "Qwen3.5 4B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "max_tokens": 700,
        "disable_thinking": True,
    },
    {
        "name": "MiniCPM5 2B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "max_tokens": 700,
        "disable_thinking": True,
    },
]

# ─── Константы запуска ─────────────────────────────────────────────────────────
TEMPERATURE = 0.1
TIMEOUT_API = 90
TIMEOUT_RUN = 10
INTERPRETER = os.path.join(os.path.dirname(__file__), "lupus_proto.py")
TASKS_FILE = os.path.join(os.path.dirname(__file__), "tasks.json")
RETRY_COUNT = 2

# ─── Глобальная переменная: текущая конфигурация модели и файл результатов ───
current_model_name = None
current_model_config = None
current_results_file = None
last_api_error = ""

LUPUS_SYSTEM = (
    "You are an expert Lupus programmer. Write ONLY valid Lupus code to solve the task. "
    "No markdown, no explanation, no code blocks. Just the code. "
    "Use only the task text; do not use or expect a cheat sheet or reference material."
)

PYTHON_SYSTEM = (
    "You are an expert Python programmer. Write ONLY valid Python 3 code to solve the task. "
    "No markdown, no explanation, no code blocks. Use print() for output. "
    "Use only the task text; do not use or expect a cheat sheet or reference material."
)


def _normalize_model_entry(entry: dict) -> dict:
    """Нормализует запись модели из списка MODELS_TO_TEST."""
    name = entry.get("name") or entry.get("model_name") or entry.get("model")
    if not name:
        raise ValueError(f"Модель в конфиге без имени: {entry!r}")

    api_url = entry.get("api_url") or entry.get("base_url") or "http://localhost:1234/v1"
    if api_url.endswith("/chat/completions"):
        chat_url = api_url
    elif api_url.endswith("/v1"):
        chat_url = f"{api_url.rstrip('/')}/chat/completions"
    else:
        chat_url = f"{api_url.rstrip('/')}/chat/completions"

    api_key = entry.get("api_key") or os.environ.get("OPENAI_API_KEY") or "lm-studio"
    model_id = entry.get("model") or entry.get("model_name") or name

    return {
        "name": name,
        "model": model_id,
        "api_url": chat_url,
        "api_key": api_key,
        "max_tokens": int(entry.get("max_tokens", 700)),
        "disable_thinking": bool(entry.get("disable_thinking", True)),
    }


def build_models_registry() -> dict:
    registry = {}
    for entry in MODELS_TO_TEST:
        cfg = _normalize_model_entry(entry)
        registry[cfg["name"]] = cfg
    return registry


MODELS = build_models_registry()


def set_current_model(model_name: str):
    """Устанавливает текущую модель и путь к файлу результатов."""
    global current_model_name, current_model_config, current_results_file
    if model_name not in MODELS:
        return False
    current_model_name = model_name
    current_model_config = MODELS[model_name]
    current_results_file = os.path.join(os.path.dirname(__file__), f"results_{model_name}.json")
    return True


def model_language(model_name: str | None = None) -> str:
    """Определяет язык модели по имени: lupus или python."""
    name = (model_name or current_model_name or "").lower()
    if "python" in name:
        return "python"
    if "lupus" in name:
        return "lupus"
    raise ValueError(f"Невозможно определить язык модели: {model_name or current_model_name!r}")


def system_prompt_for_model(model_name: str | None = None) -> str:
    """Возвращает системный промпт только под нужный язык."""
    lang = model_language(model_name)
    if lang == "lupus":
        return LUPUS_SYSTEM
    return PYTHON_SYSTEM


def call_model(system: str, prompt: str) -> str | None:
    global last_api_error
    if current_model_config is None:
        return None

    last_api_error = ""
    user_prompt = prompt
    payload_data = {
        "model": current_model_config["model"],
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user_prompt},
        ],
        "temperature": 0.1,
        "max_tokens": current_model_config["max_tokens"],
        "stream": False,
        "chat_template_kwargs": {"enable_thinking": False},
    }

    payload_variants = [payload_data]
    if current_model_config.get("disable_thinking", False):
        alt_prompt = user_prompt + "\n/no_think"
        alt_payload = {
            "model": current_model_config["model"],
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": alt_prompt},
            ],
            "temperature": 0.1,
            "max_tokens": current_model_config["max_tokens"],
            "stream": False,
            "chat_template_kwargs": {"enable_thinking": False},
        }
        payload_variants.append(alt_payload)

    for attempt in range(RETRY_COUNT):
        for payload in payload_variants:
            try:
                payload_json = json.dumps(payload).encode("utf-8")
                req = urllib_request.Request(
                    current_model_config["api_url"],
                    data=payload_json,
                    headers={
                        "Content-Type": "application/json",
                        "Authorization": f"Bearer {current_model_config['api_key']}",
                    },
                    method="POST",
                )
                with urllib_request.urlopen(req, timeout=TIMEOUT_API) as resp:
                    data = json.loads(resp.read().decode("utf-8"))
                    if "choices" not in data or not data["choices"]:
                        last_api_error = f"Пустой ответ API: {data}"
                        return None
                    return data["choices"][0]["message"]["content"]
            except urllib_request.HTTPError as e:
                body = e.read().decode("utf-8", errors="ignore")
                last_api_error = f"HTTP {e.code}: {body[:500]}"
            except URLError as e:
                last_api_error = f"URLError: {e}"
            except Exception as e:
                last_api_error = f"{type(e).__name__}: {e}"

        if attempt != RETRY_COUNT - 1:
            time.sleep(2)

    return None


def strip_thinking(text: str) -> str:
    """Убирает <think>...</think> блоки Qwen3."""
    return re.sub(r"<think>.*?</think>", "", text, flags=re.DOTALL).strip()


def extract_code(text: str) -> str:
    text = strip_thinking(text)
    m = re.search(r"```(?:lupus|python|lisp|scheme|py)?\s*\n?(.*?)```", text, re.DOTALL)
    if m:
        return m.group(1).strip()
    text = re.sub(r"`([^`\n]+)`", r"\1", text)
    lines = [
        line
        for line in text.splitlines()
        if not re.match(r"^\s*(This|Here|The|Note|Output|Result)\b", line)
    ]
    return "\n".join(lines).strip()


def run_lupus(code: str) -> tuple[str, str]:
    """Возвращает (status, output): status = 'ok'|'error'|'timeout'"""
    tmp = os.path.join(tempfile.gettempdir(), "_lupus_exp.lupus")
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(code)
    try:
        r = subprocess.run(
            [sys.executable, INTERPRETER, tmp],
            capture_output=True,
            text=True,
            timeout=TIMEOUT_RUN,
        )
        out = r.stdout.strip()
        if r.returncode != 0 or "Ошибка" in out:
            return "error", out
        return "ok", out
    except subprocess.TimeoutExpired:
        return "timeout", "TIMEOUT"
    except Exception as e:
        return "error", str(e)


def run_python(code: str) -> tuple[str, str]:
    tmp = os.path.join(tempfile.gettempdir(), "_python_exp.py")
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(code)
    try:
        r = subprocess.run(
            [sys.executable, tmp],
            capture_output=True,
            text=True,
            timeout=TIMEOUT_RUN,
        )
        out = r.stdout.strip()
        if r.returncode != 0:
            return "error", (r.stderr.strip() or out)[:120]
        return "ok", out
    except subprocess.TimeoutExpired:
        return "timeout", "TIMEOUT"
    except Exception as e:
        return "error", str(e)


def check(got: str, expected: str) -> bool:
    return got.strip() == expected.strip()


G = lambda t: f"\033[32m{t}\033[0m"
R = lambda t: f"\033[31m{t}\033[0m"
Y = lambda t: f"\033[33m{t}\033[0m"
B = lambda t: f"\033[1m{t}\033[0m"
D = lambda t: f"\033[2m{t}\033[0m"


def load_results() -> dict:
    if current_results_file and os.path.exists(current_results_file):
        with open(current_results_file, encoding="utf-8") as f:
            return json.load(f)
    return {}


def save_results(results: dict):
    if current_results_file:
        with open(current_results_file, "w", encoding="utf-8") as f:
            json.dump(results, f, ensure_ascii=False, indent=2)


def run_task(task: dict) -> dict:
    lang = model_language()
    runner = run_lupus if lang == "lupus" else run_python
    system = system_prompt_for_model()

    row = {
        "id": task["id"],
        "cat": task["cat"],
        "name": task["name"],
        "expected": task["expected"],
        "ts": datetime.now().isoformat(),
    }

    t0 = time.time()
    raw = call_model(system, task["prompt"])
    if raw is None:
        row[lang] = {"status": "api_error", "code": "", "output": "", "ms": 0}
        return row

    code = extract_code(raw)
    gen_ms = int((time.time() - t0) * 1000)

    t1 = time.time()
    run_status, output = runner(code)
    run_ms = int((time.time() - t1) * 1000)

    correct = check(output, task["expected"])
    status = "pass" if correct else ("wrong" if run_status == "ok" else run_status)

    row[lang] = {
        "status": status,
        "code": code,
        "output": output,
        "ms": gen_ms + run_ms,
    }

    return row


def list_models():
    print()
    print(B("Доступные модели:"))
    print()
    print(f"  {'Имя':28s}  {'api_url':45s}  {'max_tokens':11s}")
    print(f"  {'-'*28}  {'-'*45}  {'-'*11}")
    for name, cfg in MODELS.items():
        print(f"  {name:28s}  {cfg['api_url']:45s}  {cfg['max_tokens']:11d}")
    print()


def print_report(results: dict):
    rows = list(results.values())
    if not rows:
        print("Нет данных.")
        return

    lang = model_language()
    total = len(rows)
    print()
    print(B("=" * 80))
    print(B(f"  ОТЧЁТ ПО ЭКСПЕРИМЕНТУ — {current_model_name.upper()}"))
    print(B(f"  Результаты: {current_results_file}"))
    print(B(f"  Язык: {lang.upper()}"))
    print(B("=" * 80))

    print()
    print(B("  Общий результат:"))
    passed = sum(1 for r in rows if r.get(lang, {}).get("status") == "pass")
    wrong = sum(1 for r in rows if r.get(lang, {}).get("status") == "wrong")
    errors = sum(1 for r in rows if r.get(lang, {}).get("status") in ("error", "timeout"))
    api_e = sum(1 for r in rows if r.get(lang, {}).get("status") == "api_error")
    pct = passed / total * 100 if total else 0
    bar = G("█") * passed + D("░") * (total - passed)
    label = "Lupus" if lang == "lupus" else "Python"
    print(f"  {B(label):8s}  {bar}  {G(f'{passed}/{total}')} ({pct:.1f}%)")
    if wrong:
        print(f"           Неверный вывод:  {Y(str(wrong))}")
    if errors:
        print(f"           Ошибки запуска:  {R(str(errors))}")
    if api_e:
        print(f"           Ошибки API:      {R(str(api_e))}")

    print()
    print(B("  По категориям:"))
    cats = sorted(set(r["cat"] for r in rows))
    print(f"  {'Категория':14s}  {label:^12s}")
    print(f"  {'-'*14}  {'-'*12}")
    for cat in cats:
        cat_rows = [r for r in rows if r["cat"] == cat]
        n = len(cat_rows)
        p = sum(1 for r in cat_rows if r.get(lang, {}).get("status") == "pass")
        stat = G(f"{p}/{n}") if p == n else (Y(f"{p}/{n}") if p > 0 else R(f"{p}/{n}"))
        print(f"  {cat:14s}  {stat:^21s}")

    print()
    print(B("  Итог:"))
    print(f"  {label}: {passed}/{total}  ({pct:.1f}%)")

    fails = [r for r in rows if r.get(lang, {}).get("status") != "pass"]
    if fails:
        print()
        print(B(f"  Провалы {label} ({len(fails)}):"))
        for r in fails[:10]:
            info = r.get(lang, {})
            st = {"wrong": "⚠️ ", "error": "❌ ", "timeout": "⏱️ ", "api_error": "🔌"}.get(info.get("status", ""), "? ")
            print(f"  {st} #{r['id']:3d} [{r['cat']:10s}] {r['name']}")
            print(f"       Ожидалось: '{r['expected']}'")
            print(f"       Получено:  '{info.get('output', '')[:60]}'")
        if len(fails) > 10:
            print(f"  ... и ещё {len(fails)-10}")

    print()
    print(B("=" * 80))
    print(f"  Результаты сохранены: {current_results_file}")
    print()


def run_experiment(task_ids: list[int] | None, resume: bool, report_only: bool):
    with open(TASKS_FILE, encoding="utf-8") as f:
        all_tasks = json.load(f)

    results = load_results()

    if report_only:
        print_report(results)
        return

    tasks = all_tasks
    if task_ids:
        tasks = [t for t in tasks if t["id"] in task_ids]
    if resume:
        tasks = [t for t in tasks if str(t["id"]) not in results]

    if not tasks:
        print("Все задачи уже выполнены. Используйте --report для отчёта.")
        print_report(results)
        return

    lang = model_language()
    print(f"\n🔌 Проверяем {current_model_name} ({lang})...", end=" ", flush=True)
    system = system_prompt_for_model()
    test = call_model(system, "ok")
    if test is None:
        print(R("ОШИБКА"))
        print(f"\n  Причина: {last_api_error or 'неизвестна'}")
        print(f"\n  Нужно:\n  1. Проверить доступность {current_model_config['api_url']}\n  2. Проверить API ключ: {current_model_config['api_key'][:20]}...\n  3. Проверить, что модель '{current_model_name}' загружена в LM Studio")
        sys.exit(1)
    print(G("OK"))

    total = len(tasks)
    print(f"\n{B('='*80)}")
    print(f"{B('  LUPUS EXPERIMENT — BATCH RUNNER')}")
    print(f"  Модель: {current_model_name}")
    print(f"  Язык: {lang.upper()}")
    print(f"  Результаты: {current_results_file}")
    print(f"  Задач к выполнению: {total}")
    print(f"  Режим: {'продолжение' if resume else 'полный прогон'}")
    print(f"{B('='*80)}\n")

    for i, task in enumerate(tasks, 1):
        progress = f"[{i:3d}/{total}]"
        tid = task["id"]
        tcat = task["cat"]
        tname = task["name"]
        print(f"{D(progress)} {B(f'#{tid:3d}')} {tcat:10s} — {tname}")

        row = run_task(task)
        results[str(task["id"])] = row
        save_results(results)

        info = row.get(lang, {})
        st = info.get("status", "?")
        ms = info.get("ms", 0)
        out = info.get("output", "")[:40]
        if st == "pass":
            sym = G("✅ PASS")
        elif st == "wrong":
            sym = Y(f"⚠️  WRONG → '{out}'")
        elif st in ("error", "timeout"):
            sym = R(f"❌ {st.upper()} → '{out}'")
        else:
            sym = R("🔌 API ERROR")
        label = "Lupus" if lang == "lupus" else "Python"
        print(f"         {label}: {sym} {D(f'({ms}ms)')}")
        print()

    print_report(results)


def run_for_model(model_name: str, task_ids: list[int] | None, resume: bool, report_only: bool):
    if not set_current_model(model_name):
        print(R(f"Ошибка: модель '{model_name}' не найдена."))
        print("\nДоступные модели:")
        for name in MODELS.keys():
            print(f"  - {name}")
        sys.exit(1)
    run_experiment(task_ids=task_ids, resume=resume, report_only=report_only)


def run_all_models(task_ids: list[int] | None, resume: bool, report_only: bool):
    for name in MODELS.keys():
        print(f"\n{B('═' * 90)}")
        print(f"{B(f'  Запуск модели: {name}')}")
        print(f"{B('═' * 90)}")
        run_for_model(name, task_ids=task_ids, resume=resume, report_only=report_only)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Lupus Experiment Batch Runner for OpenAI-compatible models")
    parser.add_argument("--model", type=str, default="", help="Имя модели из MODELS_TO_TEST")
    parser.add_argument("--all-models", action="store_true", help="Запустить эксперимент для всех моделей из списка")
    parser.add_argument("--list-models", action="store_true", help="Показать список доступных моделей")
    parser.add_argument("--resume", action="store_true", help="Пропустить уже выполненные задачи")
    parser.add_argument("--report", action="store_true", help="Только вывести отчёт")
    parser.add_argument("--ids", type=str, default="", help="Конкретные ID через запятую: 1,5,10")
    parser.add_argument("--cat", type=str, default="", help="Только категория: arithmetic|logic|loops|recursion|lists|strings|combined")
    args = parser.parse_args()

    if args.list_models:
        list_models()
        sys.exit(0)

    if not MODELS:
        print(R("Ошибка: MODELS_TO_TEST пуст.") )
        sys.exit(1)

    ids = [int(x.strip()) for x in args.ids.split(",") if x.strip()] if args.ids else None

    if args.cat:
        with open(TASKS_FILE, encoding="utf-8") as f:
            all_t = json.load(f)
        cat_ids = [t["id"] for t in all_t if t["cat"] == args.cat]
        ids = cat_ids if not ids else [x for x in ids if x in cat_ids]

    if args.all_models:
        run_all_models(task_ids=ids, resume=args.resume, report_only=args.report)
        sys.exit(0)

    if not args.model:
        if len(MODELS) == 1:
            model_name = next(iter(MODELS.keys()))
        else:
            print(R("Ошибка: не указана модель."))
            print("\nДоступные модели:")
            for name in MODELS.keys():
                print(f"  - {name}")
            print("\nИспользуйте: python run_experiment_Model_custom.py --model <имя> [опции]")
            print("Или: python run_experiment_Model_custom.py --all-models")
            sys.exit(1)
    else:
        model_name = args.model

    run_for_model(model_name, task_ids=ids, resume=args.resume, report_only=args.report)
