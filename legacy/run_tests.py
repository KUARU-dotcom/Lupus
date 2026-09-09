#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Простой тестовый скрипт для примеров Lupus Alpha v0.1
"""

import subprocess
import sys
import os

def запустить_пример(имя_файла: str) -> tuple:
    """Запустить пример и вернуть (успех, вывод, ошибка)."""
    try:
        результат = subprocess.run(
            [sys.executable, 'lupus_proto.py', имя_файла],
            capture_output=True,
            text=True,
            timeout=10
        )
        успех = результат.returncode == 0
        вывод = результат.stdout
        ошибка = результат.stderr
        return успех, вывод, ошибка
    except subprocess.TimeoutExpired:
        return False, "", "Превышено время выполнения"
    except Exception as e:
        return False, "", str(e)


def главная():
    """Запустить все тесты примеров."""
    примеры = [
        'examples/calc.lupus',
        'examples/list.lupus',
        'examples/fib.lupus',
    ]
    
    print("=" * 60)
    print("Lupus Alpha v0.1 - Тестирование примеров")
    print("=" * 60)
    print()
    
    всех_успешно = True
    
    for пример in примеры:
        print(f"📋 Запуск: {пример}")
        print("-" * 60)
        
        если_существует = os.path.exists(пример)
        if not если_существует:
            print(f"❌ ОШИБКА: Файл не найден: {пример}")
            print()
            всех_успешно = False
            continue
        
        успех, вывод, ошибка = запустить_пример(пример)
        
        if успех:
            print("✅ Успешно выполнено")
            if вывод:
                print("\nВывод:")
                print(вывод)
        else:
            print("❌ Ошибка при выполнении")
            всех_успешно = False
            if ошибка:
                print("\nСообщение об ошибке:")
                print(ошибка)
            if вывод:
                print("\nЧастичный вывод:")
                print(вывод)
        
        print()
    
    print("=" * 60)
    if всех_успешно:
        print("✅ Все примеры успешно выполнены!")
    else:
        print("❌ Некоторые примеры завершились с ошибкой")
    print("=" * 60)
    
    sys.exit(0 if всех_успешно else 1)


if __name__ == '__main__':
    главная()
