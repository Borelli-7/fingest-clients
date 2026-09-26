//! Wallets bounded context, client side.
//!
//! Wallets hold money in one currency; expenses move it. Every rule enforced here is one
//! the server also enforces — this layer exists to catch it in the form, not to be the
//! authority.

pub mod money;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

use std::collections::HashMap;
use std::rc::Rc;

use chrono::NaiveDate;
use fingest_client_ports::{ClientError, ClientEvent, EventBus, Session, WalletsApi};
use fingest_contracts::{
    ExpenseDto, ExpenseInputRequest, SummaryDto, UpdateExpenseRequest, UpdateWalletRequest,
    WalletDto,
};
use fingest_kernel::{CategoryRef, DateRange, Money};

pub use money::{parse_money, require_non_negative, require_same_currency};

/// What the form collects before an expense exists.
#[derive(Debug, Clone, PartialEq)]
pub struct NewExpense {
    pub amount: Money,
    pub date: NaiveDate,
    pub description: String,
    pub category: CategoryRef,
}

pub struct WalletUseCase {
    api: Rc<dyn WalletsApi>,
    events: Rc<dyn EventBus>,
}

impl WalletUseCase {
    pub fn new(api: Rc<dyn WalletsApi>, events: Rc<dyn EventBus>) -> Self {
        Self { api, events }
    }

    // --- wallets ---

    pub async fn list(
        &self,
        session: &Session,
        login: &str,
    ) -> Result<Vec<WalletDto>, ClientError> {
        authorise(session, login)?;
        self.api.list(login).await
    }

    /// One wallet by id. `Ok(None)` means the list loaded and the id is not in it, which a
    /// screen must show differently from a failed request.
    pub async fn find(
        &self,
        session: &Session,
        login: &str,
        wallet_id: i32,
    ) -> Result<Option<WalletDto>, ClientError> {
        let wallets = self.list(session, login).await?;
        Ok(wallets.into_iter().find(|w| w.id == Some(wallet_id)))
    }

    pub async fn create(
        &self,
        session: &Session,
        login: &str,
        name: &str,
        amount: Money,
    ) -> Result<WalletDto, ClientError> {
        authorise(session, login)?;
        let name = require_name(name)?;
        require_non_negative(&amount)?;

        let created = self
            .api
            .create(
                login,
                // The server takes a full WalletDto on create and assigns the id itself.
                WalletDto {
                    id: None,
                    name,
                    amount,
                },
            )
            .await?;
        self.events.publish(ClientEvent::WalletChanged);

        Ok(created)
    }

    pub async fn rename(
        &self,
        session: &Session,
        login: &str,
        wallet_id: i32,
        name: &str,
    ) -> Result<WalletDto, ClientError> {
        authorise(session, login)?;
        let name = require_name(name)?;

        let updated = self
            .api
            .update(
                login,
                wallet_id,
                UpdateWalletRequest {
                    name: Some(name),
                    // Left absent on purpose: a rename must not restate the balance, or a
                    // stale form value would overwrite spending that happened meanwhile.
                    amount: None,
                },
            )
            .await?;
        self.events.publish(ClientEvent::WalletChanged);

        Ok(updated)
    }

    pub async fn delete(
        &self,
        session: &Session,
        login: &str,
        wallet_id: i32,
    ) -> Result<(), ClientError> {
        authorise(session, login)?;

        self.api.delete(login, wallet_id).await?;
        self.events.publish(ClientEvent::WalletChanged);

        Ok(())
    }

    // --- reads ---

    pub async fn summary(
        &self,
        session: &Session,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<SummaryDto, ClientError> {
        authorise(session, login)?;
        self.api.summary(login, wallet_id, range).await
    }

    pub async fn list_expenses(
        &self,
        session: &Session,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<Vec<ExpenseDto>, ClientError> {
        authorise(session, login)?;
        self.api.list_expenses(login, wallet_id, range).await
    }

    pub async fn highest_expense(
        &self,
        session: &Session,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<Option<ExpenseDto>, ClientError> {
        authorise(session, login)?;
        self.api.highest_expense(login, wallet_id, range).await
    }

    pub async fn counted_categories(
        &self,
        session: &Session,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<HashMap<String, i64>, ClientError> {
        authorise(session, login)?;
        self.api.counted_categories(login, wallet_id, range).await
    }

    // --- expenses ---

    /// `wallet_balance` is passed in so the currency rule can be checked without a second
    /// fetch; the caller already has the wallet it is spending from.
    pub async fn record_expense(
        &self,
        session: &Session,
        login: &str,
        wallet_id: i32,
        wallet_balance: &Money,
        expense: NewExpense,
    ) -> Result<ExpenseDto, ClientError> {
        authorise(session, login)?;
        require_non_negative(&expense.amount)?;
        require_same_currency(wallet_balance, &expense.amount)?;

        let description = expense.description.trim().to_owned();
        if description.is_empty() {
            return Err(ClientError::BadRequest(
                "A description is required".to_owned(),
            ));
        }

        let recorded = self
            .api
            .create_expense(
                login,
                wallet_id,
                ExpenseInputRequest {
                    amount: expense.amount,
                    date: expense.date,
                    description,
                    category: expense.category,
                },
            )
            .await?;
        self.events
            .publish(ClientEvent::ExpenseChanged { wallet_id });

        Ok(recorded)
    }

    pub async fn update_expense(
        &self,
        session: &Session,
        login: &str,
        wallet_id: i32,
        expense_id: i32,
        patch: UpdateExpenseRequest,
    ) -> Result<ExpenseDto, ClientError> {
        authorise(session, login)?;
        if let Some(amount) = &patch.amount {
            require_non_negative(amount)?;
        }

        let updated = self
            .api
            .update_expense(login, wallet_id, expense_id, patch)
            .await?;
        self.events
            .publish(ClientEvent::ExpenseChanged { wallet_id });

        Ok(updated)
    }

    /// Removing an expense returns its amount to the balance — v1 removed the row and left
    /// the balance permanently wrong. The event is what makes the balance on screen follow.
    pub async fn delete_expense(
        &self,
        session: &Session,
        login: &str,
        wallet_id: i32,
        expense_id: i32,
    ) -> Result<(), ClientError> {
        authorise(session, login)?;

        self.api
            .delete_expense(login, wallet_id, expense_id)
            .await?;
        self.events
            .publish(ClientEvent::ExpenseChanged { wallet_id });

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

fn require_name(name: &str) -> Result<String, ClientError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ClientError::BadRequest(
            "A wallet name is required".to_owned(),
        ));
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::StubWalletsApi;
    use fingest_client_ports::testing::RecordingBus;
    use fingest_contracts::UserDto;
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

    fn use_case(api: Rc<StubWalletsApi>) -> (WalletUseCase, Rc<RecordingBus>) {
        let events = Rc::new(RecordingBus::default());
        (WalletUseCase::new(api, events.clone()), events)
    }

    fn category() -> CategoryRef {
        CategoryRef {
            name: "Food".into(),
            profit: false,
        }
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()
    }

    fn expense(amount: &str, currency: &str) -> NewExpense {
        NewExpense {
            amount: parse_money(amount, currency).unwrap(),
            date: today(),
            description: "Lunch".into(),
            category: category(),
        }
    }

    #[test]
    fn creating_a_wallet_announces_the_list_is_stale() {
        let api = StubWalletsApi::ok();
        let (wallets, events) = use_case(api);

        block_on(wallets.create(
            &session("bob", false),
            "bob",
            "Main",
            parse_money("100", "USD").unwrap(),
        ))
        .unwrap();

        assert_eq!(events.recorded(), vec![ClientEvent::WalletChanged]);
    }

    #[test]
    fn a_wallet_cannot_be_opened_with_a_negative_balance() {
        let api = StubWalletsApi::ok();
        let (wallets, _) = use_case(api.clone());

        let result = block_on(wallets.create(
            &session("bob", false),
            "bob",
            "Main",
            parse_money("-5", "USD").unwrap(),
        ));

        assert!(result.is_err());
        assert_eq!(api.calls(), 0);
    }

    #[test]
    fn a_blank_wallet_name_never_reaches_the_network() {
        let api = StubWalletsApi::ok();
        let (wallets, _) = use_case(api.clone());

        assert!(
            block_on(wallets.create(
                &session("bob", false),
                "bob",
                "   ",
                parse_money("1", "USD").unwrap()
            ))
            .is_err()
        );
        assert_eq!(api.calls(), 0);
    }

    #[test]
    fn acting_on_someone_elses_wallets_is_refused_before_the_network() {
        let api = StubWalletsApi::ok();
        let (wallets, _) = use_case(api.clone());

        let err = block_on(wallets.list(&session("bob", false), "alice")).unwrap_err();

        assert!(matches!(err, ClientError::Forbidden(_)));
        assert_eq!(api.calls(), 0);
    }

    #[test]
    fn finding_a_listed_wallet_returns_it() {
        let (wallets, _) = use_case(StubWalletsApi::ok());

        let found = block_on(wallets.find(&session("bob", false), "bob", 1)).unwrap();

        assert_eq!(found.and_then(|w| w.id), Some(1));
    }

    /// A deep link to a deleted wallet must read as "not found", not as an endless load.
    #[test]
    fn an_unknown_wallet_id_is_absent_rather_than_an_error() {
        let (wallets, _) = use_case(StubWalletsApi::ok());

        assert_eq!(
            block_on(wallets.find(&session("bob", false), "bob", 999)),
            Ok(None)
        );
    }

    #[test]
    fn a_failed_lookup_keeps_its_error() {
        let api = StubWalletsApi::failing(ClientError::Network("offline".into()));
        let (wallets, _) = use_case(api);

        let err = block_on(wallets.find(&session("bob", false), "bob", 1)).unwrap_err();

        assert!(matches!(err, ClientError::Network(_)));
    }

    #[test]
    fn an_admin_may_act_on_another_account() {
        let api = StubWalletsApi::ok();
        let (wallets, _) = use_case(api.clone());

        assert!(block_on(wallets.list(&session("root", true), "bob")).is_ok());
        assert_eq!(api.calls(), 1);
    }

    /// Deviation D8. v1 took this and silently left the balance untouched.
    #[test]
    fn spending_in_the_wrong_currency_is_refused_before_the_network() {
        let api = StubWalletsApi::ok();
        let (wallets, events) = use_case(api.clone());
        let balance = parse_money("100", "USD").unwrap();

        let err = block_on(wallets.record_expense(
            &session("bob", false),
            "bob",
            1,
            &balance,
            expense("10", "EUR"),
        ))
        .unwrap_err();

        assert!(err.message().contains("Currency mismatch"));
        assert_eq!(api.calls(), 0);
        assert!(events.recorded().is_empty());
    }

    #[test]
    fn a_matching_currency_is_recorded_and_announced() {
        let api = StubWalletsApi::ok();
        let (wallets, events) = use_case(api);
        let balance = parse_money("100", "USD").unwrap();

        block_on(wallets.record_expense(
            &session("bob", false),
            "bob",
            7,
            &balance,
            expense("10", "USD"),
        ))
        .unwrap();

        assert_eq!(
            events.recorded(),
            vec![ClientEvent::ExpenseChanged { wallet_id: 7 }]
        );
    }

    #[test]
    fn an_expense_needs_a_description() {
        let api = StubWalletsApi::ok();
        let (wallets, _) = use_case(api.clone());
        let balance = parse_money("100", "USD").unwrap();
        let mut blank = expense("10", "USD");
        blank.description = "   ".into();

        assert!(
            block_on(wallets.record_expense(&session("bob", false), "bob", 1, &balance, blank))
                .is_err()
        );
        assert_eq!(api.calls(), 0);
    }

    #[test]
    fn a_negative_expense_is_refused() {
        let api = StubWalletsApi::ok();
        let (wallets, _) = use_case(api.clone());
        let balance = parse_money("100", "USD").unwrap();

        assert!(
            block_on(wallets.record_expense(
                &session("bob", false),
                "bob",
                1,
                &balance,
                expense("-10", "USD")
            ))
            .is_err()
        );
        assert_eq!(api.calls(), 0);
    }

    /// The balance change is the server's job; the event is what makes the screen follow.
    #[test]
    fn deleting_an_expense_announces_the_wallet_is_stale() {
        let api = StubWalletsApi::ok();
        let (wallets, events) = use_case(api);

        block_on(wallets.delete_expense(&session("bob", false), "bob", 7, 42)).unwrap();

        assert_eq!(
            events.recorded(),
            vec![ClientEvent::ExpenseChanged { wallet_id: 7 }]
        );
    }

    #[test]
    fn a_failed_delete_announces_nothing() {
        let api = StubWalletsApi::failing(ClientError::NotFound("expense not found".into()));
        let (wallets, events) = use_case(api);

        assert!(block_on(wallets.delete_expense(&session("bob", false), "bob", 7, 42)).is_err());
        assert!(events.recorded().is_empty());
    }

    /// A rename must not carry the balance, or a stale form would undo spending.
    #[test]
    fn renaming_leaves_the_balance_alone() {
        let api = StubWalletsApi::ok();
        let (wallets, _) = use_case(api.clone());

        block_on(wallets.rename(&session("bob", false), "bob", 1, "Holiday")).unwrap();

        let patch = api.last_wallet_patch().unwrap();
        assert_eq!(patch.name.as_deref(), Some("Holiday"));
        assert!(patch.amount.is_none());
    }
}
