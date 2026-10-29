//! Property-based тесты семантики Lupus (V0.7 этап 2).
//!
//! Каждый тест прогоняет своё свойство на 1000 детерминированных входах.
//! При нарушении выводится минимальный контрпример, найденный shrink-циклом.

use lupus::property::{self, Failure, DEFAULT_CASES, DEFAULT_SEED};

fn assert_property(result: Result<(), Failure>) {
    if let Err(failure) = result {
        panic!("{}", failure.report());
    }
}

fn seed(offset: u64) -> u64 {
    DEFAULT_SEED.wrapping_add(offset)
}

#[test]
fn test_property_commutative_add() {
    assert_property(property::check_commutative_add(DEFAULT_CASES, seed(1)));
}

#[test]
fn test_property_assoc_add() {
    assert_property(property::check_assoc_add(DEFAULT_CASES, seed(2)));
}

#[test]
fn test_property_length_map() {
    assert_property(property::check_length_map(DEFAULT_CASES, seed(3)));
}

#[test]
fn test_property_length_filter() {
    assert_property(property::check_length_filter(DEFAULT_CASES, seed(4)));
}

#[test]
fn test_property_length_append() {
    assert_property(property::check_length_append(DEFAULT_CASES, seed(5)));
}

#[test]
fn test_property_string_reverse() {
    assert_property(property::check_string_reverse(DEFAULT_CASES, seed(6)));
}

#[test]
fn test_property_mul_zero() {
    assert_property(property::check_mul_zero(DEFAULT_CASES, seed(7)));
}

#[test]
fn test_property_add_zero() {
    assert_property(property::check_add_zero(DEFAULT_CASES, seed(8)));
}

#[test]
fn test_property_head_some() {
    assert_property(property::check_head_some(DEFAULT_CASES, seed(9)));
}
