//! Интерпретатор языка Lupus (tree-walk).
//!
//! Обходит AST и вычисляет формы. Содержит логику специальных форм
//! (`define`, `define-mutable`, `set!`, `if`, `while`, `begin`, `and`, `or`,
//! `lambda`) и вызов встроенных / пользовательских функций.

use crate::ast::Expr;
use crate::builtins;
use crate::environment::Environment;
use crate::error::Error;
use crate::lexer::{Token, TokenKind};
use crate::value::{is_truthy, Closure, Value};
use std::cell::RefCell;
use std::rc::Rc;

/// Интерпретатор с глобальным окружением.
pub struct Interpreter {
    /// Глобальное окружение — точка отсчёта для поиска переменных.
    global: Rc<RefCell<Environment>>,
    /// Мягкий режим `--lenient-parens`: игнорировать лишние `)` вместо падения.
    lenient_parens: bool,
}

impl Interpreter {
    /// Создаёт новый интерпретатор с пустым глобальным окружением
    /// и выключенным lenient-режимом (по умолчанию).
    pub fn new() -> Self {
        Interpreter {
            global: Environment::new(),
            lenient_parens: false,
        }
    }

    /// Создаёт интерпретатор с явно заданным lenient-режимом.
    ///
    /// При `lenient = true` лишние закрывающие скобки `)` игнорируются
    /// (с предупреждением в stderr), интерпретатор не падает.
    pub fn with_lenient_parens(lenient: bool) -> Self {
        Interpreter {
            global: Environment::new(),
            lenient_parens: lenient,
        }
    }

    /// Включает/выключает lenient-режим после создания интерпретатора.
    pub fn set_lenient_parens(&mut self, lenient: bool) {
        self.lenient_parens = lenient;
    }

    /// Возвращает копию shared-ссылки на глобальное окружение.
    fn global_env(&self) -> Rc<RefCell<Environment>> {
        self.global.clone()
    }

    /// Вычисляет AST-программу (список форм верхнего уровня).
    pub fn run(&mut self, forms: Vec<Expr>) -> Result<(), Error> {
        for form in forms {
            self.eval(&form, self.global_env())?;
        }
        Ok(())
    }

    /// Лексирует, парсит и выполняет исходный код, возвращая значение
    /// последней формы. Состояние (глобальное окружение) сохраняется
    /// между вызовами.
    pub fn eval_source(&mut self, src: &str) -> Result<Value, Error> {
        let mut tokens = crate::lexer::lex(src)?;
        // В lenient-режиме лишние `)` убираются ещё до парсинга.
        if self.lenient_parens {
            tokens = sanitize_tokens_lenient(tokens);
        }
        let forms = crate::parser::parse(tokens)?;
        let mut last = Value::None;
        for form in &forms {
            last = self.eval(form, self.global_env())?;
        }
        Ok(last)
    }

    /// Оценка одного выражения в контексте окружения.
    pub fn eval(&self, expr: &Expr, env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        match expr {
            Expr::Number(n) => Ok(Value::Int(*n)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::None => Ok(Value::None),
            Expr::Symbol(name) => self.eval_symbol(&env, name),
            Expr::List(elems) => self.eval_list(elems, env),
        }
    }
}

impl Interpreter {
    /// Обработка списков (вызовов форм).
    fn eval_list(&self, elems: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if elems.is_empty() {
            return Ok(Value::None);
        }

        let first = &elems[0];
        let rest = &elems[1..];

        // Специальные формы, опознаваемые по имени-символу.
        if let Expr::Symbol(name) = first {
            match name.as_str() {
                "define" => return self.do_define(rest, &env),
                "define-mutable" => return self.do_define_mutable(rest, &env),
                "set!" => return self.do_set(rest, &env),
                "lambda" => return self.do_lambda(rest, env),
                "if" => return self.do_if(rest, env),
                "while" => return self.do_while(rest, env),
                "begin" => return self.do_begin(rest, env),
                "and" => return self.do_and(rest, env),
                "or" => return self.do_or(rest, env),
                "cond" => return self.do_cond(rest, env),
                "for" => return self.do_for(rest, env),
                "range" => return self.do_range(rest, env),
                "let" => return self.do_let(rest, &env),
                "match" => return self.do_match(rest, env),
                "if-let" => return self.do_if_let(rest, env),
                "assert" => return self.do_assert(rest, env),
                "not" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::not(&args);
                }
                "print" => return self.do_print(rest, env),
                "int->str" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::int_to_str(&args);
                }
                "string-append" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::string_append(&args);
                }
                "string-length" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::string_length(&args);
                }
                "string-split" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::string_split(&args);
                }
                "string-reverse" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::string_reverse(&args);
                }
                "str->int" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::str_to_int(&args);
                }
                "list" => {
                    let args = self.eval_all(rest, &env)?;
                    return Ok(builtins::make_list(&args));
                }
                // Конструкторы Option/Result: (some e), (success e), (failure e).
                "some" => {
                    if rest.len() != 1 {
                        return Err(Error::new(format!(
                            "(some ...) требует ровно 1 аргумент, получено {}",
                            rest.len()
                        )));
                    }
                    let v = self.eval(&rest[0], env)?;
                    return Ok(Value::Some(Box::new(v)));
                }
                "success" => {
                    if rest.len() != 1 {
                        return Err(Error::new(format!(
                            "(success ...) требует ровно 1 аргумент, получено {}",
                            rest.len()
                        )));
                    }
                    let v = self.eval(&rest[0], env)?;
                    return Ok(Value::Success(Box::new(v)));
                }
                "failure" => {
                    if rest.len() != 1 {
                        return Err(Error::new(format!(
                            "(failure ...) требует ровно 1 аргумент, получено {}",
                            rest.len()
                        )));
                    }
                    let v = self.eval(&rest[0], env)?;
                    return Ok(Value::Failure(Box::new(v)));
                }
                // ─── Функции списков (ЭТАП 2) ───────────────────────────────────
                "cons" => {
                    let args = self.eval_all(rest, &env)?;
                    if args.len() != 2 {
                        return Err(Error::new(
                            "(cons ...) требует ровно 2 аргумента".to_string(),
                        ));
                    }
                    let list = match &args[1] {
                        Value::List(items) => items.clone(),
                        _ => return Err(Error::new("Второй аргумент cons должен быть списком")),
                    };
                    let mut result = vec![args[0].clone()];
                    result.extend(list);
                    return Ok(Value::List(result));
                }
                "head" => {
                    let args = self.eval_all(rest, &env)?;
                    if args.len() != 1 {
                        return Err(Error::new(
                            "(head ...) требует ровно 1 аргумент".to_string(),
                        ));
                    }
                    match &args[0] {
                        Value::List(items) if !items.is_empty() => {
                            return Ok(Value::Some(Box::new(items[0].clone())));
                        }
                        _ => return Ok(Value::None),
                    }
                }
                "tail" => {
                    let args = self.eval_all(rest, &env)?;
                    if args.len() != 1 {
                        return Err(Error::new(
                            "(tail ...) требует ровно 1 аргумент".to_string(),
                        ));
                    }
                    match &args[0] {
                        Value::List(items) if items.len() > 1 => {
                            return Ok(Value::Some(Box::new(Value::List(items[1..].to_vec()))));
                        }
                        _ => return Ok(Value::None),
                    }
                }
                "empty?" => {
                    let args = self.eval_all(rest, &env)?;
                    if args.len() != 1 {
                        return Err(Error::new(
                            "(empty? ...) требует ровно 1 аргумент".to_string(),
                        ));
                    }
                    match &args[0] {
                        Value::List(items) => return Ok(Value::Bool(items.is_empty())),
                        _ => return Err(Error::new("empty? требует список")),
                    }
                }
                "append" => {
                    let args = self.eval_all(rest, &env)?;
                    if args.len() != 2 {
                        return Err(Error::new(
                            "(append ...) требует ровно 2 аргумента".to_string(),
                        ));
                    }
                    match (&args[0], &args[1]) {
                        (Value::List(a), Value::List(b)) => {
                            let mut result = a.clone();
                            result.extend(b.clone());
                            return Ok(Value::List(result));
                        }
                        _ => return Err(Error::new("append требует два списка")),
                    }
                }
                "list-map" => {
                    let args = self.eval_all(rest, &env)?;
                    if args.len() != 2 {
                        return Err(Error::new(
                            "(list-map ...) требует ровно 2 аргумента".to_string(),
                        ));
                    }
                    let list = match &args[0] {
                        Value::List(items) => items.clone(),
                        _ => {
                            return Err(Error::new("Первый аргумент list-map должен быть списком"))
                        }
                    };
                    let f = match &args[1] {
                        Value::Closure(c) => c,
                        _ => {
                            return Err(Error::new("Второй аргумент list-map должен быть функцией"))
                        }
                    };
                    let mut result = Vec::with_capacity(list.len());
                    for item in list {
                        let mapped = self.call_closure_with_values(f, vec![item.clone()], &env)?;
                        result.push(mapped);
                    }
                    return Ok(Value::List(result));
                }
                "list-filter" => {
                    let args = self.eval_all(rest, &env)?;
                    if args.len() != 2 {
                        return Err(Error::new(
                            "(list-filter ...) требует ровно 2 аргумента".to_string(),
                        ));
                    }
                    let list = match &args[0] {
                        Value::List(items) => items.clone(),
                        _ => {
                            return Err(Error::new(
                                "Первый аргумент list-filter должен быть списком",
                            ))
                        }
                    };
                    let f = match &args[1] {
                        Value::Closure(c) => c,
                        _ => {
                            return Err(Error::new(
                                "Второй аргумент list-filter должен быть функцией",
                            ))
                        }
                    };
                    let mut result = Vec::new();
                    for item in list {
                        let keep = self.call_closure_with_values(f, vec![item.clone()], &env)?;
                        if is_truthy(&keep) {
                            result.push(item);
                        }
                    }
                    return Ok(Value::List(result));
                }
                "list-fold" => {
                    let args = self.eval_all(rest, &env)?;
                    if args.len() != 3 {
                        return Err(Error::new(
                            "(list-fold ...) требует ровно 3 аргумента".to_string(),
                        ));
                    }
                    let list = match &args[0] {
                        Value::List(items) => items.clone(),
                        _ => {
                            return Err(Error::new("Первый аргумент list-fold должен быть списком"))
                        }
                    };
                    let f = match &args[2] {
                        Value::Closure(c) => c,
                        _ => {
                            return Err(Error::new(
                                "Третий аргумент list-fold должен быть функцией",
                            ))
                        }
                    };
                    let mut acc = args[1].clone();
                    for item in list {
                        acc = self.call_closure_with_values(f, vec![acc, item], &env)?;
                    }
                    return Ok(acc);
                }
                "nth" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::nth(&args);
                }
                // `length` и алиас `len` — одно и то же.
                "length" | "len" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::length(&args);
                }
                // Встроенная проверка наличия значения в списке (см. задачу #60).
                "contains?" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::contains(&args);
                }
                "+" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::add(&args);
                }
                "-" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::sub(&args);
                }
                "*" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::mul(&args);
                }
                "/" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::div(&args);
                }
                "%" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::rem(&args);
                }
                // Алиас `mod` для `%` (по прецеденту с алиасом `len` для `length`).
                "mod" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::rem(&args);
                }
                "pow" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::pow(&args);
                }
                "min" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::min(&args);
                }
                "max" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::max(&args);
                }
                "abs" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::abs(&args);
                }
                "=" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::eq(&args);
                }
                "!=" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::neq(&args);
                }
                "<" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::lt(&args);
                }
                ">" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::gt(&args);
                }
                "<=" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::le(&args);
                }
                ">=" => {
                    let args = self.eval_all(rest, &env)?;
                    return builtins::ge(&args);
                }
                "test" => return Ok(Value::None),
                _ => {}
            }
        }

        // Пользовательская функция: первый элемент вычисляется как значение.
        self.eval_call(elems, env)
    }

    /// Вычисляет все формы и возвращает список значений.
    fn eval_all(
        &self,
        forms: &[Expr],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Vec<Value>, Error> {
        forms.iter().map(|f| self.eval(f, env.clone())).collect()
    }

    /// Вызов пользовательской функции.
    fn eval_call(&self, elems: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        let first = &elems[0];
        let actual_args = &elems[1..];

        let func_val = self.eval(first, env.clone())?;

        match func_val {
            Value::Closure(closure) => self.call_closure(&closure, actual_args, &env),
            _ => Err(Error::new("Не могу вызвать не-функцию")),
        }
    }

    /// Вызывает замыкание с переданными формами-аргументами.
    fn call_closure(
        &self,
        closure: &Closure,
        actual_args: &[Expr],
        caller_env: &Rc<RefCell<Environment>>,
    ) -> Result<Value, Error> {
        if actual_args.len() != closure.params.len() {
            return Err(Error::new(format!(
                "Функция ожидает {} аргументов, получено {}",
                closure.params.len(),
                actual_args.len()
            )));
        }

        // Аргументы вычисляются в окружении вызывающего кода.
        let arg_vals = self.eval_all(actual_args, caller_env)?;

        // Новое окружение вызова с родителем — окружением замыкания.
        let func_env = Environment::new_child(closure.env.clone());
        for (param, val) in closure.params.iter().zip(arg_vals.iter()) {
            Environment::define(&func_env, param, val.clone(), false);
        }

        self.eval(&closure.body, func_env)
    }

    /// Вызывает замыкание с уже вычисленными значениями аргументов.
    ///
    /// Используется [`list_map`], [`list_filter`], [`list_fold`]:
    /// аргументы для `f` уже определены (элементы списка), и их не нужно
    /// повторно вычислять из AST-форм.
    fn call_closure_with_values(
        &self,
        closure: &Closure,
        arg_vals: Vec<Value>,
        _caller_env: &Rc<RefCell<Environment>>,
    ) -> Result<Value, Error> {
        if arg_vals.len() != closure.params.len() {
            return Err(Error::new(format!(
                "Функция ожидает {} аргументов, получено {}",
                closure.params.len(),
                arg_vals.len()
            )));
        }

        let func_env = Environment::new_child(closure.env.clone());
        for (param, val) in closure.params.iter().zip(arg_vals.iter()) {
            Environment::define(&func_env, param, val.clone(), false);
        }

        self.eval(&closure.body, func_env)
    }

    /// (define имя значение) или (define (имя параметры...) тело...).
    fn do_define(&self, rest: &[Expr], env: &Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() < 2 {
            return Err(Error::new(format!(
                "(define ...) требует минимум 2 аргумента, получено {}",
                rest.len()
            )));
        }

        match &rest[0] {
            // (define имя значение)
            Expr::Symbol(name) => {
                let value = self.eval(&rest[1], env.clone())?;
                Environment::define(env, name, value, false);
                Ok(Value::None)
            }
            // (define (имя параметры...) тело...)
            Expr::List(header) => {
                if header.is_empty() {
                    return Err(Error::new("Пустой список в определении функции"));
                }
                let fn_name = match &header[0] {
                    Expr::Symbol(n) => n.clone(),
                    _ => return Err(Error::new("Имя функции должно быть символом")),
                };
                let mut param_names = Vec::new();
                for p in &header[1..] {
                    match p {
                        Expr::Symbol(s) => param_names.push(s.clone()),
                        _ => return Err(Error::new("Параметры должны быть символами")),
                    }
                }

                let body_expr = if rest.len() == 2 {
                    rest[1].clone()
                } else {
                    Expr::List(
                        std::iter::once(Expr::Symbol("begin".to_string()))
                            .chain(rest[1..].iter().cloned())
                            .collect(),
                    )
                };

                let closure = Closure {
                    params: param_names,
                    body: body_expr,
                    env: env.clone(),
                };
                Environment::define(env, &fn_name, Value::Closure(closure), false);
                Ok(Value::None)
            }
            _ => Err(Error::new("Некорректное определение")),
        }
    }

    /// (define-mutable имя значение).
    fn do_define_mutable(
        &self,
        rest: &[Expr],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Value, Error> {
        if rest.len() != 2 {
            return Err(Error::new(format!(
                "(define-mutable ...) требует ровно 2 аргумента, получено {}",
                rest.len()
            )));
        }
        let name = match &rest[0] {
            Expr::Symbol(n) => n,
            _ => {
                return Err(Error::new(
                    "Первый аргумент define-mutable должен быть символом",
                ))
            }
        };
        let value = self.eval(&rest[1], env.clone())?;
        Environment::define(env, name, value, true);
        Ok(Value::None)
    }

    /// (set! имя значение).
    fn do_set(&self, rest: &[Expr], env: &Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 2 {
            return Err(Error::new(format!(
                "(set! ...) требует ровно 2 аргумента, получено {}",
                rest.len()
            )));
        }
        let name = match &rest[0] {
            Expr::Symbol(n) => n,
            _ => return Err(Error::new("Первый аргумент set! должен быть символом")),
        };
        let new_value = self.eval(&rest[1], env.clone())?;
        match Environment::set(env, name, new_value) {
            Ok(()) => Ok(Value::None),
            Err(e) if e.message.starts_with("Переменная не определена") => {
                // Улучшенное сообщение с подсказками по похожим именам.
                Err(self.undefined_variable_error(env, name))
            }
            Err(e) => Err(e),
        }
    }

    /// (lambda (параметры...) тело...).
    fn do_lambda(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() < 2 {
            return Err(Error::new(format!(
                "(lambda ...) требует минимум 2 аргумента, получено {}",
                rest.len()
            )));
        }
        let param_list = match &rest[0] {
            Expr::List(items) => items,
            _ => {
                return Err(Error::new(
                    "Первый аргумент lambda должен быть списком параметров",
                ))
            }
        };

        let mut param_names = Vec::new();
        for p in param_list {
            match p {
                Expr::Symbol(s) => param_names.push(s.clone()),
                _ => return Err(Error::new("Параметры lambda должны быть символами")),
            }
        }

        let body_expr = if rest.len() == 2 {
            rest[1].clone()
        } else {
            Expr::List(
                std::iter::once(Expr::Symbol("begin".to_string()))
                    .chain(rest[1..].iter().cloned())
                    .collect(),
            )
        };

        Ok(Value::Closure(Closure {
            params: param_names,
            body: body_expr,
            env,
        }))
    }

    /// (if условие then else).
    fn do_if(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 3 && rest.len() != 2 {
            return Err(Error::new(format!(
                "(if ...) требует ровно 2 или 3 аргумента, получено {}",
                rest.len()
            )));
        }

        let cond_val = self.eval(&rest[0], env.clone())?;

        if is_truthy(&cond_val) {
            self.eval(&rest[1], env)
        } else if rest.len() == 3 {
            self.eval(&rest[2], env)
        } else {
            Ok(Value::None)
        }
    }

    /// (while условие тело...).
    fn do_while(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() < 2 {
            return Err(Error::new(format!(
                "(while ...) требует минимум 2 аргумента, получено {}",
                rest.len()
            )));
        }

        let cond_form = &rest[0];
        let body_forms = &rest[1..];

        let mut result = Value::None;

        loop {
            let cond_val = self.eval(cond_form, env.clone())?;
            if !is_truthy(&cond_val) {
                break;
            }
            for form in body_forms {
                result = self.eval(form, env.clone())?;
            }
        }

        Ok(result)
    }

    /// (begin форма...).
    fn do_begin(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        let mut result = Value::None;
        for form in rest {
            result = self.eval(form, env.clone())?;
        }
        Ok(result)
    }

    /// (and форма...) — короткое замыкание, возвращает bool.
    fn do_and(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        for form in rest {
            let result = self.eval(form, env.clone())?;
            if !is_truthy(&result) {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    }

    /// (or форма...) — короткое замыкание, возвращает первое истинное значение.
    fn do_or(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        for form in rest {
            let result = self.eval(form, env.clone())?;
            if is_truthy(&result) {
                return Ok(result);
            }
        }
        Ok(Value::Bool(false))
    }

    /// (print строка).
    fn do_print(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 1 {
            return Err(Error::new(format!(
                "(print ...) требует ровно 1 аргумент, получено {}",
                rest.len()
            )));
        }
        let value = self.eval(&rest[0], env)?;
        builtins::print_arg(&value)?;
        Ok(Value::None)
    }

    /// (cond (условие тело...) ... (else тело...))
    ///
    /// Ветки проверяются по порядку. Тело может содержать несколько форм
    /// (неявный `begin`). Если ни одна ветка не совпала и `else` отсутствует —
    /// возвращается `None`.
    fn do_cond(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        for clause in rest {
            let Expr::List(items) = clause else {
                return Err(Error::new("Каждая ветка cond должна быть списком"));
            };
            if items.is_empty() {
                return Err(Error::new("Пустая ветка cond"));
            }
            let header = &items[0];
            let body = &items[1..];

            // Ветка (else тело...) — безусловная.
            if let Expr::Symbol(name) = header {
                if name == "else" {
                    if body.is_empty() {
                        return Ok(Value::None);
                    }
                    let mut result = Value::None;
                    for form in body {
                        result = self.eval(form, env.clone())?;
                    }
                    return Ok(result);
                }
            }

            let cond_val = self.eval(header, env.clone())?;
            if is_truthy(&cond_val) {
                if body.is_empty() {
                    return Ok(Value::None);
                }
                let mut result = Value::None;
                for form in body {
                    result = self.eval(form, env.clone())?;
                }
                return Ok(result);
            }
        }
        Ok(Value::None)
    }

    /// (for переменная in iter-выражение тело...)
    ///
    /// Выражение обязано вычислиться в список, иначе — ошибка. Переменная
    /// связывается заново на каждой итерации. Возвращает `None`.
    fn do_for(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        // Ожидаем: [var, "in", iter, тело...]
        if rest.len() < 3 {
            return Err(Error::new(
                "(for ...) требует форму: (for var in iter тело...)".to_string(),
            ));
        }
        let var = match &rest[0] {
            Expr::Symbol(s) => s.clone(),
            _ => return Err(Error::new("Переменная цикла for должна быть символом")),
        };
        if !matches!(&rest[1], Expr::Symbol(s) if s == "in") {
            return Err(Error::new("for: ожидалось ключевое слово 'in'"));
        }
        let iter_expr = &rest[2];
        let body_forms = &rest[3..];

        let iter_val = self.eval(iter_expr, env.clone())?;
        let items = match iter_val {
            Value::List(items) => items,
            _ => {
                return Err(Error::new(
                    "for: выражение после 'in' должно вычисляться в список",
                ))
            }
        };

        for item in items {
            let iter_env = Environment::new_child(env.clone());
            Environment::define(&iter_env, &var, item, false);
            for form in body_forms {
                self.eval(form, iter_env.clone())?;
            }
        }
        Ok(Value::None)
    }

    /// (range start end) → список целых чисел [start, end).
    ///
    /// Если `start >= end` — пустой список.
    fn do_range(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 2 {
            return Err(Error::new(format!(
                "(range ...) требует ровно 2 аргумента, получено {}",
                rest.len()
            )));
        }
        let start = match self.eval(&rest[0], env.clone())? {
            Value::Int(v) => v,
            _ => return Err(Error::new("Первый аргумент range должен быть целым числом")),
        };
        let end = match self.eval(&rest[1], env.clone())? {
            Value::Int(v) => v,
            _ => return Err(Error::new("Второй аргумент range должен быть целым числом")),
        };

        let mut items = Vec::new();
        let mut i = start;
        while i < end {
            items.push(Value::Int(i));
            i += 1;
        }
        Ok(Value::List(items))
    }

    /// (let ((переменная значение) ...) тело...)
    ///
    /// Все связи вычисляются в **внешнем** окружении (параллельно), затем
    /// создаётся дочернее окружение, где выполняются формы тела.
    fn do_let(&self, rest: &[Expr], env: &Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.is_empty() {
            return Err(Error::new("(let ...) требует список связей и тело"));
        }
        let bindings = match &rest[0] {
            Expr::List(items) => items,
            _ => {
                return Err(Error::new(
                    "Первым аргументом let должен быть список связей",
                ))
            }
        };

        // 1) Вычисляем все значения в ВНЕШНЕМ окружении (параллельно).
        let mut pairs: Vec<(String, Value)> = Vec::with_capacity(bindings.len());
        for b in bindings {
            let Expr::List(inner) = b else {
                return Err(Error::new(
                    "Каждая связь let должна быть списком (имя значение)",
                ));
            };
            if inner.len() != 2 {
                return Err(Error::new("Связь let должна быть списком из 2 элементов"));
            }
            let name = match &inner[0] {
                Expr::Symbol(s) => s.clone(),
                _ => return Err(Error::new("Имя в связи let должно быть символом")),
            };
            let value = self.eval(&inner[1], env.clone())?;
            pairs.push((name, value));
        }

        // 2) Новое дочернее окружение с body.
        let let_env = Environment::new_child(env.clone());
        for (name, value) in &pairs {
            Environment::define(&let_env, name, value.clone(), false);
        }

        let body_forms = &rest[1..];
        let mut result = Value::None;
        for form in body_forms {
            result = self.eval(form, let_env.clone())?;
        }
        Ok(result)
    }

    /// (match выражение (паттерн тело...) ...)
    ///
    /// Паттерны: `_`; переменная; литерал (число/строка/булево/none);
    /// `(none)`; `(some p)`; `(success p)`; `(failure p)` — где `p` — вложенный
    /// паттерн. Совпадает первая подходящая ветка; переменные паттерна
    /// связываются в окружении тела. Если ни одна ветка не совпала —
    /// runtime-ошибка.
    fn do_match(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() < 2 {
            return Err(Error::new(
                "(match ...) требует выражение и минимум одну ветку".to_string(),
            ));
        }
        let expr_val = self.eval(&rest[0], env.clone())?;
        let clauses = &rest[1..];

        for clause in clauses {
            let Expr::List(items) = clause else {
                return Err(Error::new("Каждая ветка match должна быть списком"));
            };
            if items.is_empty() {
                return Err(Error::new("Пустая ветка match"));
            }
            let pattern = &items[0];
            let body = &items[1..];

            if let Some(bindings) = Self::match_pattern(pattern, &expr_val) {
                let match_env = Environment::new_child(env.clone());
                for (name, value) in bindings {
                    Environment::define(&match_env, &name, value, false);
                }
                let mut result = Value::None;
                for form in body {
                    result = self.eval(form, match_env.clone())?;
                }
                return Ok(result);
            }
        }

        Err(Error::new("match: ни одна ветка не совпала"))
    }

    /// Сопоставляет паттерн со значением, возвращая новые связывания
    /// переменных (или `None`, если паттерн не совпал).
    fn match_pattern(pattern: &Expr, value: &Value) -> Option<Vec<(String, Value)>> {
        match pattern {
            // `_` — совпадает с чем угодно, без связываний.
            Expr::Symbol(name) if name == "_" => Some(Vec::new()),
            // Переменная — совпадает с чем угодно, связывает имя со значением.
            Expr::Symbol(name) => Some(vec![(name.clone(), value.clone())]),
            Expr::Number(n) => {
                if *value == Value::Int(*n) {
                    Some(Vec::new())
                } else {
                    None
                }
            }
            Expr::Str(s) => {
                if *value == Value::Str(s.clone()) {
                    Some(Vec::new())
                } else {
                    None
                }
            }
            Expr::Bool(b) => {
                if *value == Value::Bool(*b) {
                    Some(Vec::new())
                } else {
                    None
                }
            }
            Expr::None => {
                if *value == Value::None {
                    Some(Vec::new())
                } else {
                    None
                }
            }
            // Составные паттерны: (none), (some p), (success p), (failure p).
            Expr::List(items) => {
                if items.len() == 1 {
                    if let Expr::Symbol(name) = &items[0] {
                        if name == "none" && *value == Value::None {
                            return Some(Vec::new());
                        }
                    }
                    return None;
                }
                if items.len() == 2 {
                    if let Expr::Symbol(name) = &items[0] {
                        let inner = match name.as_str() {
                            "some" => match value {
                                Value::Some(v) => Some(v),
                                _ => None,
                            },
                            "success" => match value {
                                Value::Success(v) => Some(v),
                                _ => None,
                            },
                            "failure" => match value {
                                Value::Failure(v) => Some(v),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let Some(inner) = inner {
                            return Self::match_pattern(&items[1], inner);
                        }
                    }
                }
                None
            }
        }
    }

    /// (if-let (имя выражение) then else)
    ///
    /// Если выражение дало `(some v)` — связать `имя = v` и выполнить `then`,
    /// иначе — выполнить `else`. Работает только с Option (по спеке V1.0).
    fn do_if_let(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 3 {
            return Err(Error::new(format!(
                "(if-let ...) требует ровно 3 аргумента, получено {}",
                rest.len()
            )));
        }
        let binding = match &rest[0] {
            Expr::List(items) if items.len() == 2 => items,
            _ => {
                return Err(Error::new(
                    "Первый аргумент if-let должен быть списком (имя выражение)",
                ))
            }
        };
        let name = match &binding[0] {
            Expr::Symbol(s) => s.clone(),
            _ => return Err(Error::new("Имя в if-let должно быть символом")),
        };
        let val = self.eval(&binding[1], env.clone())?;
        let then_form = &rest[1];
        let else_form = &rest[2];

        match val {
            Value::Some(v) => {
                let branch_env = Environment::new_child(env.clone());
                Environment::define(&branch_env, &name, *v, false);
                self.eval(then_form, branch_env)
            }
            Value::None => self.eval(else_form, env.clone()),
            _ => Err(Error::new(
                "if-let: выражение должно вычисляться в (some v) или none",
            )),
        }
    }

    /// (assert выражение)
    ///
    /// Если выражение истинно — возвращает `None`, иначе — runtime-ошибка
    /// с указанием формы для отладки.
    fn do_assert(&self, rest: &[Expr], env: Rc<RefCell<Environment>>) -> Result<Value, Error> {
        if rest.len() != 1 {
            return Err(Error::new(format!(
                "(assert ...) требует ровно 1 аргумент, получено {}",
                rest.len()
            )));
        }
        let result = self.eval(&rest[0], env)?;
        if is_truthy(&result) {
            Ok(Value::None)
        } else {
            let form_text = format!("{:?}", rest[0]);
            Err(Error::new(format!("Assert не выполнен: <{}>", form_text)))
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    /// Вычисляет символ (переменную), добавляя в сообщение об ошибке
    /// подсказки с похожими именами из окружения.
    fn eval_symbol(&self, env: &Rc<RefCell<Environment>>, name: &str) -> Result<Value, Error> {
        match Environment::get(env, name) {
            Ok(value) => Ok(value),
            Err(e) if e.message.starts_with("Переменная не определена") => {
                Err(self.undefined_variable_error(env, name))
            }
            Err(e) => Err(e),
        }
    }

    /// Строит подробное сообщение о неопределённой переменной со списком
    /// вариантов, похожих по написанию Levenshtein-расстоянию.
    fn undefined_variable_error(&self, env: &Rc<RefCell<Environment>>, name: &str) -> Error {
        let known = Environment::all_names(env);

        let mut scored: Vec<(usize, String)> = known
            .into_iter()
            .map(|candidate| (levenshtein(name, &candidate), candidate))
            .collect();
        scored.sort_by_key(|(dist, _)| *dist);

        let threshold = (name.len() / 2).clamp(2, 4);
        let suggestions: Vec<String> = scored
            .into_iter()
            .filter(|(dist, _)| *dist <= threshold)
            .take(3)
            .map(|(_, candidate)| format!("'{}'", candidate))
            .collect();

        let mut message = format!("Переменная '{}' не определена", name);
        if !suggestions.is_empty() {
            message.push_str(&format!(
                ". Возможно, вы имели в виду: {}",
                suggestions.join(", ")
            ));
        }
        Error::new(message)
    }
}

/// Удаляет «лишние» закрывающие скобки `)` из потока токенов.
///
/// Используется в lenient-режиме (`--lenient-parens`): если в коде, сгенерённом
/// моделью, встречается несбалансированная `)` (например, `...))` после цикла),
/// она игнорируется с предупреждением в stderr, а не вызывает падение парсера.
///
/// Замечание: требование «игнорировать RPAREN в `eval_list()`» архитектурно
/// невыполнимо — скобки потребляются парсером до построения AST, поэтому
/// чистка производится на уровне токенов, до парсинга.
pub fn sanitize_tokens_lenient(tokens: Vec<Token>) -> Vec<Token> {
    let mut depth: i64 = 0;
    let mut out = Vec::with_capacity(tokens.len());

    for token in tokens {
        match token.kind {
            TokenKind::LParen => {
                depth += 1;
                out.push(token);
            }
            TokenKind::RParen if depth <= 0 => {
                eprintln!(
                    "Предупреждение: игнорирую лишнюю ')' на строке {}, колонке {}",
                    token.line, token.col
                );
            }
            TokenKind::RParen => {
                depth -= 1;
                out.push(token);
            }
            _ => out.push(token),
        }
    }

    out
}

/// Расстояние Левенштейна (для подсказок по неопределённым переменным).
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];

    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let del = prev[j] + 1; // удаление
            let ins = curr[j - 1] + 1; // вставка
            let sub = prev[j - 1] + cost; // замена
            curr[j] = del.min(ins).min(sub);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b.len()]
}
