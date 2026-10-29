//! Минимизация программ Lupus на уровне AST.
//!
//! Когда fuzz находит панику, исходная программа может быть очень большой.
//! Shrinker жадно перебирает эквивалентные по наблюдаемому поведению
//! преобразования AST и оставляет только те, при которых баг всё ещё
//! воспроизводится:
//!
//! - удаление не относящихся к багу форм верхнего уровня (ddmin-подобно);
//! - удаление веток `if`/`cond`/`match`/`begin` и тел циклов;
//! - замена глубоких выражений простыми константами;
//! - сокращение списков и списков аргументов.
//!
//! Каждый кандидат сериализуется через [`crate::ast::expr_to_source`], поэтому
//! скобки всегда сбалансированы. Проверка «баг всё ещё есть» выполняется
//! переданным замыканием: для fuzz это повторный запуск CLI и сравнение класса
//! отказа (panic/аварийный код), для тестов — проверка кода runtime-ошибки.

use crate::ast::{expr_to_source, Expr, ExprKind};
use crate::error::Error;

/// Максимальное число проверок кандидатов по умолчанию.
pub const DEFAULT_MAX_ATTEMPTS: usize = 4_000;

/// Настройки shrink-цикла.
#[derive(Debug, Clone, Copy)]
pub struct ShrinkConfig {
    /// Максимальное число запусков проверяющего замыкания.
    pub max_attempts: usize,
}

impl Default for ShrinkConfig {
    fn default() -> Self {
        ShrinkConfig {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
        }
    }
}

/// Результат минимизации.
#[derive(Debug, Clone)]
pub struct ShrinkReport {
    /// Исходная программа после нормализации AST.
    pub original_source: String,
    /// Минимальная программа, всё ещё воспроизводящая баг.
    pub minimized_source: String,
    /// Число строк исходной программы.
    pub original_lines: usize,
    /// Число строк минимальной программы.
    pub minimized_lines: usize,
    /// Сколько кандидатов проверено.
    pub attempts: usize,
    /// Сколько преобразований принято.
    pub accepted: usize,
}

impl ShrinkReport {
    /// Сколько строк удалено.
    pub fn removed_lines(&self) -> usize {
        self.original_lines.saturating_sub(self.minimized_lines)
    }

    /// Насколько уменьшился размер в процентах (0–100).
    pub fn reduction_percent(&self) -> usize {
        (self.removed_lines() * 100)
            .checked_div(self.original_lines)
            .unwrap_or(0)
    }
}

/// Минимизирует `source`, пока `reproduces` возвращает `true`.
///
/// Исходная программа обязана воспроизводить баг, иначе возвращается
/// `shrink-not-reproducible`. Если проверяющее замыкание перестало
/// подтверждать минимальную программу (например, из-за недетерминизма),
/// возвращается исходная нормализованная программа.
pub fn shrink_source<F>(source: &str, reproduces: F) -> Result<ShrinkReport, Error>
where
    F: FnMut(&str) -> bool,
{
    shrink_source_with_config(source, ShrinkConfig::default(), reproduces)
}

/// То же, что [`shrink_source`], но с явным лимитом попыток.
pub fn shrink_source_with_config<F>(
    source: &str,
    config: ShrinkConfig,
    mut reproduces: F,
) -> Result<ShrinkReport, Error>
where
    F: FnMut(&str) -> bool,
{
    let forms = crate::parse_source(source)?;
    let original_source = render_program(&forms);

    if !reproduces(&original_source) {
        return Err(Error::lint(
            "shrink-not-reproducible",
            "Исходная программа не воспроизводит баг",
        ));
    }

    let (forms, attempts, accepted) = {
        let mut shrinker = Shrinker {
            forms,
            reproduces: &mut reproduces,
            max_attempts: config.max_attempts.max(1),
            attempts: 0,
            accepted: 0,
        };
        shrinker.reduce_top_level();
        shrinker.simplify_all();
        (shrinker.forms, shrinker.attempts, shrinker.accepted)
    };

    let minimized_source = render_program(&forms);
    let minimized_source = if reproduces(&minimized_source) {
        minimized_source
    } else {
        original_source.clone()
    };

    Ok(ShrinkReport {
        original_lines: count_lines(source),
        minimized_lines: count_lines(&minimized_source),
        original_source,
        minimized_source,
        attempts,
        accepted,
    })
}

/// Сериализует набор форм верхнего уровня в исходный текст.
pub fn render_program(forms: &[Expr]) -> String {
    let mut out = String::new();
    for form in forms {
        out.push_str(&expr_to_source(form));
        out.push('\n');
    }
    out
}

fn count_lines(source: &str) -> usize {
    source.lines().count().max(1)
}

struct Shrinker<'a, F: FnMut(&str) -> bool> {
    forms: Vec<Expr>,
    reproduces: &'a mut F,
    max_attempts: usize,
    attempts: usize,
    accepted: usize,
}

impl<F> Shrinker<'_, F>
where
    F: FnMut(&str) -> bool,
{
    /// Проверяет, что кандидат всё ещё воспроизводит баг.
    fn reproduces(&mut self, forms: &[Expr]) -> bool {
        if self.attempts >= self.max_attempts {
            return false;
        }
        let source = render_program(forms);
        debug_assert!(
            crate::generator::is_balanced(&source),
            "shrink: несбалансированный кандидат:\n{source}"
        );
        self.attempts += 1;
        (self.reproduces)(&source)
    }

    /// Удаляет крупные блоки форм верхнего уровня, затем отдельные формы.
    ///
    /// Размер блока уменьшается вдвое: для 500 строк это порядка десяти
    /// проверок вместо 500.
    fn reduce_top_level(&mut self) {
        let mut size = self.forms.len() / 2;
        while size >= 1 {
            let mut index = 0;
            while index + size <= self.forms.len() {
                if self.forms.len() <= size {
                    break;
                }
                let mut candidate = self.forms.clone();
                candidate.drain(index..index + size);
                if self.reproduces(&candidate) {
                    self.forms = candidate;
                    self.accepted += 1;
                } else {
                    index += size;
                }
            }
            if size == 1 {
                break;
            }
            size /= 2;
        }
    }

    /// Жадно применяет упрощения к первому подходящему узлу.
    fn simplify_all(&mut self) {
        loop {
            let mut paths = Vec::new();
            let mut prefix = Vec::new();
            collect_paths(&self.forms, &mut prefix, &mut paths);

            let mut changed = false;
            for path in paths {
                if self.simplify_path(&path) {
                    changed = true;
                    break;
                }
                if self.attempts >= self.max_attempts {
                    return;
                }
            }

            if !changed {
                break;
            }
        }
    }

    /// Пробует заменить узел по пути `path` более простым вариантом.
    fn simplify_path(&mut self, path: &[usize]) -> bool {
        let Some(node) = get_at(&self.forms, path).cloned() else {
            return false;
        };
        let current_size = expr_size(&node);

        for candidate in node_candidates(&node) {
            if expr_size(&candidate) >= current_size {
                continue;
            }

            let mut forms = self.forms.clone();
            if !replace_at(&mut forms, path, candidate) {
                continue;
            }

            if self.reproduces(&forms) {
                self.forms = forms;
                self.accepted += 1;
                return true;
            }

            if self.attempts >= self.max_attempts {
                return false;
            }
        }

        false
    }
}

fn collect_paths(forms: &[Expr], prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    for (index, form) in forms.iter().enumerate() {
        prefix.push(index);
        out.push(prefix.clone());
        if let ExprKind::List(items) = &form.kind {
            collect_paths(items, prefix, out);
        }
        prefix.pop();
    }
}

fn get_at<'a>(forms: &'a [Expr], path: &[usize]) -> Option<&'a Expr> {
    let (first, rest) = path.split_first()?;
    let form = forms.get(*first)?;
    if rest.is_empty() {
        Some(form)
    } else {
        get_at(form.as_list()?, rest)
    }
}

fn replace_at(forms: &mut [Expr], path: &[usize], replacement: Expr) -> bool {
    let Some((first, rest)) = path.split_first() else {
        return false;
    };
    let Some(form) = forms.get_mut(*first) else {
        return false;
    };
    if rest.is_empty() {
        *form = replacement;
        return true;
    }
    match &mut form.kind {
        ExprKind::List(items) => replace_at(items, rest, replacement),
        _ => false,
    }
}

/// Размер узла: число узлов плюс длина строковых и символьных литералов.
///
/// Нужен, чтобы принимать только строго уменьшающие преобразования и не
/// зацикливаться на заменах `символ ↔ константа` того же размера.
fn expr_size(expr: &Expr) -> usize {
    match &expr.kind {
        ExprKind::List(items) => 1 + items.iter().map(expr_size).sum::<usize>(),
        ExprKind::Str(value) => 1 + value.chars().count(),
        ExprKind::Symbol(name) => 1 + name.chars().count(),
        _ => 1,
    }
}

fn simple_constants() -> Vec<Expr> {
    vec![
        Expr::unspanned(ExprKind::None),
        Expr::unspanned(ExprKind::Number(0)),
        Expr::unspanned(ExprKind::Bool(false)),
        Expr::unspanned(ExprKind::Str(String::new())),
        Expr::unspanned(ExprKind::Bool(true)),
    ]
}

/// Возвращает более простые варианты узла в порядке предпочтения.
fn node_candidates(expr: &Expr) -> Vec<Expr> {
    let mut out = Vec::new();
    let ExprKind::List(items) = &expr.kind else {
        return out;
    };
    if items.is_empty() {
        return out;
    }

    let mut push = |candidate: Expr| {
        if candidate != *expr && !out.contains(&candidate) {
            out.push(candidate);
        }
    };

    // Самое агрессивное: заменить узел простой константой.
    for constant in simple_constants() {
        push(constant);
    }

    let head = items[0].as_symbol();
    match head {
        Some("begin") => {
            for body in &items[1..] {
                push(body.clone());
            }
            for index in 1..items.len() {
                push(without_item(items, index));
            }
        }
        Some("if") => {
            if let Some(branch) = items.get(2) {
                push(branch.clone());
            }
            if let Some(branch) = items.get(3) {
                push(branch.clone());
            }
        }
        Some("cond") => {
            for clause in &items[1..] {
                if let Some(list) = clause.as_list() {
                    if let Some(body) = list.last() {
                        push(body.clone());
                    }
                }
            }
            for index in 1..items.len() {
                push(without_item(items, index));
            }
        }
        Some("match") => {
            for clause in items.iter().skip(2) {
                if let Some(list) = clause.as_list() {
                    if let Some(body) = list.last() {
                        push(body.clone());
                    }
                }
            }
            for index in 2..items.len() {
                push(without_item(items, index));
            }
        }
        Some("while") | Some("for") | Some("loop") | Some("lambda") => {
            if let Some(body) = items.last() {
                push(body.clone());
            }
        }
        Some("define") | Some("define-public") | Some("define-const") | Some("define-mutable") => {
            if items.len() > 2 {
                if let Some(body) = items.last() {
                    push(body.clone());
                }
            }
        }
        Some("set!") => {
            if let Some(value) = items.get(2) {
                push(value.clone());
            }
        }
        Some("import") | Some("defstruct") => {}
        _ => {}
    }

    // Сокращение списка аргументов: убираем по одному элементу с конца.
    for index in (1..items.len()).rev() {
        push(without_item(items, index));
    }

    // Замена аргументов константами (в том числе «глубоких» выражений).
    for index in 1..items.len() {
        for constant in simple_constants() {
            let mut candidate = items.to_vec();
            candidate[index] = constant;
            push(Expr::unspanned(ExprKind::List(candidate)));
        }
    }

    // Литерал пустого списка как более простая форма любого списка.
    if head == Some("list") {
        push(Expr::unspanned(ExprKind::List(Vec::new())));
    }

    out
}

fn without_item(items: &[Expr], index: usize) -> Expr {
    let mut candidate = items.to_vec();
    candidate.remove(index);
    Expr::unspanned(ExprKind::List(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expr_size_prefers_constants() {
        let nested = Expr::unspanned(ExprKind::List(vec![
            Expr::unspanned(ExprKind::Symbol("+".to_string())),
            Expr::unspanned(ExprKind::Number(1)),
            Expr::unspanned(ExprKind::Number(2)),
        ]));
        assert!(expr_size(&nested) > expr_size(&Expr::unspanned(ExprKind::Number(0))));
    }

    #[test]
    fn node_candidates_include_branch_and_constant_reductions() {
        let form = Expr::unspanned(ExprKind::List(vec![
            Expr::unspanned(ExprKind::Symbol("if".to_string())),
            Expr::unspanned(ExprKind::Bool(true)),
            Expr::unspanned(ExprKind::Number(1)),
            Expr::unspanned(ExprKind::Number(2)),
        ]));
        let candidates = node_candidates(&form);
        assert!(candidates
            .iter()
            .any(|candidate| expr_to_source(candidate) == "1"));
        assert!(candidates
            .iter()
            .any(|candidate| expr_to_source(candidate) == "0"));
    }

    #[test]
    fn replace_at_replaces_nested_node() {
        let mut forms = vec![Expr::unspanned(ExprKind::List(vec![
            Expr::unspanned(ExprKind::Symbol("print".to_string())),
            Expr::unspanned(ExprKind::Number(1)),
        ]))];
        assert!(replace_at(
            &mut forms,
            &[0, 1],
            Expr::unspanned(ExprKind::Number(7))
        ));
        assert_eq!(render_program(&forms), "(print 7)\n");
    }
}
