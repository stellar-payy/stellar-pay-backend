use std::str::FromStr;

use rust_decimal::Decimal;
use stellar_pay_core::PaymentError;

// === Validation

// Stellar represents XLM with fixed 7-decimal precision, so anything with
// more fractional digits cannot round-trip through the ledger.
pub fn validate_amount(amount: &str) -> Result<Decimal, PaymentError> {
    let parsed =
        Decimal::from_str(amount).map_err(|_| PaymentError::InvalidAmount(amount.to_string()))?;

    if parsed <= Decimal::ZERO {
        return Err(PaymentError::InvalidAmount(amount.to_string()));
    }

    if parsed.scale() > 7 {
        return Err(PaymentError::InvalidAmount(amount.to_string()));
    }

    return Ok(parsed);
}

// === Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_amount() {
        assert!(validate_amount("10.5000000").is_ok());
    }

    #[test]
    fn accepts_whole_number_amount() {
        assert!(validate_amount("10").is_ok());
    }

    #[test]
    fn rejects_zero() {
        assert!(validate_amount("0").is_err());
    }

    #[test]
    fn rejects_negative() {
        assert!(validate_amount("-1").is_err());
    }

    #[test]
    fn rejects_non_numeric() {
        assert!(validate_amount("abc").is_err());
    }

    #[test]
    fn rejects_too_many_fractional_digits() {
        assert!(validate_amount("1.12345678").is_err());
    }
}
