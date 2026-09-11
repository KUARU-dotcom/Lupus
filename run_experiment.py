#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Lupus Experiment — Batch Runner
Запуск: python run_experiment.py --model merged-lupus-2b
        python run_experiment.py --model merged-python-2b --resume
        python run_experiment.py --list-models
        python run_experiment.py --model merged-lupus-2b --ids 1,5,10
        python run_experiment.py --model merged-lupus-2b --report
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

# ─── Конфигурация моделей ─────────────────────────────────────────────────────
MODELS_TO_TEST = [
    {
        "name": "Qwen3.8 4B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "Merged Lupus 2B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "Merged Lupus 2Bv2",
        "model_id": "merged-lupus-2bv2",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "Qwen3.5 2B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "Merged Python 2B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "python",
    },
    {
        "name": "Bonsai 27B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "Qwen3.5 9B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "Qwen3.5 4B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "MiniCPM5 2B",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "qwen3.5-2b",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "qwen3.5-4b",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "qwen3.5-9b",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "qwen3.8-4b-distill",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "bonsai-27b",
        "api_url": "http://localhost:1234/v1/chat/completions",
        "api_key": "lm-studio",
        "language": "lupus",
    },
    {
        "name": "deepseek-v4-flash",
        "api_url": "https://knyazevai.work/v1/chat/completions",
        "api_key": "",
        "language": "lupus",
    },
    {
        "name": "kimi-2.6",
        "api_url": "https://knyazevai.work/v1/chat/completions",
        "api_key": "",
        "language": "lupus",
    },
    {
        "name": "MiniMaxAI/MiniMax-M2.7",
        "api_url": "https://knyazevai.work/v1/chat/completions",
        "api_key": "",
        "language": "lupus",
    },
]

RUNTIME_MODELS: dict[str, dict] = {}

SPECIAL_MODEL_ALIASES = {
    "qwen3.8-4b-distill": "Qwen3.8 4B",
    "qwen3-8-4b-distill": "Qwen3.8 4B",
    "qwen3.8-4b": "Qwen3.8 4B",
    "qwen3-8-4b": "Qwen3.8 4B",
    "merged-lupus-2b": "Merged Lupus 2B",
    "merged-lupus-2bv2": "Merged Lupus 2Bv2",
    "merged-python-2b": "Merged Python 2B",
    "qwen3.5-2b": "Qwen3.5 2B",
    "qwen3-5-2b": "Qwen3.5 2B",
    "qwen3.5-9b": "Qwen3.5 9B",
    "qwen3-5-9b": "Qwen3.5 9B",
    "qwen3.5-4b": "Qwen3.5 4B",
    "qwen3-5-4b": "Qwen3.5 4B",
    "minicpm5-2b": "MiniCPM5 2B",
    "bonsai-27b": "Bonsai 27B",
}


def normalize_model_alias(name: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")


def get_model_aliases() -> dict[str, str]:
    aliases = dict(SPECIAL_MODEL_ALIASES)
    for model_cfg in MODELS_TO_TEST:
        model_name = model_cfg["name"]
        aliases[normalize_model_alias(model_name)] = model_name
        aliases[normalize_model_alias(model_name.replace(" ", ""))] = model_name
    return aliases


def build_model_registry() -> dict[str, dict]:
    registry = {}
    for model_cfg in MODELS_TO_TEST:
        registry[model_cfg["name"]] = model_cfg
        registry.setdefault(normalize_model_alias(model_cfg["name"]), model_cfg)
    for runtime_name, runtime_cfg in RUNTIME_MODELS.items():
        registry[runtime_name] = runtime_cfg
        registry.setdefault(normalize_model_alias(runtime_name), runtime_cfg)
    return registry


def resolve_model_name(requested_name: str | None) -> str | None:
    if not requested_name:
        return None
    requested_name = requested_name.strip()
    registry = build_model_registry()
    if requested_name in registry:
        return registry[requested_name]["name"]
    alias_map = get_model_aliases()
    direct_alias = alias_map.get(normalize_model_alias(requested_name))
    if direct_alias:
        return direct_alias
    for name, cfg in registry.items():
        if name.lower() == requested_name.lower():
            return cfg["name"]
    return None


def append_runtime_model(model_cfg: dict) -> dict:
    model_name = model_cfg["name"].strip()
    if not model_name:
        raise ValueError("Имя модели не может быть пустым")
    RUNTIME_MODELS[model_name] = {
        "name": model_name,
        "api_url": model_cfg.get("api_url") or "http://localhost:1234/v1/chat/completions",
        "api_key": model_cfg.get("api_key") or "lm-studio",
        "language": model_cfg.get("language") or "lupus",
    }
    return RUNTIME_MODELS[model_name]


def ask_for_value(prompt_text: str, default: str = "") -> str:
    answer = input(f"{prompt_text}{f' [{default}]' if default else ''}: ").strip()
    return answer if answer else default


def add_runtime_model() -> dict:
    print("\nДобавление модели на время теста")
    name = ask_for_value("Название модели", "custom-model")
    model_id = ask_for_value("Модель ID / имя в API", name)
    api_url = ask_for_value("URL API", "http://localhost:1234/v1/chat/completions")
    api_key = ask_for_value("API key (оставьте пустым для lm-studio / env KNYAZEV_KEY)", "")
    language = ask_for_value("Язык по умолчанию [lupus/python/both]", "lupus").lower()
    if language not in {"lupus", "python", "both"}:
        language = "lupus"
    model_cfg = {
        "name": name,
        "model_id": model_id,
        "api_url": api_url,
        "api_key": api_key,
        "language": language,
    }
    return append_runtime_model(model_cfg)


# ─── Постоянная конфигурация ──────────────────────────────────────────────────
TEMPERATURE = 0.1
MAX_TOKENS = 1024
TIMEOUT_API = 90
TIMEOUT_RUN = 10
RETRY_COUNT = 2

BASE_DIR = os.path.dirname(__file__)
LUPUS_BIN = os.path.join(BASE_DIR, "target", "release", "lupus")
if not os.path.exists(LUPUS_BIN):
    LUPUS_BIN = os.path.join(BASE_DIR, "target", "debug", "lupus")

TASKS_FILE = os.path.join(BASE_DIR, "tasks.json")

# ─── Системные промпты ────────────────────────────────────────────────────────
LUPUS_SYSTEM = (
    "You are a programmer. Write ONLY Lupus code. "
    "No markdown, no explanation, no code blocks."
)

PYTHON_SYSTEM = (
    "You are an expert Python programmer. Write ONLY valid Python 3 code. "
    "No markdown, no explanation, no code blocks. Use print() for output."
)

CHEATSHEET_PATH = os.path.join(BASE_DIR, "lupus_cheatsheet.txt")


def load_cheatsheet() -> str:
    if os.path.exists(CHEATSHEET_PATH):
        try:
            with open(CHEATSHEET_PATH, encoding="utf-8") as file:
                return file.read()
        except Exception:
            return ""
    return ""


def build_lupus_system(use_cheatsheet: bool = True) -> str:
    system = LUPUS_SYSTEM
    if use_cheatsheet:
        sheet = load_cheatsheet()
        if sheet:
            system = f"{system}\n\n{sheet}"
    return system

# ─── API ──────────────────────────────────────────────────────────────────────
def call_model(model_cfg: dict, system: str, prompt: str) -> str | None:
    api_key = (model_cfg.get("api_key") or os.environ.get("KNYAZEV_KEY") or "lm-studio").strip()
    if not api_key:
        api_key = "lm-studio"

    payload = {
        "model": model_cfg.get("model_id") or model_cfg["name"],
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt},
        ],
        "temperature": TEMPERATURE,
        "max_tokens": MAX_TOKENS,
        "stream": False,
        "chat_template_kwargs": {"enable_thinking": False},
    }

    for attempt in range(RETRY_COUNT):
        try:
            req = urllib_request.Request(
                model_cfg["api_url"],
                data=json.dumps(payload).encode("utf-8"),
                headers={
                    "Content-Type": "application/json",
                    "Authorization": f"Bearer {api_key}",
                },
                method="POST",
            )
            with urllib_request.urlopen(req, timeout=TIMEOUT_API) as response:
                data = json.loads(response.read().decode("utf-8"))
                return data["choices"][0]["message"]["content"]
        except (KeyboardInterrupt, SystemExit):
            raise
        except URLError:
            if attempt == RETRY_COUNT - 1:
                return None
            time.sleep(2)
        except Exception:
            return None
    return None

# ─── Утилиты ──────────────────────────────────────────────────────────────────
def strip_thinking(text: str) -> str:
    """Убираем блоки <think>...</think> из ответа модели."""
    return re.sub(r"<think>.*?</think>", "", text, flags=re.DOTALL).strip()


def extract_code(text: str) -> str:
    text = strip_thinking(text)
    match = re.search(
        r"```(?:lupus|python|lisp|scheme|py)?\s*\n?(.*?)```",
        text,
        re.DOTALL,
    )
    if match:
        return match.group(1).strip()
    text = re.sub(r"`([^`\n]+)`", r"\1", text)
    lines = [
        line for line in text.splitlines()
        if not re.match(r"^\s*(This|Here|The|Note|Output|Result)\b", line)
    ]
    return "\n".join(lines).strip()


def run_lupus(code: str) -> tuple[str, str]:
    """Возвращает (status, output): status = ok, error или timeout."""
    tmp = os.path.join(tempfile.gettempdir(), "_lupus_exp.lupus")
    with open(tmp, "w", encoding="utf-8") as file:
        file.write(code)
    try:
        result = subprocess.run(
            [LUPUS_BIN, "--lenient-parens", tmp],
            capture_output=True,
            text=True,
            timeout=TIMEOUT_RUN,
        )
        output = result.stdout.strip()
        if result.returncode != 0 or "Ошибка" in output:
            return "error", output
        return "ok", output
    except subprocess.TimeoutExpired:
        return "timeout", "TIMEOUT"
    except Exception as error:
        return "error", str(error)


def run_python(code: str) -> tuple[str, str]:
    tmp = os.path.join(tempfile.gettempdir(), "_python_exp.py")
    with open(tmp, "w", encoding="utf-8") as file:
        file.write(code)
    try:
        result = subprocess.run(
            [sys.executable, tmp],
            capture_output=True,
            text=True,
            timeout=TIMEOUT_RUN,
        )
        output = result.stdout.strip()
        if result.returncode != 0:
            return "error", (result.stderr.strip() or output)[:120]
        return "ok", output
    except subprocess.TimeoutExpired:
        return "timeout", "TIMEOUT"
    except Exception as error:
        return "error", str(error)


def check(got: str, expected: str) -> bool:
    return got.strip() == expected.strip()


# ─── Цвета ────────────────────────────────────────────────────────────────────
G = lambda text: f"\033[32m{text}\033[0m"
R = lambda text: f"\033[31m{text}\033[0m"
Y = lambda text: f"\033[33m{text}\033[0m"
B = lambda text: f"\033[1m{text}\033[0m"
D = lambda text: f"\033[2m{text}\033[0m"


# ─── Сохранение / загрузка ────────────────────────────────────────────────────
def get_results_file(model_name: str) -> str:
    return os.path.join(BASE_DIR, f"results_{model_name}.json")


def load_results(results_file: str) -> dict:
    if os.path.exists(results_file):
        with open(results_file, encoding="utf-8") as file:
            return json.load(file)
    return {}


def save_results(results: dict, results_file: str):
    with open(results_file, "w", encoding="utf-8") as file:
        json.dump(results, file, ensure_ascii=False, indent=2)


# ─── Один прогон задачи ───────────────────────────────────────────────────────
def run_task(task: dict, model_cfg: dict, lang: str, use_cheatsheet: bool = True) -> dict:
    system = build_lupus_system(use_cheatsheet=use_cheatsheet) if lang == "lupus" else PYTHON_SYSTEM
    runner = run_lupus if lang == "lupus" else run_python
    row = {
        "id": task["id"],
        "cat": task["cat"],
        "name": task["name"],
        "expected": task["expected"],
        "ts": datetime.now().isoformat(),
    }

    started = time.time()
    raw = call_model(model_cfg, system, task["prompt"])
    if raw is None:
        row[lang] = {"status": "api_error", "code": "", "output": "", "ms": 0}
        return row

    code = extract_code(raw)
    generation_ms = int((time.time() - started) * 1000)
    started = time.time()
    run_status, output = runner(code)
    run_ms = int((time.time() - started) * 1000)

    correct = check(output, task["expected"])
    status = "pass" if correct else ("wrong" if run_status == "ok" else run_status)
    row[lang] = {
        "status": status,
        "code": code,
        "output": output,
        "ms": generation_ms + run_ms,
    }
    return row


def list_models():
    print("\nДоступные модели:\n")
    print(f"  {'Имя':25s}  {'Язык':8s}  {'API URL'}")
    print(f"  {'-' * 25}  {'-' * 8}  {'-' * 45}")
    for model_cfg in MODELS_TO_TEST:
        print(
            f"  {model_cfg['name']:25s}  {model_cfg['language']:8s}  "
            f"{model_cfg['api_url']}"
        )
    print()


def print_report(results: dict, lang: str):
    rows = list(results.values())
    if not rows:
        print("Нет данных.")
        return

    total = len(rows)
    passed = sum(1 for row in rows if row.get(lang, {}).get("status") == "pass")
    wrong = sum(1 for row in rows if row.get(lang, {}).get("status") == "wrong")
    errors = sum(
        1 for row in rows if row.get(lang, {}).get("status") in ("error", "timeout")
    )
    api_errors = sum(
        1 for row in rows if row.get(lang, {}).get("status") == "api_error"
    )
    label = "Lupus" if lang == "lupus" else "Python"
    print(f"\n  {B(label)}: {G(f'{passed}/{total}')} ({passed / total * 100:.1f}%)")
    if wrong:
        print(f"           Неверный вывод: {Y(str(wrong))}")
    if errors:
        print(f"           Ошибки запуска: {R(str(errors))}")
    if api_errors:
        print(f"           Ошибки API: {R(str(api_errors))}")

    print("\n  По категориям:")
    for category in sorted(set(row["cat"] for row in rows)):
        category_rows = [row for row in rows if row["cat"] == category]
        count = sum(
            1 for row in category_rows if row.get(lang, {}).get("status") == "pass"
        )
        total_category = len(category_rows)
        value = G(f"{count}/{total_category}") if count == total_category else R(f"{count}/{total_category}")
        print(f"  {category:14s}  {value}")


# ─── Основной прогон ──────────────────────────────────────────────────────────
def run_experiment(
    task_ids: list[int] | None,
    resume: bool,
    report_only: bool,
    selected_model: str | None = None,
    preferred_lang: str | None = None,
    use_cheatsheet: bool = True,
    api_key: str | None = None,
):
    with open(TASKS_FILE, encoding="utf-8") as file:
        all_tasks = json.load(file)

    registry = build_model_registry()
    selected_name = resolve_model_name(selected_model) if selected_model else None
    if selected_name:
        models = [registry[selected_name]]
    else:
        models = list(registry.values())

    if not models:
        print(R(f"Ошибка: модель '{selected_model}' не найдена."))
        list_models()
        return

    for model_cfg in models:
        model_name = model_cfg["name"]
        lang_choices = [preferred_lang] if preferred_lang else ["lupus", "python"]
        if preferred_lang == "both":
            lang_choices = ["lupus", "python"]
        lang_choices = [lang for lang in lang_choices if lang in {"lupus", "python"}]
        if not lang_choices:
            lang_choices = ["lupus"]

        if api_key is not None:
            model_cfg["api_key"] = api_key

        if model_cfg.get("api_url", "").startswith("https://knyazevai.work") and not model_cfg.get("api_key"):
            model_cfg["api_key"] = os.environ.get("KNYAZEV_KEY", "")

        for lang in lang_choices:
            results_file = get_results_file(model_name)
            results = load_results(results_file)
            model_lang = lang
            if lang == "python":
                model_lang = "python"
            else:
                model_lang = "lupus"

            try:
                print(f"\n{B('=' * 62)}")
                print(f"  МОДЕЛЬ: {B(model_name)} ({model_lang})")
                print(f"  Результаты: {results_file}")
                print(f"{B('=' * 62)}\n")

                tasks = all_tasks
                if task_ids:
                    tasks = [task for task in tasks if task["id"] in task_ids]
                if resume:
                    tasks = [task for task in tasks if str(task["id"]) not in results]

                if report_only:
                    print_report(results, model_lang)
                    continue
                if not tasks:
                    print("Все задачи уже выполнены. Используйте --report для отчёта.")
                    print_report(results, model_lang)
                    continue

                print(f"Проверяем соединение с {model_name}...", end=" ", flush=True)
                if call_model(model_cfg, "Reply: ok", "ok") is None:
                    print(R("ОШИБКА"))
                    print(f"Проверьте доступность API: {model_cfg['api_url']}")
                    continue
                print(G("OK"))

                total = len(tasks)
                for index, task in enumerate(tasks, 1):
                    print(f"[{index:3d}/{total}] #{task['id']:3d} {task['cat']:10s} — {task['name']}")
                    row = run_task(task, model_cfg, model_lang, use_cheatsheet=use_cheatsheet)
                    results[str(task["id"])] = row
                    save_results(results, results_file)

                    info = row.get(model_lang, {})
                    status = info.get("status", "?")
                    output = info.get("output", "")[:40]
                    elapsed = info.get("ms", 0)
                    if status == "pass":
                        result = G("✅ PASS")
                    elif status == "wrong":
                        result = Y(f"⚠️  WRONG → '{output}'")
                    elif status in ("error", "timeout"):
                        result = R(f"❌ {status.upper()} → '{output}'")
                    else:
                        result = R("🔌 API ERROR")
                    label = "Lupus" if model_lang == "lupus" else "Python"
                    print(f"         {label}: {result} {D(f'({elapsed}ms)')}\n")

                print_report(results, model_lang)
            except KeyboardInterrupt:
                print("\n\nПрервано пользователем. Промежуточные результаты сохранены.\n")
                return


def prompt_interactive_config() -> tuple[str | None, str | None, bool, str | None]:
    print("\nИнтерактивный выбор конфигурации")
    print("1) Выбрать из списка моделей")
    print("2) Добавить модель временно")
    print("3) Использовать модель по имени")
    choice = input("Выбор [1/2/3]: ").strip() or "1"

    if choice == "2":
        model_cfg = add_runtime_model()
        return model_cfg["name"], None, True, model_cfg.get("api_key")

    if choice == "3":
        selected = input("Введите имя модели: ").strip()
        if not selected:
            return None, None, True, None
        return selected, None, True, None

    registry = build_model_registry()
    names = sorted({cfg["name"] for cfg in registry.values()})
    for index, name in enumerate(names, 1):
        print(f"  {index}. {name}")
    selected_index = input(f"Выберите модель [1-{len(names)}]: ").strip()
    try:
        idx = int(selected_index) - 1
        if 0 <= idx < len(names):
            return names[idx], None, True, None
    except ValueError:
        pass
    return None, None, True, None


def prompt_language_choice() -> str:
    print("\nВыберите язык:")
    print("1) Lupus")
    print("2) Python")
    print("3) Оба сразу")
    choice = (input("Выбор [1/2/3]: ").strip() or "3").lower()
    if choice == "2":
        return "python"
    if choice == "3":
        return "both"
    return "lupus"


def prompt_cheatsheet_choice() -> bool:
    choice = (input("Использовать шпаргалку Lupus? [Y/n]: ").strip() or "y").lower()
    return choice not in {"n", "no", "false", "0"}


# ─── CLI ──────────────────────────────────────────────────────────────────────
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Lupus Experiment Batch Runner")
    parser.add_argument(
        "--model",
        type=str,
        default="",
        help="Имя конкретной модели (например qwen3.8-4b-distill)",
    )
    parser.add_argument("--lang", type=str, default="both", help="lupus | python | both")
    parser.add_argument("--no-cheatsheet", action="store_true", help="Не использовать шпаргалку Lupus")
    parser.add_argument("--api-key", type=str, default="", help="API key для внешнего API, если используется")
    parser.add_argument("--interactive", action="store_true", help="Интерактивный выбор параметров")

    model_aliases = get_model_aliases()
    for alias, model_name in model_aliases.items():
        parser.add_argument(
            f"--{alias}",
            dest="model",
            action="store_const",
            const=model_name,
            help=argparse.SUPPRESS,
        )

    parser.add_argument("--list-models", action="store_true", help="Вывести список моделей")
    parser.add_argument("--resume", action="store_true", help="Пропустить выполненные задачи")
    parser.add_argument("--report", action="store_true", help="Только вывести отчёт")
    parser.add_argument("--ids", type=str, default="", help="ID через запятую: 1,5,10")
    parser.add_argument("--cat", type=str, default="", help="Только категория задач")
    args = parser.parse_args()

    if args.list_models:
        list_models()
        sys.exit(0)

    interactive_mode = args.interactive or (not args.model and not args.list_models and sys.stdin.isatty())
    if interactive_mode:
        selected_model, _, _, runtime_api_key = prompt_interactive_config()
        language_choice = prompt_language_choice()
        use_cheatsheet = prompt_cheatsheet_choice()
        if selected_model is None:
            print("Модель не выбрана. Выход.")
            sys.exit(1)
        args.model = selected_model
        args.lang = language_choice
        args.no_cheatsheet = not use_cheatsheet
        if runtime_api_key:
            args.api_key = runtime_api_key

    selected_model = args.model or None
    if selected_model:
        resolved = resolve_model_name(selected_model)
        if resolved:
            selected_model = resolved
        else:
            selected_model = selected_model.strip()

    preferred_lang = args.lang or "both"
    preferred_lang = preferred_lang.lower()
    if preferred_lang not in {"lupus", "python", "both"}:
        preferred_lang = "both"

    use_cheatsheet = not args.no_cheatsheet
    ids = [int(value.strip()) for value in args.ids.split(",") if value.strip()] if args.ids else None
    if args.cat:
        with open(TASKS_FILE, encoding="utf-8") as file:
            all_tasks = json.load(file)
        category_ids = [task["id"] for task in all_tasks if task["cat"] == args.cat]
        ids = category_ids if ids is None else [value for value in ids if value in category_ids]

    try:
        run_experiment(
            task_ids=ids,
            resume=args.resume,
            report_only=args.report,
            selected_model=selected_model,
            preferred_lang=preferred_lang,
            use_cheatsheet=use_cheatsheet,
            api_key=args.api_key or None,
        )
    except KeyboardInterrupt:
        print("\nПрервано пользователем. Промежуточные результаты сохранены.\n")
        sys.exit(130)

