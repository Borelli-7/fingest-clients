//! In-memory port double for planning.

use std::cell::Cell;
use std::rc::Rc;

use async_trait::async_trait;
use fingest_client_ports::{BudgetFilter, ClientError, PlanningApi};
use fingest_contracts::{BudgetDto, BudgetInputRequest, BudgetOutputDto, UpdateBudgetRequest};
use fingest_kernel::{CategoryRef, Currency, DateRange, Money};

/// Succeeds or fails uniformly, counting calls so a test can prove a rule short-circuited
/// before the network.
pub struct StubPlanningApi {
    outcome: Result<(), ClientError>,
    calls: Cell<usize>,
}

impl StubPlanningApi {
    pub fn ok() -> Rc<Self> {
        Rc::new(Self {
            outcome: Ok(()),
            calls: Cell::new(0),
        })
    }

    pub fn failing(error: ClientError) -> Rc<Self> {
        Rc::new(Self {
            outcome: Err(error),
            calls: Cell::new(0),
        })
    }

    pub fn calls(&self) -> usize {
        self.calls.get()
    }

    fn record(&self) -> Result<(), ClientError> {
        self.calls.set(self.calls.get() + 1);
        self.outcome.clone()
    }
}

fn money() -> Money {
    Money::new(bigdecimal_zero(), Currency::default())
}

fn bigdecimal_zero() -> bigdecimal::BigDecimal {
    bigdecimal::BigDecimal::from(0)
}

fn budget() -> BudgetDto {
    BudgetDto {
        id: Some(1),
        category: CategoryRef {
            name: "Food".into(),
            profit: false,
        },
        total: money(),
        date_range: DateRange::new(None, None),
    }
}

#[async_trait(?Send)]
impl PlanningApi for StubPlanningApi {
    async fn list(&self, _: &str, _: &BudgetFilter) -> Result<Vec<BudgetOutputDto>, ClientError> {
        self.record().map(|()| {
            vec![BudgetOutputDto {
                id: Some(1),
                category: budget().category,
                total: money(),
                date_range: DateRange::new(None, None),
                spent: money(),
                left: money(),
            }]
        })
    }

    async fn create(&self, _: &str, _: BudgetInputRequest) -> Result<BudgetDto, ClientError> {
        self.record().map(|()| budget())
    }

    async fn update(
        &self,
        _: &str,
        _: i32,
        _: UpdateBudgetRequest,
    ) -> Result<BudgetDto, ClientError> {
        self.record().map(|()| budget())
    }

    async fn delete(&self, _: &str, _: i32) -> Result<(), ClientError> {
        self.record()
    }
}
