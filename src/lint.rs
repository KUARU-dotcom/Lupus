//! Линт-проверки V0.5 (без HM-тайпчекера).
//!
//! Реализованы: `unused-variable`, `core-shadowing`, `duplicate-definition`,
//! `test-name-duplicate`, `test-in-function`, `import-collision`,
//! `invalid-directive`, синтаксические проверки `define-public`
//! (`missing-return-type`, `missing-param-type`), `const-non-literal`,
//! а также статическое разрешение импортов (`unknown-module`, `import-cycle`).

use crate::ast::{Expr, ExprKind};
use crate::builtins::is_core_name;
use crate::error::Error;
use crate::modules;
use crate::native;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Возвращает все диагностики линта для программы.
pub fn diagnostics(forms: &[Expr], file: Option<&Path>) -> Vec<Error> {
    let mut errors = Vec::new();

    check_test_names(forms, &mut errors);
    check_function_bodies(forms, false, &mut errors);
    // Импорты проверяются до unused-variable: коллизия `:all` должна
    // диагностироваться как import-collision, а не как неиспользуемое имя.
    check_imports(forms, file, &mut errors);
    check_definitions(forms, &mut errors);
    check_directives(forms, &mut errors);

    errors
}

/// Проверяет программу и возвращает первую диагностику.
pub fn check(forms: &[Expr], file: Option<&Path>) -> Result<(), Error> {
    match diagnostics(forms, file).into_iter().next() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Ищет объявления тестов и дубликаты имён.
fn check_test_names(forms: &[Expr], errors: &mut Vec<Error>) {
    let mut seen: HashSet<String> = HashSet::new();
    for form in forms {
        let Some(items) = form.as_list() else {
            continue;
        };
        if !matches!(items.first(), Some(Expr { kind: ExprKind::Symbol(name), .. }) if name == "test")
        {
            continue;
        }
        if let Some(Expr {
            kind: ExprKind::Str(name),
            ..
        }) = items.get(1)
        {
            if !seen.insert(name.clone()) {
                errors.push(Error::lint(
                    "test-name-duplicate",
                    format!("Тест с именем '{}' уже объявлен в этом файле", name),
                ));
            }
        }
    }
}

/// Ищет `test` внутри функций и лямбд.
fn check_function_bodies(forms: &[Expr], inside_function: bool, errors: &mut Vec<Error>) {
    for expr in forms {
        check_function_body(expr, inside_function, errors);
    }
}

fn check_function_body(expr: &Expr, inside_function: bool, errors: &mut Vec<Error>) {
    let Some(items) = expr.as_list() else {
        return;
    };
    let head = match items.first() {
        Some(Expr {
            kind: ExprKind::Symbol(name),
            ..
        }) => name.as_str(),
        _ => "",
    };

    if inside_function && head == "test" {
        errors.push(Error::lint(
            "test-in-function",
            "Форма (test ...) допустима только на верхнем уровне",
        ));
    }

    let body_slice = function_body(items);
    if let Some(body) = body_slice {
        for form in body {
            check_function_body(form, true, errors);
        }
        return;
    }

    for item in items.iter().skip(1) {
        check_function_body(item, inside_function, errors);
    }
}

fn is_function_definition(items: &[Expr]) -> bool {
    matches!(
        items.get(1),
        Some(Expr {
            kind: ExprKind::List(_),
            ..
        })
    )
}

/// Возвращает тело функции/лямбды, если форма является определением функции.
fn function_body(items: &[Expr]) -> Option<&[Expr]> {
    let head = match items.first() {
        Some(Expr {
            kind: ExprKind::Symbol(name),
            ..
        }) => name.as_str(),
        _ => return None,
    };
    match head {
        "define" | "define-public" if is_function_definition(items) => {
            let rest = items.get(2..).unwrap_or(&[]);
            if matches!(rest.first(), Some(Expr { kind: ExprKind::Symbol(arrow), .. }) if arrow == "->")
            {
                Some(rest.get(2..).unwrap_or(&[]))
            } else {
                Some(rest)
            }
        }
        "lambda" => {
            let rest = items.get(2..).unwrap_or(&[]);
            if matches!(rest.first(), Some(Expr { kind: ExprKind::Symbol(arrow), .. }) if arrow == "->")
            {
                Some(rest.get(2..).unwrap_or(&[]))
            } else {
                Some(rest)
            }
        }
        _ => None,
    }
}

/// Проверяет определения верхнего уровня.
fn check_definitions(forms: &[Expr], errors: &mut Vec<Error>) {
    let mut defined: HashSet<String> = HashSet::new();

    for form in forms {
        let Some(items) = form.as_list() else {
            continue;
        };
        let head = match items.first() {
            Some(Expr {
                kind: ExprKind::Symbol(name),
                ..
            }) => name.as_str(),
            _ => continue,
        };
        if !matches!(
            head,
            "define" | "define-mutable" | "define-const" | "define-public" | "defstruct"
        ) {
            continue;
        }

        let rest = &items[1..];
        let Some(name) = definition_name(rest) else {
            continue;
        };
        // Некорректную арность определения диагностирует интерпретатор
        // (arity-mismatch); линт анализирует только корректные формы.
        let well_formed = match head {
            "define-mutable" | "define-const" => rest.len() == 2,
            "define" | "define-public" => rest.len() >= 2,
            _ => true,
        };
        if !well_formed {
            continue;
        }

        if !defined.insert(name.clone()) {
            errors.push(Error::lint(
                "duplicate-definition",
                format!("Повторное определение '{}'", name),
            ));
        }

        if is_core_name(&name) {
            errors.push(Error::lint(
                "core-shadowing",
                format!("Имя '{}' занято функцией ядра языка", name),
            ));
        }

        if head == "define-public" {
            check_public_definition(&items[1..], &name, errors);
        }

        if head == "define-const" {
            match items.get(2) {
                Some(value) if is_literal(value) => {}
                Some(_) => errors.push(Error::lint(
                    "const-non-literal",
                    format!("Значение define-const '{}' не является литералом", name),
                )),
                None => {}
            }
        }

        if head != "define-public" && head != "defstruct" {
            let uses: usize = forms.iter().map(|form| count_symbol(form, &name)).sum();
            if uses <= 1 {
                errors.push(Error::lint(
                    "unused-variable",
                    format!("Переменная '{}' определена, но не используется", name),
                ));
            }
        }
    }
}

fn definition_name(rest: &[Expr]) -> Option<String> {
    match rest.first() {
        Some(Expr {
            kind: ExprKind::Symbol(name),
            ..
        }) => Some(name.clone()),
        Some(Expr {
            kind: ExprKind::List(header),
            ..
        }) => match header.first() {
            Some(Expr {
                kind: ExprKind::Symbol(name),
                ..
            }) => Some(name.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn is_literal(expr: &Expr) -> bool {
    matches!(
        &expr.kind,
        ExprKind::Number(_)
            | ExprKind::Float(_)
            | ExprKind::Str(_)
            | ExprKind::Bool(_)
            | ExprKind::None
    )
}

fn count_symbol(expr: &Expr, name: &str) -> usize {
    match &expr.kind {
        ExprKind::Symbol(symbol) => usize::from(symbol == name),
        ExprKind::List(items) => items.iter().map(|item| count_symbol(item, name)).sum(),
        _ => 0,
    }
}

/// Синтаксические требования к `define-public`.
fn check_public_definition(rest: &[Expr], name: &str, errors: &mut Vec<Error>) {
    let Some(Expr {
        kind: ExprKind::List(header),
        ..
    }) = rest.first()
    else {
        errors.push(Error::type_error(
            "missing-return-type",
            format!(
                "define-public '{}' требует заголовок функции и аннотацию -> тип",
                name
            ),
        ));
        return;
    };

    for param in header.iter().skip(1) {
        let annotated = matches!(&param.kind, ExprKind::List(items) if items.len() >= 2);
        if !annotated {
            errors.push(Error::type_error(
                "missing-param-type",
                format!("Параметр public-функции '{}' не имеет аннотации типа", name),
            ));
            break;
        }
    }

    let has_return_type = matches!(rest.get(1), Some(Expr { kind: ExprKind::Symbol(arrow), .. }) if arrow == "->")
        && rest.get(2).is_some();
    if !has_return_type {
        errors.push(Error::type_error(
            "missing-return-type",
            format!("Public-функция '{}' не имеет аннотации -> тип", name),
        ));
    }
}

/// Проверяет директивы `#lupus` рекурсивно.
fn check_directives(forms: &[Expr], errors: &mut Vec<Error>) {
    for expr in forms {
        check_directive(expr, errors);
    }
}

fn check_directive(expr: &Expr, errors: &mut Vec<Error>) {
    if let ExprKind::List(items) = &expr.kind {
        {
            if matches!(items.first(), Some(Expr { kind: ExprKind::Symbol(name), .. }) if name == "#lupus")
            {
                let directive = match items.get(1) {
                    Some(Expr {
                        kind: ExprKind::Symbol(name),
                        ..
                    }) => name.as_str(),
                    _ => "",
                };
                match directive {
                    "enable-check" => {}
                    "ffi" => errors.push(
                        Error::lint(
                            "invalid-directive",
                            "Python-FFI объявлен заменённым нативными модулями",
                        )
                        .with_hint(
                            "используйте (import senko), (import kaltsit) или (import w) вместо (#lupus ffi python ...)",
                        ),
                    ),
                    other => errors.push(Error::lint(
                        "invalid-directive",
                        if other.is_empty() {
                            "Директива #lupus требует имя".to_string()
                        } else {
                            format!("Неизвестная директива #lupus {}", other)
                        },
                    )),
                }
            }
            for item in items {
                check_directive(item, errors);
            }
        }
    }
}

/// Статически разрешает импорты и проверяет коллизии `:all`.
fn check_imports(forms: &[Expr], file: Option<&Path>, errors: &mut Vec<Error>) {
    let extra_dirs: Vec<PathBuf> = file
        .and_then(|path| path.parent().map(|parent| vec![parent.to_path_buf()]))
        .unwrap_or_default();
    let mut stack = Vec::new();
    let mut visited = HashSet::new();
    check_imports_in(forms, &extra_dirs, &mut stack, &mut visited, errors);
}

fn check_imports_in(
    forms: &[Expr],
    extra_dirs: &[PathBuf],
    stack: &mut Vec<PathBuf>,
    visited: &mut HashSet<PathBuf>,
    errors: &mut Vec<Error>,
) {
    let local_names: HashSet<String> = forms
        .iter()
        .filter_map(|form| match &form.kind {
            ExprKind::List(items)
                if matches!(
                    items.first(),
                    Some(Expr { kind: ExprKind::Symbol(head), .. })
                        if matches!(
                            head.as_str(),
                            "define" | "define-mutable" | "define-const" | "define-public"
                        )
                ) =>
            {
                definition_name(&items[1..])
            }
            _ => None,
        })
        .collect();

    let mut imported_names: HashSet<String> = HashSet::new();

    for form in forms {
        if !modules::is_import_form(form) {
            continue;
        }
        let Some(items) = form.as_list() else {
            continue;
        };
        let spec = match modules::parse_import(&items[1..]) {
            Ok(spec) => spec,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };

        let mut exported: Vec<String> = Vec::new();
        if spec.is_native() {
            exported.extend(native::names(&spec.package).iter().map(|s| s.to_string()));
        } else {
            match modules::resolve_file_from(&spec.package, extra_dirs) {
                None => {
                    errors.push(
                        Error::lint(
                            "unknown-module",
                            format!(
                                "Неизвестный модуль '{}': нет нативного пакета и файла stdlib/{}.lupus",
                                spec.package, spec.package
                            ),
                        )
                        .with_hint(format!(
                            "Доступные нативные пакеты V0.6: {}",
                            native::PACKAGES.join(", ")
                        )),
                    );
                    continue;
                }
                Some(path) => {
                    let canonical = modules::canonical(&path);
                    if stack.contains(&canonical) {
                        errors.push(Error::lint(
                            "import-cycle",
                            format!("Циклический импорт модуля: {}", path.display()),
                        ));
                        continue;
                    }
                    if visited.insert(canonical.clone()) {
                        stack.push(canonical);
                        match std::fs::read_to_string(&path)
                            .map_err(|error| {
                                Error::lint(
                                    "unknown-module",
                                    format!(
                                        "Не удалось прочитать модуль {}: {}",
                                        path.display(),
                                        error
                                    ),
                                )
                            })
                            .and_then(|source| {
                                crate::parse_source(&source).map_err(|error| {
                                    error.with_file_if_located(path.display().to_string())
                                })
                            }) {
                            Ok(module_forms) => {
                                exported = module_forms
                                    .iter()
                                    .filter_map(|form| match &form.kind {
                                        ExprKind::List(items)
                                            if matches!(
                                                items.first(),
                                                Some(Expr { kind: ExprKind::Symbol(head), .. }) if head == "define-public"
                                            ) =>
                                        {
                                            definition_name(&items[1..])
                                        }
                                        _ => None,
                                    })
                                    .collect();
                                check_imports_in(&module_forms, extra_dirs, stack, visited, errors);
                            }
                            Err(error) => {
                                errors.push(error);
                            }
                        }
                        stack.pop();
                    }
                }
            }
        }

        if spec.import_all {
            for name in exported {
                if local_names.contains(&name)
                    || imported_names.contains(&name)
                    || is_core_name(&name)
                {
                    errors.push(Error::lint(
                        "import-collision",
                        format!(
                            "Имя '{}' из модуля '{}' конфликтует с существующим именем",
                            name, spec.package
                        ),
                    ));
                }
                imported_names.insert(name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Vec<Expr> {
        crate::parse_source(src).expect("parse")
    }

    #[test]
    fn lint_clean_program() {
        let forms = parse("(define (f x) x) (print (f 1))");
        assert!(check(&forms, None).is_ok());
    }

    #[test]
    fn lint_unused_variable() {
        let forms = parse("(define y 10)");
        let error = check(&forms, None).unwrap_err();
        assert_eq!(error.code, "unused-variable");
    }
}
