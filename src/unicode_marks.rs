//! Комбинирующие знаки (Unicode general categories Mn/Mc), разрешённые
//! в продолжении идентификатора.
//!
//! Таблица намеренно компактна и покрывает только диапазоны, согласованные
//! в ERRATA V0.5.2 (BUG-45). Это не полная база Unicode: нормализация NFC/NFD
//! не выполняется, поэтому `ко́фе` (U+0301 после `о`) и `кофе` — разные
//! идентификаторы. Полные таблицы и нормализация — бэклог V1.0.
//!
//! Имена файлов модулей рекомендуется держать ASCII: составные имена с
//! комбинирующими знаками зависят от конкретной последовательности code points.

/// Диапазоны code points (включительно), относящиеся к Mn/Mc.
const MARK_RANGES: &[(u32, u32)] = &[
    (0x0300, 0x036F),
    (0x0483, 0x0489),
    (0x0591, 0x05BD),
    (0x05BF, 0x05BF),
    (0x05C1, 0x05C2),
    (0x05C4, 0x05C5),
    (0x05C7, 0x05C7),
    (0x0610, 0x061A),
    (0x064B, 0x065F),
    (0x0670, 0x0670),
    (0x06D6, 0x06DC),
    (0x06DF, 0x06E4),
    (0x06E7, 0x06E8),
    (0x06EA, 0x06ED),
    (0x0711, 0x0711),
    (0x0730, 0x074A),
    (0x07A6, 0x07B0),
    (0x07EB, 0x07F3),
    (0x0900, 0x0902),
    (0x093C, 0x093C),
    (0x0941, 0x0948),
    (0x094D, 0x094D),
    (0x0951, 0x0957),
    (0x0962, 0x0963),
    (0xFE20, 0xFE2F),
];

/// Проверяет, является ли символ комбинирующим знаком из таблицы Mn/Mc.
///
/// Таблица отсортирована, поэтому используется бинарный поиск по диапазонам.
pub fn is_combining_mark(ch: char) -> bool {
    let code = ch as u32;
    MARK_RANGES
        .binary_search_by(|&(start, end)| {
            if code < start {
                std::cmp::Ordering::Greater
            } else if code > end {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::is_combining_mark;

    #[test]
    fn covers_range_edges() {
        assert!(is_combining_mark('\u{0300}'));
        assert!(is_combining_mark('\u{0301}'));
        assert!(is_combining_mark('\u{036F}'));
        assert!(is_combining_mark('\u{FE20}'));
        assert!(is_combining_mark('\u{FE2F}'));
    }

    #[test]
    fn rejects_non_marks_and_gaps() {
        assert!(!is_combining_mark('a'));
        assert!(!is_combining_mark('я'));
        assert!(!is_combining_mark('\u{0370}'));
        assert!(!is_combining_mark('\u{05BE}'));
        assert!(!is_combining_mark('\u{05C0}'));
        assert!(!is_combining_mark('\u{05C3}'));
    }
}
