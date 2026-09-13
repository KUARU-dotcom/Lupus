//! Харнес тест-системы LiF (Lupus in Fabula) — выпускные ворота релиза V0.4.
//!
//! Обходит каталоги `tests/golden/` и `tests/negative/`, запускает бинарник
//! интерпретатора и сверяет stdout байт-в-байт и коды выхода. Дополнительно
//! проверяет карту покрытия `tests/coverage_map.toml`: у каждого символа должна
//! быть как минимум одна positive- и одна negative-проверка, и все перечисленные
//! тесты должны существовать на диске. Падение любого файла проваливает тест
//! с указанием имени файла.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

/// Корень каталога тестов (`tests/`).
fn test_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// Запускает бинарник интерпретатора на файле, возвращает (stdout, код выхода).
///
/// Применяется таймаут 5 секунд: если интерпретатор зависает на файле (например,
/// на неизвестном символе), вместо бесконечного ожидания возвращается код
/// выхода -1 и пометка времени ожидания в stdout — тест падает, а не виснет.
fn run_bin(file: &Path) -> (String, i32) {
    let file = file.to_path_buf();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let out = Command::new(env!("CARGO_BIN_EXE_lupus"))
            .arg(&file)
            .output()
            .expect("не удалось запустить интерпретатор");
        let _ = tx.send((
            String::from_utf8_lossy(&out.stdout).to_string(),
            out.status.code().unwrap_or(-1),
        ));
    });

    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(result) => result,
        Err(_) => (String::from("<TIME-OUT: интерпретатор завис на файле>"), -1),
    }
}

/// Возвращает отсортированный список `.lupus`-файлов в каталоге.
fn list_lupus_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("не удалось прочитать каталог тестов")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "lupus").unwrap_or(false))
        .collect();
    files.sort();
    files
}

/// Golden-тесты: точное совпадение stdout и код выхода 0.
#[test]
fn lif_golden() {
    let dir = test_root().join("golden");
    let files = list_lupus_files(&dir);
    assert!(!files.is_empty(), "каталог golden пуст — нет golden-тестов");

    let mut errors = Vec::new();
    for file in &files {
        let expected_path = file.with_extension("expected");
        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| String::from("<нет .expected файла>"));
        let (out, code) = run_bin(file);
        let name = file.file_name().unwrap().to_string_lossy();
        if code != 0 {
            errors.push(format!("[{}] код выхода = {} (ожидался 0)", name, code));
        }
        if out != expected {
            errors.push(format!(
                "[{}] stdout не совпадает (ожидалось {}/байт, получено {}/байт)",
                name,
                expected.len(),
                out.len()
            ));
        }
    }
    assert!(
        errors.is_empty(),
        "Golden-провалы ({}/{}):\n{}",
        errors.len(),
        files.len(),
        errors.join("\n")
    );
}

/// Negative-тесты: ненулевой код выхода и подстрока сообщения в stdout.
#[test]
fn lif_negative() {
    let dir = test_root().join("negative");
    let files = list_lupus_files(&dir);
    assert!(
        !files.is_empty(),
        "каталог negative пуст — нет negative-тестов"
    );

    let mut errors = Vec::new();
    for file in &files {
        let error_path = file.with_extension("error");
        let want = std::fs::read_to_string(&error_path)
            .unwrap_or_else(|_| String::from("<нет .error файла>"));
        let want = want.trim();
        let (out, code) = run_bin(file);
        let name = file.file_name().unwrap().to_string_lossy();
        if code == 0 {
            errors.push(format!("[{}] код выхода = 0 (ожидался ненулевой)", name));
        }
        if !out.contains(want) {
            errors.push(format!(
                "[{}] сообщение не содержит подстроку {:?} (получено: {:?})",
                name, want, out
            ));
        }
    }
    assert!(
        errors.is_empty(),
        "Negative-провалы ({}/{}):\n{}",
        errors.len(),
        files.len(),
        errors.join("\n")
    );
}

/// Извлекает список элементов из фрагмента вида `positive = [ "a", "b" ]`.
fn extract_list(section: &str, marker: &str) -> Vec<String> {
    let Some(idx) = section.find(marker) else {
        return Vec::new();
    };
    let rest = &section[idx + marker.len()..];
    let Some(open) = rest.find('[') else {
        return Vec::new();
    };
    let inner = &rest[open + 1..];
    let Some(close) = inner.find(']') else {
        return Vec::new();
    };
    inner[..close]
        .split(',')
        .map(|x| x.trim().trim_matches('"').to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

/// Разбирает строку карты покрытия в (символ, positive, negative).
fn parse_coverage_line(line: &str) -> (String, Vec<String>, Vec<String>) {
    let key = line.split('"').nth(1).unwrap_or("").to_string();
    let pos = extract_list(line, "positive =");
    let neg = extract_list(line, "negative =");
    (key, pos, neg)
}

/// Coverage-map: у каждого символа ≥1 positive и ≥1 negative, и все
/// перечисленные тесты существуют на диске.
#[test]
fn lif_coverage_map() {
    let path = test_root().join("coverage_map.toml");
    let text = std::fs::read_to_string(&path).expect("отсутствует tests/coverage_map.toml");

    let mut errors = Vec::new();
    let mut symbol_count = 0usize;

    let mut in_symbols = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[symbols]" {
            in_symbols = true;
            continue;
        }
        if !in_symbols {
            continue;
        }

        let (sym, pos, neg) = parse_coverage_line(line);
        if sym.is_empty() {
            errors.push(format!("строка не разобрана: {}", line));
            continue;
        }
        symbol_count += 1;

        if pos.is_empty() {
            errors.push(format!("[{}] нет ни одной positive-проверки", sym));
        }
        if neg.is_empty() {
            errors.push(format!("[{}] нет ни одной negative-проверки", sym));
        }

        for p in &pos {
            if !test_root()
                .join("golden")
                .join(format!("{}.lupus", p))
                .exists()
            {
                errors.push(format!("[{}] golden-файл отсутствует: {}.lupus", sym, p));
            }
            if !test_root()
                .join("golden")
                .join(format!("{}.expected", p))
                .exists()
            {
                errors.push(format!(
                    "[{}] expected-файл отсутствует: {}.expected",
                    sym, p
                ));
            }
        }
        for n in &neg {
            if !test_root()
                .join("negative")
                .join(format!("{}.lupus", n))
                .exists()
            {
                errors.push(format!("[{}] negative-файл отсутствует: {}.lupus", sym, n));
            }
            if !test_root()
                .join("negative")
                .join(format!("{}.error", n))
                .exists()
            {
                errors.push(format!("[{}] error-файл отсутствует: {}.error", sym, n));
            }
        }
    }

    assert!(
        symbol_count >= 1,
        "секция [symbols] пуста или отсутствует в coverage_map.toml"
    );
    assert!(
        errors.is_empty(),
        "Ошибки карты покрытия (символов: {}):\n{}",
        symbol_count,
        errors.join("\n")
    );
}
