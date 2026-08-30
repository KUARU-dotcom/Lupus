#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Диагностика подключения к LM Studio. Запуск: python check_connection.py"""

import json
from urllib import request as urllib_request
from urllib.error import URLError

PORTS    = [1234, 8080, 8081, 11434]
API_KEY  = "api_key"
PATHS    = ["/v1/chat/completions", "/api/chat", "/v1/completions"]

def try_get(url):
    try:
        req_get = urllib_request.Request(url, headers={"Authorization": f"Bearer {API_KEY}"})
        with urllib_request.urlopen(req_get, timeout=3) as r:
            return r.status, r.read(200).decode()
    except URLError as e:
        return None, str(e)

def try_post(url, payload):
    data = json.dumps(payload).encode()
    req = urllib_request.Request(url, data=data,
          headers={"Content-Type": "application/json", "Authorization": f"Bearer {API_KEY}"}, method="POST")
    try:
        with urllib_request.urlopen(req, timeout=10) as r:
            return r.status, json.loads(r.read())
    except URLError as e:
        return None, str(e)
    except Exception as e:
        return None, str(e)

print("\n🔍 Ищем LM Studio сервер...\n")

found_url = None

for port in PORTS:
    base = f"http://127.0.0.1:{port}"
    # Сначала проверяем /v1/models — стандартный эндпоинт
    status, body = try_get(f"{base}/v1/models")
    if status:
        print(f"✅ Сервер найден: {base}")
        print(f"   /v1/models → {body[:200]}")
        found_url = f"{base}/v1/chat/completions"
        break
    else:
        print(f"❌ {base} — {str(body)[:60]}")

if not found_url:
    print("\n⚠️  Сервер не найден ни на одном порту.")
    print("\nЧто сделать в LM Studio:")
    print("  1. Вкладка 'Local Server' (или Developer / API)")
    print("  2. Нажать 'Start Server'")
    print("  3. Убедиться что в строке адреса написано: localhost:1234")
    print("  4. Запустить этот скрипт снова")
else:
    print(f"\n📡 Тестируем запрос к {found_url} ...\n")
    payload = {
        "model": "local-model",
        "messages": [{"role": "user", "content": "Say: hello"}],
        "max_tokens": 20,
        "temperature": 0.1,
    }
    status, resp = try_post(found_url, payload)
    if status:
        content = resp["choices"][0]["message"]["content"]
        print(f"✅ Ответ модели: '{content}'")
        print(f"\n✅ Всё работает! URL для скрипта: {found_url}")
        print(f"\nВ run_experiment.py строка 12 должна быть:")
        print(f'   API_URL = "{found_url}"')
    else:
        print(f"❌ Запрос не прошёл: {resp}")
        print("\nВозможные причины:")
        print("  - Модель ещё грузится (подожди)")
        print("  - Нужно выбрать модель в LM Studio")
