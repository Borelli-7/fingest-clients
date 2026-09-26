//! Where a budget is heading, from what has been spent so far.
//!
//! Computed here rather than on the server because it is presentation, not money: `spent`
//! and `total` still come from the API, and nothing derived here is ever sent back.

use bigdecimal::{BigDecimal, RoundingMode, Signed};
use chrono::NaiveDate;
use fingest_contracts::BudgetOutputDto;
use fingest_kernel::{DateRange, Money};

/// Minor units, matching the `NUMERIC(19,2)` column every amount comes from.
const SCALE: i64 = 2;

/// A budget's spending extended at its current daily pace to the last day of its period.
#[derive(Debug, Clone)]
pub struct Projection {
    /// Days of the period that have started, today included.
    pub days_elapsed: i64,
    pub days_total: i64,
    /// Spending by the period's last day if the pace so far holds.
    pub projected_spent: Money,
    /// `total - projected_spent`; negative when the budget is on course to be overspent.
    pub projected_left: Money,
}

impl Projection {
    pub fn overspends(&self) -> bool {
        self.projected_left.amount.is_negative()
    }
}

/// `None` when there is nothing to extrapolate: an open-ended period, one that has not
/// started or has already ended, or amounts in different currencies.
pub fn project(budget: &BudgetOutputDto, today: NaiveDate) -> Option<Projection> {
    let range = &budget.date_range;
    // An absent bound is stored as a sentinel date, and a pace over millennia is noise.
    if range.start == DateRange::min_date() || range.end == DateRange::max_date() {
        return None;
    }
    if !range.contains_date(today) {
        return None;
    }

    let days_total = (range.end - range.start).num_days() + 1;
    let days_elapsed = (today - range.start).num_days() + 1;

    let projected = (&budget.spent.amount * BigDecimal::from(days_total)
        / BigDecimal::from(days_elapsed))
    .with_scale_round(SCALE, RoundingMode::HalfEven);
    let projected_spent = Money::new(projected, budget.spent.currency.clone());
    let projected_left = budget.total.sub(&projected_spent).ok()?;

    Some(Projection {
        days_elapsed,
        days_total,
        projected_spent,
        projected_left,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fingest_kernel::{CategoryRef, Currency};
    use std::str::FromStr;

    fn money(amount: &str) -> Money {
        Money::new(BigDecimal::from_str(amount).unwrap(), Currency::default())
    }

    fn day(n: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, n).expect("a September date")
    }

    /// 1–30 September: a 30-day period.
    fn budget(total: &str, spent: &str) -> BudgetOutputDto {
        BudgetOutputDto {
            id: Some(1),
            category: CategoryRef {
                name: "Food".into(),
                profit: false,
            },
            total: money(total),
            date_range: DateRange::new(Some(day(1)), Some(day(30))),
            spent: money(spent),
            left: money(total).sub(&money(spent)).unwrap(),
        }
    }

    fn amount(value: &Money) -> String {
        value.amount.to_string()
    }

    #[test]
    fn halfway_through_the_pace_doubles() {
        let p = project(&budget("500", "150"), day(15)).unwrap();

        assert_eq!((p.days_elapsed, p.days_total), (15, 30));
        assert_eq!(amount(&p.projected_spent), "300.00");
        assert_eq!(amount(&p.projected_left), "200.00");
        assert!(!p.overspends());
    }

    #[test]
    fn a_fast_pace_is_flagged_before_the_money_runs_out() {
        let p = project(&budget("500", "400"), day(15)).unwrap();

        assert_eq!(amount(&p.projected_spent), "800.00");
        assert_eq!(amount(&p.projected_left), "-300.00");
        assert!(p.overspends());
    }

    /// On the last day there is nothing left to extrapolate.
    #[test]
    fn on_the_last_day_the_projection_is_what_was_spent() {
        let p = project(&budget("500", "123.45"), day(30)).unwrap();

        assert_eq!(amount(&p.projected_spent), "123.45");
    }

    /// Today counts as a day, so the first day does not divide by zero.
    #[test]
    fn the_first_day_counts_as_elapsed() {
        let p = project(&budget("500", "10"), day(1)).unwrap();

        assert_eq!(p.days_elapsed, 1);
        assert_eq!(amount(&p.projected_spent), "300.00");
    }

    #[test]
    fn a_repeating_fraction_rounds_to_minor_units() {
        let p = project(&budget("500", "100"), day(7)).unwrap();

        assert_eq!(amount(&p.projected_spent), "428.57");
    }

    #[test]
    fn a_period_that_has_not_started_or_has_ended_is_not_projected() {
        let not_started = BudgetOutputDto {
            date_range: DateRange::new(Some(day(10)), Some(day(20))),
            ..budget("500", "0")
        };

        assert!(project(&not_started, day(9)).is_none());
        assert!(project(&not_started, day(21)).is_none());
    }

    #[test]
    fn an_open_ended_period_is_not_projected() {
        let open = BudgetOutputDto {
            date_range: DateRange::new(Some(day(1)), None),
            ..budget("500", "100")
        };

        assert!(project(&open, day(15)).is_none());
    }

    #[test]
    fn mismatched_currencies_are_not_projected() {
        let mixed = BudgetOutputDto {
            total: Money::new(BigDecimal::from(500), Currency::new("USD").unwrap()),
            ..budget("500", "100")
        };

        assert!(project(&mixed, day(15)).is_none());
    }
}
