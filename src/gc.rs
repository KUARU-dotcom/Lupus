//! Консервативный mark-and-sweep сборщик мусора для `Rc`-графа Lupus.
//!
//! В языке нет сборщика с точным отслеживанием всех временных значений на
//! Rust-стеке. Поэтому используется консервативный критерий: окружение
//! считается корнем, если на него есть сильная `Rc`-ссылка, не объяснимая
//! ссылками из других зарегистрированных окружений. Такие «внешние» ссылки
//! могут приходить из глобального окружения интерпретатора, активного кадра
//! вызова, кэша модулей или значения, удерживаемого хостом.
//!
//! Mark phase обходит все достижимые от корней окружения и значения. Sweep
//! phase очищает связи недостижимых окружений (`bindings` и `parent`), что
//! разрывает циклы `env → container → closure → env`; после этого `Rc`
//! освобождает объекты обычным образом. Сборщик не сканирует Rust-стек
//! побайтово (это потребовало бы `unsafe`), но консервативно сохраняет все
//! окружения, на которые стек ещё держит сильную ссылку.
//!
//! Счётчик аллокаций атомарный, но реестр окружений — thread-local: значения
//! `Value` и `Environment` не являются `Send`, интерпретатор однопоточный.

use crate::environment::Environment;
use crate::value::{Traversable, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// Порог аллокаций, после которого автоматически запускается сборка.
pub const GC_THRESHOLD: usize = 1000;

thread_local! {
    static GC_STATE: GcState = GcState::new();
}

struct GcState {
    /// Слабые ссылки на все созданные окружения текущего потока.
    registry: RefCell<Vec<Weak<RefCell<Environment>>>>,
    /// Счётчик аллокаций с момента последней сборки.
    allocations: AtomicUsize,
    /// Защита от рекурсивного запуска GC.
    collecting: Cell<bool>,
    /// Автоматический запуск по порогу (отключается в тестах перфоманса).
    enabled: Cell<bool>,
}

impl GcState {
    fn new() -> Self {
        Self {
            registry: RefCell::new(Vec::new()),
            allocations: AtomicUsize::new(0),
            collecting: Cell::new(false),
            enabled: Cell::new(true),
        }
    }
}

/// Результат одной сборки.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GcStats {
    /// Сколько объектов (окружений и связей) очищено.
    pub collected: usize,
    /// Приблизительный объём освобождённых данных в байтах.
    pub freed_bytes: usize,
    /// Сколько окружений оказалось достижимым.
    pub marked: usize,
    /// Сколько аллокаций накопилось с прошлой сборки.
    pub allocations: usize,
    /// Длительность сборки в микросекундах.
    pub duration_micros: u64,
}

/// Регистрирует окружение и учитывает аллокацию.
///
/// Вызывается из [`Environment::new`] и [`Environment::new_child`] после
/// создания `Rc`. При достижении порога запускает сборку.
pub fn register(env: &Rc<RefCell<Environment>>) {
    GC_STATE.with(|state| {
        state.registry.borrow_mut().push(Rc::downgrade(env));
    });
    record_allocation();
}

/// Учитывает одну аллокацию.
pub fn record_allocation() {
    record_allocations(1);
}

/// Учитывает несколько аллокаций и при необходимости запускает сборку.
pub fn record_allocations(count: usize) {
    let should_collect = GC_STATE.with(|state| {
        let total = state
            .allocations
            .fetch_add(count, Ordering::Relaxed)
            .saturating_add(count);
        state.enabled.get() && total >= GC_THRESHOLD && !state.collecting.get()
    });
    if should_collect {
        let _ = collect_impl(false);
    }
}

/// Текущее значение счётчика аллокаций.
pub fn allocations() -> usize {
    GC_STATE.with(|state| state.allocations.load(Ordering::Relaxed))
}

/// Возвращает множество идентификаторов достижимых окружений.
///
/// Публичная функция нужна для диагностики и тестов; обычный код должен
/// вызывать [`collect`].
pub fn mark() -> HashSet<usize> {
    GC_STATE.with(|state| {
        let live = snapshot(state);
        mark_live(&live)
    })
}

/// Очищает недостижимые окружения, используя результат [`mark`].
pub fn sweep(marked: &HashSet<usize>) -> GcStats {
    GC_STATE.with(|state| {
        let live = snapshot(state);
        let stats = sweep_live(&live, marked);
        drop(live);
        prune_registry(state);
        stats
    })
}

/// Выполняет полную сборку. Логирует результат, если что-то было собрано.
pub fn collect() -> GcStats {
    collect_impl(false)
}

/// Выполняет полную сборку и всегда печатает строку `GC: collected ...`.
///
/// Используется встроенной функцией `(gc-collect)` и тестами логирования.
pub fn collect_verbose() -> GcStats {
    collect_impl(true)
}

fn collect_impl(force_log: bool) -> GcStats {
    let stats = GC_STATE.with(|state| {
        if state.collecting.replace(true) {
            return GcStats::default();
        }

        let started = Instant::now();
        let allocations = state.allocations.swap(0, Ordering::Relaxed);
        let live = snapshot(state);
        let marked = mark_live(&live);
        let mut stats = sweep_live(&live, &marked);
        stats.allocations = allocations;
        stats.duration_micros = started.elapsed().as_micros() as u64;

        // `snapshot` хранит Weak и для окружений, которые сейчас будут
        // очищены. Пока Rc жив, Weak нельзя отличить от живого, поэтому
        // сначала отпускаем снимок, затем убираем протухшие записи: иначе
        // заголовки RcBox очищенных окружений остаются в памяти до
        // следующей сборки.
        drop(live);
        prune_registry(state);
        state.collecting.set(false);
        stats
    });

    if force_log || stats.collected > 0 {
        eprintln!(
            "GC: collected {} objects, freed {} bytes",
            stats.collected, stats.freed_bytes
        );
    }

    stats
}

struct LiveEnv {
    id: usize,
    env: Rc<RefCell<Environment>>,
}

fn env_id(env: &Rc<RefCell<Environment>>) -> usize {
    Rc::as_ptr(env) as usize
}

/// Снимает живой срез реестра и удаляет протухшие `Weak`.
fn snapshot(state: &GcState) -> Vec<LiveEnv> {
    let mut registry = state.registry.borrow_mut();
    let old = std::mem::take(&mut *registry);
    let mut live = Vec::with_capacity(old.len());

    for weak in old {
        if let Some(env) = weak.upgrade() {
            let id = env_id(&env);
            live.push(LiveEnv { id, env });
        }
    }

    *registry = live.iter().map(|item| Rc::downgrade(&item.env)).collect();
    live
}

/// Удаляет из реестра записи, чьи окружения уже освобождены.
///
/// Это важно после sweep: `Weak` продлевает жизнь заголовка `RcBox`, даже
/// когда сильных ссылок больше нет.
fn prune_registry(state: &GcState) {
    let mut registry = state.registry.borrow_mut();
    let old = std::mem::take(&mut *registry);
    registry.extend(old.into_iter().filter(|weak| weak.strong_count() > 0));
}

/// Считает сильные ссылки на окружения, которые приходят из других
/// зарегистрированных окружений (parent + замыкания в bindings).
fn internal_refs(live: &[LiveEnv]) -> HashMap<usize, usize> {
    let mut counts = HashMap::new();

    for item in live {
        let env = item.env.borrow();
        env.visit_envs(&mut |parent| {
            *counts.entry(env_id(parent)).or_insert(0) += 1;
        });
        env.visit_values(&mut |value| count_value_refs(value, &mut counts));
    }

    counts
}

fn count_value_refs(value: &Value, counts: &mut HashMap<usize, usize>) {
    value.visit_envs(&mut |env| {
        *counts.entry(env_id(env)).or_insert(0) += 1;
    });
    value.visit_values(&mut |child| count_value_refs(child, counts));
}

/// Mark phase: корни — окружения с внешними сильными ссылками.
fn mark_live(live: &[LiveEnv]) -> HashSet<usize> {
    let internal = internal_refs(live);
    let mut stack = Vec::new();

    // Локальный Rc в `live` тоже учитывается в strong_count, поэтому
    // вычитаем его. Если после вычитания внутренних ссылок остаётся
    // положительное число, ссылка пришла извне графа окружений.
    for item in live {
        let strong = Rc::strong_count(&item.env).saturating_sub(1);
        let known_internal = internal.get(&item.id).copied().unwrap_or(0);
        if strong > known_internal {
            stack.push(item.env.clone());
        }
    }

    let mut marked = HashSet::new();
    while let Some(env) = stack.pop() {
        if !marked.insert(env_id(&env)) {
            continue;
        }

        let borrowed = env.borrow();
        borrowed.visit_envs(&mut |parent| stack.push(parent.clone()));
        borrowed.visit_values(&mut |value| mark_value(value, &mut stack));
    }

    marked
}

fn mark_value(value: &Value, stack: &mut Vec<Rc<RefCell<Environment>>>) {
    value.visit_envs(&mut |env| stack.push(env.clone()));
    value.visit_values(&mut |child| mark_value(child, stack));
}

/// Sweep phase: очищает недостижимые окружения, разрывая сильные циклы.
fn sweep_live(live: &[LiveEnv], marked: &HashSet<usize>) -> GcStats {
    let mut collected = 0;
    let mut freed_bytes = 0;

    for item in live {
        if marked.contains(&item.id) {
            continue;
        }

        let mut env = item.env.borrow_mut();
        let (objects, bytes) = env.gc_clear();
        collected += objects;
        freed_bytes += bytes;
    }

    GcStats {
        collected,
        freed_bytes,
        marked: marked.len(),
        allocations: 0,
        duration_micros: 0,
    }
}

/// Включает или отключает автоматическую сборку по порогу.
///
/// Ручной [`collect`] продолжает работать. Используется тестами перфоманса,
/// чтобы сравнить один и тот же сценарий с GC и без него.
#[doc(hidden)]
pub fn set_enabled(enabled: bool) {
    GC_STATE.with(|state| state.enabled.set(enabled));
}

/// Сбрасывает thread-local состояние GC. Используется тестами, чтобы
/// изолировать замеры; в обычном коде не нужен.
#[doc(hidden)]
pub fn reset_for_tests() {
    GC_STATE.with(|state| {
        state.registry.borrow_mut().clear();
        state.allocations.store(0, Ordering::Relaxed);
        state.collecting.set(false);
        state.enabled.set(true);
    });
}
