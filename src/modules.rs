//! Разрешение модулей и разбор формы `import`.
//!
//! Порядок поиска (ERRATA §1): нативный реестр → `stdlib/<имя>.lupus` →
//! каталоги из `LUPUS_PATH`. Нативные пакеты V0.6: `senko`, `kaltsit`, `w`,
//! `texas`, `lappland`, `amiya`.

use crate::ast::{Expr, ExprKind};
use crate::error::Error;
use crate::native;
use std::path::{Path, PathBuf};

/// Разобранная форма импорта.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSpec {
    /// Имя пакета.
    pub package: String,
    /// Второй уровень `(import (pkg mod))`, если указан.
    pub module: Option<String>,
    /// Переопределение префикса через `:as`.
    pub alias: Option<String>,
    /// Импорт всех имён без префикса.
    pub import_all: bool,
}

impl ImportSpec {
    /// Префикс квалифицированных имён после импорта.
    pub fn prefix(&self) -> String {
        self.alias
            .clone()
            .or_else(|| self.module.clone())
            .unwrap_or_else(|| self.package.clone())
    }

    /// Является ли пакет нативным.
    pub fn is_native(&self) -> bool {
        native::is_native(&self.package)
    }
}

/// Проверяет, является ли форма формой импорта.
pub fn is_import_form(expr: &Expr) -> bool {
    matches!(&expr.kind, ExprKind::List(items) if matches!(items.first(), Some(Expr { kind: ExprKind::Symbol(name), .. }) if name == "import"))
}

/// Разбирает `rest` формы `(import ...)` в [`ImportSpec`].
pub fn parse_import(rest: &[Expr]) -> Result<ImportSpec, Error> {
    if rest.is_empty() {
        return Err(Error::lint(
            "unexpected-token",
            "(import ...) требует имя пакета",
        ));
    }

    let (package, module) = match &rest[0].kind {
        ExprKind::Symbol(package) => (package.clone(), None),
        ExprKind::List(items) => {
            if items.len() != 2 {
                return Err(Error::lint(
                    "unexpected-token",
                    "(import (пакет модуль)) требует ровно два имени",
                ));
            }
            let package = match &items[0].kind {
                ExprKind::Symbol(name) => name.clone(),
                _ => {
                    return Err(Error::lint(
                        "unexpected-token",
                        "Имя пакета в import должно быть символом",
                    ))
                }
            };
            let module = match &items[1].kind {
                ExprKind::Symbol(name) => name.clone(),
                _ => {
                    return Err(Error::lint(
                        "unexpected-token",
                        "Имя модуля в import должно быть символом",
                    ))
                }
            };
            (package, Some(module))
        }
        _ => {
            return Err(Error::lint(
                "unexpected-token",
                "Ожидалось имя пакета в форме import",
            ))
        }
    };

    let mut alias = None;
    let mut import_all = false;
    let mut index = 1;
    while index < rest.len() {
        match &rest[index].kind {
            ExprKind::Symbol(modifier) if modifier == ":as" => {
                let Some(Expr {
                    kind: ExprKind::Symbol(name),
                    ..
                }) = rest.get(index + 1)
                else {
                    return Err(Error::lint(
                        "unexpected-token",
                        "После :as ожидалось имя префикса",
                    ));
                };
                alias = Some(name.clone());
                index += 2;
            }
            ExprKind::Symbol(modifier) if modifier == ":all" => {
                import_all = true;
                index += 1;
            }
            other => {
                return Err(Error::lint(
                    "unexpected-token",
                    format!("Неизвестный модификатор импорта: {:?}", other),
                ))
            }
        }
    }

    Ok(ImportSpec {
        package,
        module,
        alias,
        import_all,
    })
}

/// Каталоги, в которых ищется `stdlib/<имя>.lupus`.
pub fn stdlib_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(current) = std::env::current_dir() {
        dirs.push(current.join("stdlib"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("stdlib"));
            if let Some(grand) = parent.parent() {
                dirs.push(grand.join("stdlib"));
            }
        }
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        dirs.push(PathBuf::from(manifest).join("stdlib"));
    }

    dirs
}

/// Каталоги из переменной окружения `LUPUS_PATH` (разделитель — `:`).
pub fn lup_path_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(value) = std::env::var("LUPUS_PATH") {
        for entry in value.split(':') {
            if entry.is_empty() {
                continue;
            }
            let base = PathBuf::from(entry);
            dirs.push(base.clone());
            dirs.push(base.join("stdlib"));
        }
    }
    dirs
}

/// Ищет файл модуля `<package>.lupus`, включая дополнительные каталоги.
pub fn resolve_file_from(package: &str, extra_dirs: &[PathBuf]) -> Option<PathBuf> {
    let filename = format!("{}.lupus", package);
    let dirs = extra_dirs
        .iter()
        .cloned()
        .chain(stdlib_dirs())
        .chain(lup_path_dirs());
    for dir in dirs {
        let candidate = dir.join(&filename);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Ищет файл модуля `<package>.lupus`.
pub fn resolve_file(package: &str) -> Option<PathBuf> {
    resolve_file_from(package, &[])
}

/// Находит модуль: нативный пакет или файл.
///
/// Возвращает `Ok(None)` для нативного пакета и `Ok(Some(path))` для файла.
pub fn resolve(spec: &ImportSpec) -> Result<Option<PathBuf>, Error> {
    if spec.is_native() {
        return Ok(None);
    }
    match resolve_file(&spec.package) {
        Some(path) => Ok(Some(path)),
        None => Err(Error::lint(
            "unknown-module",
            format!(
                "Неизвестный модуль '{}': нет нативного пакета и файла stdlib/{}.lupus",
                spec.package, spec.package
            ),
        )
        .with_hint(format!(
            "Доступные нативные пакеты V0.6: {}",
            native::PACKAGES.join(", ")
        ))),
    }
}

/// Канонизирует путь модуля для кэша и обнаружения циклов.
pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
