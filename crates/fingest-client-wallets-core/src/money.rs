//! Money entered by a human.
//!
//! The browser hands over strings. This turns one into a [`Money`] using the *same*
//! `fingest_kernel` types the server validates with, so a malformed currency or a negative
//! amount is caught in the form rather than at the far end of a round trip.
//!
//! Reusing the kernel is the whole reason `fingest-contracts` is a path dependency instead
//! of a hand-copied set of structs: these rules cannot drift from the server's.

use bigdecimal::BigDecimal;
use fingest_client_ports::ClientError;
use fingest_kernel::{Currency, Money};

/// Parses an amount and a currency code into money the server will accept.
///
/// Currency follows deviation D9 — exactly three ASCII letters, uppercased. An empty code
/// falls back to the kernel default (PLN), matching what the server does with an omitted one.
pub fn parse_money(amount: &str, currency: &str) -> Result<Money, ClientError> {
    let amount = amount.trim();
    if amount.is_empty() {
        return Err(ClientError::BadRequest("An amount is required".to_owned()));
    }

    let parsed: BigDecimal = amount
        .parse()
        .map_err(|_| ClientError::BadRequest("The amount is invalid".to_owned()))?;

    let currency = currency.trim();
    let currency = if currency.is_empty() {
        Currency::default()
    } else {
        Currency::new(currency).map_err(|error| ClientError::BadRequest(error.to_string()))?
    };

    Ok(Money::new(parsed, currency))
}

/// Refuses a negative amount where the server refuses one.
///
/// A balance may legitimately go negative through spending; what is refused is *entering*
/// a negative figure, which is exactly where the server applies the same rule.
pub fn require_non_negative(money: &Money) -> Result<(), ClientError> {
    money
        .require_non_negative()
        .map_err(|error| ClientError::BadRequest(error.to_string()))
}

/// Deviation D8: v1 accepted an expense in a currency the wallet does not hold and silently
/// left the balance unchanged. The server now answers 400; this says so without asking.
pub fn require_same_currency(wallet: &Money, expense: &Money) -> Result<(), ClientError> {
    if wallet.currency != expense.currency {
        return Err(ClientError::BadRequest(format!(
            "Currency mismatch: expected {}, got {}",
            wallet.currency, expense.currency
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_amount_parses_with_the_default_currency() {
        let money = parse_money("12.34", "").unwrap();

        assert_eq!(money.currency.as_str(), "PLN");
        assert_eq!(money.amount.to_string(), "12.34");
    }

    /// The kernel uppercases, so the form need not.
    #[test]
    fn a_lowercase_code_is_normalised() {
        assert_eq!(parse_money("1", "usd").unwrap().currency.as_str(), "USD");
    }

    #[test]
    fn a_code_that_is_not_three_letters_is_refused() {
        for code in ["US", "USDD", "US1", "12"] {
            assert!(
                parse_money("1", code).is_err(),
                "{code} should not be accepted"
            );
        }
    }

    #[test]
    fn a_non_numeric_amount_is_refused_with_v1s_wording() {
        let err = parse_money("abc", "USD").unwrap_err();

        assert_eq!(err.message(), "The amount is invalid");
    }

    #[test]
    fn an_empty_amount_is_refused() {
        assert!(parse_money("   ", "USD").is_err());
    }

    /// `BigDecimal` end to end: an `f64` round trip would not survive this.
    #[test]
    fn precision_is_not_lost() {
        let money = parse_money("12345678901234.56", "USD").unwrap();

        assert_eq!(money.amount.to_string(), "12345678901234.56");
    }

    #[test]
    fn a_negative_amount_is_refused_at_input() {
        let money = parse_money("-1.00", "USD").unwrap();

        assert!(require_non_negative(&money).is_err());
        assert!(require_non_negative(&parse_money("0", "USD").unwrap()).is_ok());
    }

    #[test]
    fn spending_in_another_currency_is_refused_before_the_round_trip() {
        let wallet = parse_money("100", "USD").unwrap();
        let expense = parse_money("10", "EUR").unwrap();

        let err = require_same_currency(&wallet, &expense).unwrap_err();

        assert!(err.message().contains("expected USD"));
        assert!(err.message().contains("got EUR"));
    }

    #[test]
    fn matching_currencies_pass() {
        let wallet = parse_money("100", "USD").unwrap();
        let expense = parse_money("10", "usd").unwrap();

        assert!(require_same_currency(&wallet, &expense).is_ok());
    }
}
