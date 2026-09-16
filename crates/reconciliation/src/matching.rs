use std::str::FromStr;

use rust_decimal::Decimal;

// === Amount matching

// Defense-in-depth on top of Horizon's memo-based match: a payer who
// reuses or guesses a memo but sends the wrong amount must not be
// auto-confirmed. Fails closed on any parse error.
pub fn amounts_match(expected: &str, actual: &str) -> bool {
    let expected_decimal = match Decimal::from_str(expected) {
        Ok(value) => value,
        Err(_) => return false,
    };

    let actual_decimal = match Decimal::from_str(actual) {
        Ok(value) => value,
        Err(_) => return false,
    };

    return expected_decimal == actual_decimal;
}

// === Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_equal_amounts_with_different_scale() {
        assert!(amounts_match("10", "10.0000000"));
    }

    #[test]
    fn rejects_different_amounts() {
        assert!(!amounts_match("10", "9.9999999"));
    }

    #[test]
    fn fails_closed_on_unparseable_input() {
        assert!(!amounts_match("10", "not-a-number"));
    }
}
