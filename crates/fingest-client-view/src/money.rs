use fingest_kernel::Money;

/// Minor units. Every currency the API has carried so far is two-decimal, and the database
/// column is `NUMERIC(19,2)`, so this matches what is actually stored.
const SCALE: i64 = 2;

/// Renders money for display.
///
/// `BigDecimal` keeps the scale it was computed with, so subtracting `125.50` from `2500`
/// yields `2374.5000` — arithmetically right, and wrong on screen. Rounding for display
/// only, never for storage, keeps both true.
pub fn format_money(money: &Money) -> String {
    format!("{} {}", money.amount.round(SCALE), money.currency)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fingest_client_wallets_core::parse_money;

    fn shown(amount: &str) -> String {
        format_money(&parse_money(amount, "EUR").unwrap())
    }

    /// The case that motivated this: a balance after one subtraction.
    #[test]
    fn trailing_scale_from_arithmetic_is_not_shown() {
        assert_eq!(shown("2374.5000"), "2374.50 EUR");
    }

    #[test]
    fn a_whole_number_still_shows_minor_units() {
        assert_eq!(shown("2500"), "2500.00 EUR");
    }

    #[test]
    fn a_single_decimal_is_padded() {
        assert_eq!(shown("0.5"), "0.50 EUR");
    }

    #[test]
    fn a_negative_balance_keeps_its_sign() {
        assert_eq!(shown("-12.3"), "-12.30 EUR");
    }

    #[test]
    fn the_currency_is_the_one_supplied() {
        assert_eq!(format_money(&parse_money("1", "usd").unwrap()), "1.00 USD");
    }

    /// Display rounds; the value behind it does not. A large figure must not be truncated.
    #[test]
    fn a_large_amount_is_not_truncated() {
        assert_eq!(shown("12345678901234.56"), "12345678901234.56 EUR");
    }

    #[test]
    fn a_third_decimal_rounds_rather_than_truncating() {
        assert_eq!(shown("1.006"), "1.01 EUR");
        assert_eq!(shown("1.004"), "1.00 EUR");
    }

    /// An exact half rounds to even, not up — `BigDecimal`'s behaviour, and the convention
    /// finance uses. Academic here: the column is `NUMERIC(19,2)`, so a third decimal never
    /// arrives from the server. Pinned so a future switch to half-up is a deliberate one.
    #[test]
    fn an_exact_half_rounds_to_even() {
        assert_eq!(shown("1.005"), "1.00 EUR");
        assert_eq!(shown("1.015"), "1.02 EUR");
    }
}
