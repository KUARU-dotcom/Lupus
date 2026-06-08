#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Lupus Experiment — Batch Runner
Запуск: python run_experiment.py
        python run_experiment.py --resume      (продолжить с места остановки)
        python run_experiment.py --ids 1,5,10  (только конкретные задачи)
        python run_experiment.py --cat loops   (только категория)
        python run_experiment.py --report      (только отчёт из сохранённых данных)
"""

import json, subprocess, sys, re, time, os, argparse, tempfile
from urllib import request as urllib_request
from urllib.error import URLError
from datetime import datetime

# ─── Конфигурация ─────────────────────────────────────────────────────────────
API_URL      = "http://127.0.0.1:1234/v1/chat/completions"
API_KEY      = "LM Studio API Key"
TEMPERATURE  = 0.1
MAX_TOKENS   = 700
TIMEOUT_API  = 90    # секунд
TIMEOUT_RUN  = 10    # секунд
INTERPRETER  = os.path.join(os.path.dirname(__file__), "lupus_proto.py")
TASKS_FILE   = os.path.join(os.path.dirname(__file__), "tasks.json")
RESULTS_FILE = os.path.join(os.path.dirname(__file__), "results.json")
CHEATSHEET   = os.path.join(os.path.dirname(__file__), "lupus_cheatsheet.txt")
RETRY_COUNT  = 2     # повторов при ошибке API

# ─── Загружаем шпаргалку ──────────────────────────────────────────────────────
def load_cheatsheet() -> str:
    if os.path.exists(CHEATSHEET):
        with open(CHEATSHEET, encoding="utf-8") as f:
            return f.read()
    return ""

# ─── Системные промпты ────────────────────────────────────────────────────────
def build_lupus_system() -> str:
    sheet = load_cheatsheet()
    return (
        "You are a programmer. Write ONLY Lupus code. "
        "No markdown, no explanation, no code blocks.\n\n"
        + sheet
    )

PYTHON_SYSTEM = (
    "You are a programmer. Write ONLY Python 3 code. "
    "No markdown, no explanation, no code blocks. "
    "Use print() for all output. Keep it simple."
)

# ─── API ──────────────────────────────────────────────────────────────────────
def call_model(system: str, prompt: str) -> str | None:
    payload = json.dumps({
        "model": "qwen2.5-coder-14b-instruct-abliterated",  # API Model Identifier из LM Studio
        "messages": [
            {"role": "system",  "content": system},
            {"role": "user",    "content": prompt},
        ],
        "temperature": TEMPERATURE,
        "max_tokens":  MAX_TOKENS,
        "stream":      False,
    }).encode("utf-8")

    for attempt in range(RETRY_COUNT):
        try:
            req = urllib_request.Request(
                API_URL, data=payload,
                headers={"Content-Type": "application/json", "Authorization": f"Bearer {API_KEY}"},
                method="POST",
            )
            with urllib_request.urlopen(req, timeout=TIMEOUT_API) as resp:
                data = json.loads(resp.read().decode("utf-8"))
                return data["choices"][0]["message"]["content"]
        except URLError:
            if attempt == RETRY_COUNT - 1:
                return None
            time.sleep(2)
        except Exception:
            return None
    return None

# ─── Утилиты ──────────────────────────────────────────────────────────────────
def strip_thinking(text: str) -> str:
    """Убираем <think>...</think> блоки Qwen3."""
    return re.sub(r"<think>.*?</think>", "", text, flags=re.DOTALL).strip()

def extract_code(text: str) -> str:
    text = strip_thinking(text)
    m = re.search(r"```(?:lupus|python|lisp|scheme|py)?\s*\n?(.*?)```", text, re.DOTALL)
    if m:
        return m.group(1).strip()
    text = re.sub(r"`([^`\n]+)`", r"\1", text)
    # Убираем строки с объяснениями (начинаются с #, //, или This/Here/The)
    lines = [l for l in text.splitlines()
             if not re.match(r"^\s*(This|Here|The|Note|Output|Result)\b", l)]
    return "\n".join(lines).strip()

def run_lupus(code: str) -> tuple[str, str]:
    """Возвращает (status, output): status = 'ok'|'error'|'timeout'"""
    tmp = os.path.join(tempfile.gettempdir(), "_lupus_exp.lupus")
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(code)
    try:
        r = subprocess.run(
            [sys.executable, INTERPRETER, tmp],
            capture_output=True, text=True, timeout=TIMEOUT_RUN
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
            capture_output=True, text=True, timeout=TIMEOUT_RUN
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

# ─── Цвета ────────────────────────────────────────────────────────────────────
G = lambda t: f"\033[32m{t}\033[0m"
R = lambda t: f"\033[31m{t}\033[0m"
Y = lambda t: f"\033[33m{t}\033[0m"
B = lambda t: f"\033[1m{t}\033[0m"
D = lambda t: f"\033[2m{t}\033[0m"

# ─── Сохранение / загрузка ────────────────────────────────────────────────────
def load_results() -> dict:
    if os.path.exists(RESULTS_FILE):
        with open(RESULTS_FILE, encoding="utf-8") as f:
            return json.load(f)
    return {}

def save_results(results: dict):
    with open(RESULTS_FILE, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2)

# ─── Один прогон задачи ───────────────────────────────────────────────────────
def run_task(task: dict, lupus_system: str) -> dict:
    row = {
        "id": task["id"], "cat": task["cat"], "name": task["name"],
        "expected": task["expected"], "ts": datetime.now().isoformat(),
    }

    for lang, system, runner in [
        ("lupus",  lupus_system, run_lupus),
        ("python", PYTHON_SYSTEM, run_python),
    ]:
        t0 = time.time()
        raw = call_model(system, task["prompt"])
        if raw is None:
            row[lang] = {"status": "api_error", "code": "", "output": "", "ms": 0}
            continue

        code = extract_code(raw)
        gen_ms = int((time.time() - t0) * 1000)

        t1 = time.time()
        run_status, output = runner(code)
        run_ms = int((time.time() - t1) * 1000)

        correct = check(output, task["expected"])
        status = "pass" if correct else ("wrong" if run_status == "ok" else run_status)

        row[lang] = {
            "status": status,
            "code":   code,
            "output": output,
            "ms":     gen_ms + run_ms,
        }

    return row

# ─── Отчёт ────────────────────────────────────────────────────────────────────
def print_report(results: dict):
    rows = list(results.values())
    if not rows:
        print("Нет данных.")
        return

    total = len(rows)
    print()
    print(B("=" * 62))
    print(B("  ОТЧЁТ ПО ЭКСПЕРИМЕНТУ"))
    print(B("=" * 62))

    # Общий итог
    print()
    print(B("  Общий результат:"))
    for lang in ["lupus", "python"]:
        passed = sum(1 for r in rows if r.get(lang, {}).get("status") == "pass")
        wrong  = sum(1 for r in rows if r.get(lang, {}).get("status") == "wrong")
        errors = sum(1 for r in rows if r.get(lang, {}).get("status") in ("error","timeout"))
        api_e  = sum(1 for r in rows if r.get(lang, {}).get("status") == "api_error")
        pct    = passed / total * 100
        bar    = G("█") * passed + D("░") * (total - passed)
        label  = "Lupus " if lang == "lupus" else "Python"
        print(f"  {B(label):8s}  {bar}  {G(f'{passed}/{total}')} ({pct:.1f}%)")
        if wrong:  print(f"           Неверный вывод:  {Y(str(wrong))}")
        if errors: print(f"           Ошибки запуска:  {R(str(errors))}")
        if api_e:  print(f"           Ошибки API:      {R(str(api_e))}")

    # По категориям
    print()
    print(B("  По категориям:"))
    cats = sorted(set(r["cat"] for r in rows))
    print(f"  {'Категория':14s}  {'Lupus':^12s}  {'Python':^12s}")
    print(f"  {'-'*14}  {'-'*12}  {'-'*12}")
    for cat in cats:
        cat_rows = [r for r in rows if r["cat"] == cat]
        n = len(cat_rows)
        lp = sum(1 for r in cat_rows if r.get("lupus",{}).get("status") == "pass")
        pp = sum(1 for r in cat_rows if r.get("python",{}).get("status") == "pass")
        lc = G(f"{lp}/{n}") if lp == n else (Y(f"{lp}/{n}") if lp > 0 else R(f"{lp}/{n}"))
        pc = G(f"{pp}/{n}") if pp == n else (Y(f"{pp}/{n}") if pp > 0 else R(f"{pp}/{n}"))
        print(f"  {cat:14s}  {lc:^21s}  {pc:^21s}")

    # Гипотеза
    l_pass = sum(1 for r in rows if r.get("lupus",{}).get("status") == "pass")
    p_pass = sum(1 for r in rows if r.get("python",{}).get("status") == "pass")
    print()
    print(B("  Гипотеза: Lupus лучше Python на ≥20%?"))
    if p_pass == 0:
        print(f"  {Y('Нет данных Python для сравнения')}")
    else:
        diff_pct = (l_pass - p_pass) / total * 100
        print(f"  Lupus: {l_pass}/{total}  Python: {p_pass}/{total}  Разница: {diff_pct:+.1f}%")
        if l_pass >= p_pass and diff_pct >= 20:
            print(f"  {G('✅ ГИПОТЕЗА ПОДТВЕРЖДЕНА')}")
        elif l_pass >= p_pass:
            print(f"  {Y('⚠️  Lupus лучше, но разница < 20% — нужно больше задач')}")
        elif diff_pct >= -5:
            print(f"  {Y('⚠️  Примерно одинаково')}")
        else:
            print(f"  {R('❌ ГИПОТЕЗА НЕ ПОДТВЕРЖДЕНА — Python лучше')}")

    # Провалы Lupus
    lupus_fails = [r for r in rows if r.get("lupus",{}).get("status") != "pass"]
    if lupus_fails:
        print()
        print(B(f"  Провалы Lupus ({len(lupus_fails)}):"))
        for r in lupus_fails[:10]:
            info = r.get("lupus", {})
            st = {"wrong":"⚠️ ","error":"❌ ","timeout":"⏱️ ","api_error":"🔌"}.get(info.get("status",""), "? ")
            print(f"  {st} #{r['id']:3d} [{r['cat']:10s}] {r['name']}")
            print(f"       Ожидалось: '{r['expected']}'")
            print(f"       Получено:  '{info.get('output','')[:60]}'")
        if len(lupus_fails) > 10:
            print(f"  ... и ещё {len(lupus_fails)-10}")

    print()
    print(B("=" * 62))
    print(f"  Результаты сохранены: {RESULTS_FILE}")
    print()

# ─── Основной прогон ──────────────────────────────────────────────────────────
def run_experiment(task_ids: list[int] | None, resume: bool, report_only: bool):
    # Загружаем задачи
    with open(TASKS_FILE, encoding="utf-8") as f:
        all_tasks = json.load(f)

    results = load_results()

    if report_only:
        print_report(results)
        return

    # Фильтрация
    tasks = all_tasks
    if task_ids:
        tasks = [t for t in tasks if t["id"] in task_ids]
    if resume:
        tasks = [t for t in tasks if str(t["id"]) not in results]

    if not tasks:
        print("Все задачи уже выполнены. Используйте --report для отчёта.")
        print_report(results)
        return

    # Проверяем соединение
    print(f"\n🔌 Проверяем LM Studio...", end=" ", flush=True)
    test = call_model("Reply: ok", "ok")
    if test is None:
        print(R("ОШИБКА"))
        print("\n  Нужно:\n  1. Открыть LM Studio\n  2. Загрузить модель\n  3. Local Server → Start Server")
        sys.exit(1)
    print(G("OK"))

    lupus_system = build_lupus_system()
    total = len(tasks)

    print(f"\n{B('='*62)}")
    print(f"{B('  LUPUS EXPERIMENT — BATCH RUNNER')}")
    print(f"  Задач к выполнению: {total}")
    print(f"  Режим: {'продолжение' if resume else 'полный прогон'}")
    print(f"{B('='*62)}\n")

    for i, task in enumerate(tasks, 1):
        progress = f"[{i:3d}/{total}]"
        tid = task["id"]; tcat = task["cat"]; tname = task["name"]
        print(f"{D(progress)} {B(f'#{tid:3d}')} {tcat:10s} — {tname}")

        row = run_task(task, lupus_system)
        results[str(task["id"])] = row
        save_results(results)  # сохраняем после каждой задачи

        for lang in ["lupus", "python"]:
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
            label = "Lupus " if lang == "lupus" else "Python"
            print(f"         {label}: {sym} {D(f'({ms}ms)')}")

        print()

    print_report(results)

# ─── CLI ──────────────────────────────────────────────────────────────────────
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Lupus Experiment Batch Runner")
    parser.add_argument("--resume",  action="store_true", help="Пропустить уже выполненные задачи")
    parser.add_argument("--report",  action="store_true", help="Только вывести отчёт")
    parser.add_argument("--ids",     type=str, default="",  help="Конкретные ID через запятую: 1,5,10")
    parser.add_argument("--cat",     type=str, default="",  help="Только категория: arithmetic|logic|loops|recursion|lists|strings|combined")
    args = parser.parse_args()

    ids = [int(x.strip()) for x in args.ids.split(",") if x.strip()] if args.ids else None

    if args.cat:
        with open(TASKS_FILE, encoding="utf-8") as f:
            all_t = json.load(f)
        cat_ids = [t["id"] for t in all_t if t["cat"] == args.cat]
        ids = cat_ids if not ids else [x for x in ids if x in cat_ids]

    run_experiment(task_ids=ids, resume=args.resume, report_only=args.report)
