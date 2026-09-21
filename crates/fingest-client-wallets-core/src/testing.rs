//! In-memory port double for wallets.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use fingest_client_ports::{ClientError, WalletsApi};
use fingest_contracts::{
    ExpenseDto, ExpenseInputRequest, SummaryDto, UpdateExpenseRequest, UpdateWalletRequest,
    WalletDto,
};
use fingest_kernel::{CategoryRef, Currency, DateRange, Money};

/// Succeeds or fails uniformly, and remembers the last patch it was handed.
///
/// The call count is what proves a validation rule short-circuited before the network.
pub struct StubWalletsApi {
    outcome: Result<(), ClientError>,
    calls: Cell<usize>,
    last_wallet_patch: RefCell<Option<UpdateWalletRequest>>,
    last_expense: RefCell<Option<ExpenseInputRequest>>,
}

impl StubWalletsApi {
    pub fn ok() -> Rc<Self> {
        Rc::new(Self {
            outcome: Ok(()),
            calls: Cell::new(0),
            last_wallet_patch: RefCell::new(None),
            last_expense: RefCell::new(None),
        })
    }

    pub fn failing(error: ClientError) -> Rc<Self> {
        Rc::new(Self {
            outcome: Err(error),
            calls: Cell::new(0),
            last_wallet_patch: RefCell::new(None),
            last_expense: RefCell::new(None),
        })
    }

    pub fn calls(&self) -> usize {
        self.calls.get()
    }

    pub fn last_wallet_patch(&self) -> Option<UpdateWalletRequest> {
        self.last_wallet_patch.borrow().clone()
    }

    pub fn last_expense(&self) -> Option<ExpenseInputRequest> {
        self.last_expense.borrow().clone()
    }

    fn record(&self) -> Result<(), ClientError> {
        self.calls.set(self.calls.get() + 1);
        self.outcome.clone()
    }
}

fn money(amount: i64) -> Money {
    Money::new(BigDecimal::from(amount), Currency::default())
}

fn wallet() -> WalletDto {
    WalletDto {
        id: Some(1),
        name: "Stub".into(),
        amount: money(0),
    }
}

fn expense() -> ExpenseDto {
    ExpenseDto {
        id: Some(1),
        amount: money(0),
        date: NaiveDate::from_ymd_opt(2026, 1, 1).expect("a valid date"),
        description: "Stub".into(),
        category: CategoryRef {
            name: "Food".into(),
            profit: false,
        },
    }
}

#[async_trait(?Send)]
impl WalletsApi for StubWalletsApi {
    async fn list(&self, _: &str) -> Result<Vec<WalletDto>, ClientError> {
        self.record().map(|()| vec![wallet()])
    }

    async fn create(&self, _: &str, _: WalletDto) -> Result<WalletDto, ClientError> {
        self.record().map(|()| wallet())
    }

    async fn update(
        &self,
        _: &str,
        _: i32,
        patch: UpdateWalletRequest,
    ) -> Result<WalletDto, ClientError> {
        let outcome = self.record();
        *self.last_wallet_patch.borrow_mut() = Some(patch);
        outcome.map(|()| wallet())
    }

    async fn delete(&self, _: &str, _: i32) -> Result<(), ClientError> {
        self.record()
    }

    async fn summary(&self, _: &str, _: i32, _: &DateRange) -> Result<SummaryDto, ClientError> {
        self.record().map(|()| SummaryDto {
            wallet_name: "Stub".into(),
            balance: money(0),
            expense_categories: HashMap::new(),
            income_categories: HashMap::new(),
            total_expense: money(0),
            total_income: money(0),
        })
    }

    async fn highest_expense(
        &self,
        _: &str,
        _: i32,
        _: &DateRange,
    ) -> Result<Option<ExpenseDto>, ClientError> {
        self.record().map(|()| Some(expense()))
    }

    async fn counted_categories(
        &self,
        _: &str,
        _: i32,
        _: &DateRange,
    ) -> Result<HashMap<String, i64>, ClientError> {
        self.record().map(|()| HashMap::new())
    }

    async fn list_expenses(
        &self,
        _: &str,
        _: i32,
        _: &DateRange,
    ) -> Result<Vec<ExpenseDto>, ClientError> {
        self.record().map(|()| vec![expense()])
    }

    async fn create_expense(
        &self,
        _: &str,
        _: i32,
        input: ExpenseInputRequest,
    ) -> Result<ExpenseDto, ClientError> {
        let outcome = self.record();
        *self.last_expense.borrow_mut() = Some(input);
        outcome.map(|()| expense())
    }

    async fn update_expense(
        &self,
        _: &str,
        _: i32,
        _: i32,
        _: UpdateExpenseRequest,
    ) -> Result<ExpenseDto, ClientError> {
        self.record().map(|()| expense())
    }

    async fn delete_expense(&self, _: &str, _: i32, _: i32) -> Result<(), ClientError> {
        self.record()
    }
}
