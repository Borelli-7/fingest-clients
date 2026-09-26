//! Planning bounded context, client side.
//!
//! A budget is a category, an amount and a period. The server computes `spent` and `left`
//! from the expenses that fall inside that period, which is why neither is ever calculated
//! here — deriving them client-side would be a second source of truth for money.

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub mod forecast;

pub use forecast::{Projection, project};

use std::rc::Rc;

use fingest_client_ports::{
    BudgetFilter, ClientError, ClientEvent, EventBus, PlanningApi, Session,
};
use fingest_contracts::{BudgetDto, BudgetInputRequest, BudgetOutputDto, UpdateBudgetRequest};
use fingest_kernel::{CategoryRef, DateRange, Money};

pub struct BudgetUseCase {
    api: Rc<dyn PlanningApi>,
    events: Rc<dyn EventBus>,
}

impl BudgetUseCase {
    pub fn new(api: Rc<dyn PlanningApi>, events: Rc<dyn EventBus>) -> Self {
        Self { api, events }
    }

    pub async fn list(
        &self,
        session: &Session,
        login: &str,
        filter: &BudgetFilter,
    ) -> Result<Vec<BudgetOutputDto>, ClientError> {
        authorise(session, login)?;
        self.api.list(login, filter).await
    }

    pub async fn create(
        &self,
        session: &Session,
        login: &str,
        category: CategoryRef,
        total: Money,
        period: DateRange,
    ) -> Result<BudgetDto, ClientError> {
        authorise(session, login)?;
        require_non_negative(&total)?;
        require_valid_period(&period)?;

        let created = self
            .api
            .create(
                login,
                BudgetInputRequest {
                    category,
                    total,
                    date_range: period,
                },
            )
            .await?;
        self.events.publish(ClientEvent::BudgetChanged);

        Ok(created)
    }

    pub async fn update(
        &self,
        session: &Session,
        login: &str,
        budget_id: i32,
        patch: UpdateBudgetRequest,
    ) -> Result<BudgetDto, ClientError> {
        authorise(session, login)?;
        if let Some(total) = &patch.total {
            require_non_negative(total)?;
        }
        if let Some(period) = &patch.date_range {
            require_valid_period(period)?;
        }

        let updated = self.api.update(login, budget_id, patch).await?;
        self.events.publish(ClientEvent::BudgetChanged);

        Ok(updated)
    }

    pub async fn delete(
        &self,
        session: &Session,
        login: &str,
        budget_id: i32,
    ) -> Result<(), ClientError> {
        authorise(session, login)?;

        self.api.delete(login, budget_id).await?;
        self.events.publish(ClientEvent::BudgetChanged);

        Ok(())
    }
}

/// Mirrors `require_self_or_admin` on the server, worded the same way.
fn authorise(session: &Session, login: &str) -> Result<(), ClientError> {
    if session.may_act_on(login) {
        return Ok(());
    }
    Err(ClientError::Forbidden(format!(
        "Not authorized. Path login '{login}' does not match authenticated user '{}'",
        session.login()
    )))
}

fn require_non_negative(total: &Money) -> Result<(), ClientError> {
    total
        .require_non_negative()
        .map_err(|error| ClientError::BadRequest(error.to_string()))
}

/// The kernel's own rule, so an inverted period is refused on the same terms the server
/// would refuse it.
fn require_valid_period(period: &DateRange) -> Result<(), ClientError> {
    if !period.is_valid() {
        return Err(ClientError::BadRequest(format!(
            "Invalid date range: start {} is after end {}",
            period.start, period.end
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::StubPlanningApi;
    use bigdecimal::BigDecimal;
    use chrono::NaiveDate;
    use fingest_client_ports::testing::RecordingBus;
    use fingest_contracts::UserDto;
    use fingest_kernel::Currency;
    use futures::executor::block_on;

    fn session(login: &str, admin: bool) -> Session {
        Session {
            token: "t".into(),
            user: UserDto {
                login: login.into(),
                first_name: None,
                last_name: None,
                admin,
            },
        }
    }

    fn use_case(api: Rc<StubPlanningApi>) -> (BudgetUseCase, Rc<RecordingBus>) {
        let events = Rc::new(RecordingBus::default());
        (BudgetUseCase::new(api, events.clone()), events)
    }

    fn money(amount: i64) -> Money {
        Money::new(BigDecimal::from(amount), Currency::default())
    }

    fn category() -> CategoryRef {
        CategoryRef {
            name: "Food".into(),
            profit: false,
        }
    }

    fn day(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, day).expect("a valid date")
    }

    fn period() -> DateRange {
        DateRange::new(Some(day(1)), Some(day(30)))
    }

    #[test]
    fn creating_announces_that_budgets_are_stale() {
        let api = StubPlanningApi::ok();
        let (budgets, events) = use_case(api);

        block_on(budgets.create(
            &session("bob", false),
            "bob",
            category(),
            money(500),
            period(),
        ))
        .unwrap();

        assert_eq!(events.recorded(), vec![ClientEvent::BudgetChanged]);
    }

    #[test]
    fn a_negative_total_never_reaches_the_network() {
        let api = StubPlanningApi::ok();
        let (budgets, events) = use_case(api.clone());

        let err = block_on(budgets.create(
            &session("bob", false),
            "bob",
            category(),
            money(-1),
            period(),
        ))
        .unwrap_err();

        assert!(matches!(err, ClientError::BadRequest(_)));
        assert_eq!(api.calls(), 0);
        assert!(events.recorded().is_empty());
    }

    #[test]
    fn an_inverted_period_never_reaches_the_network() {
        let api = StubPlanningApi::ok();
        let (budgets, _) = use_case(api.clone());

        let backwards = DateRange::new(Some(day(30)), Some(day(1)));
        let err = block_on(budgets.create(
            &session("bob", false),
            "bob",
            category(),
            money(500),
            backwards,
        ))
        .unwrap_err();

        assert!(err.message().contains("Invalid date range"));
        assert_eq!(api.calls(), 0);
    }

    #[test]
    fn budgets_belong_to_one_account() {
        let api = StubPlanningApi::ok();
        let (budgets, _) = use_case(api.clone());

        let err =
            block_on(budgets.list(&session("bob", false), "alice", &BudgetFilter::unfiltered()))
                .unwrap_err();

        assert!(matches!(err, ClientError::Forbidden(_)));
        assert_eq!(api.calls(), 0);
    }

    /// Deviation D4: v1 allowed only the owner here, refusing admins. v2 allows both.
    #[test]
    fn an_admin_may_read_another_accounts_budgets() {
        let api = StubPlanningApi::ok();
        let (budgets, _) = use_case(api.clone());

        assert!(
            block_on(budgets.list(&session("root", true), "bob", &BudgetFilter::unfiltered()))
                .is_ok()
        );
        assert_eq!(api.calls(), 1);
    }

    #[test]
    fn a_failed_delete_announces_nothing() {
        let api = StubPlanningApi::failing(ClientError::NotFound("budget not found".into()));
        let (budgets, events) = use_case(api);

        assert!(block_on(budgets.delete(&session("bob", false), "bob", 7)).is_err());
        assert!(events.recorded().is_empty());
    }

    /// Both windows wide open is how the server reads absent query parameters.
    #[test]
    fn the_default_filter_constrains_nothing() {
        let filter = BudgetFilter::unfiltered();

        assert_eq!(filter.start, DateRange::new(None, None));
        assert_eq!(filter.end, DateRange::new(None, None));
    }
}
