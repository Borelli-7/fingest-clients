use async_trait::async_trait;
use fingest_contracts::{
    BudgetDto, BudgetInputRequest, BudgetOutputDto, CapabilitiesDto, CategoryDto,
    CreateCategoryRequest, ExpenseDto, ExpenseInputRequest, LoginRequest, LoginResponse,
    RegisterRequest, SummaryDto, TokenValidationResponse, UpdateBudgetRequest,
    UpdateExpenseRequest, UpdateWalletRequest, UserDto, WalletDto,
};
use fingest_kernel::DateRange;

use crate::error::ClientError;

/// `?Send` throughout: wasm futures run on a single thread and are not `Send`. Requiring
/// `Send` here would make every adapter unimplementable in a browser.
#[async_trait(?Send)]
pub trait AuthApi {
    async fn register(&self, request: RegisterRequest) -> Result<UserDto, ClientError>;

    async fn login(&self, request: LoginRequest) -> Result<LoginResponse, ClientError>;

    /// Validates whatever token the transport is currently carrying.
    async fn verify(&self) -> Result<TokenValidationResponse, ClientError>;
}

/// Reads `GET /api/capabilities`.
#[async_trait(?Send)]
pub trait CapabilityApi {
    async fn fetch(&self) -> Result<CapabilitiesDto, ClientError>;
}

/// A category is keyed by `(name, profit)` on the server, so every lookup needs both.
#[async_trait(?Send)]
pub trait CatalogApi {
    async fn list(&self) -> Result<Vec<CategoryDto>, ClientError>;

    async fn create(&self, request: CreateCategoryRequest) -> Result<CategoryDto, ClientError>;

    async fn rename(
        &self,
        name: &str,
        profit: bool,
        new_name: &str,
    ) -> Result<CategoryDto, ClientError>;

    async fn delete(&self, name: &str, profit: bool) -> Result<(), ClientError>;
}

/// The two name fields an account exposes for editing.
///
/// Typed rather than stringly, mirroring `NameField` in `fingest-identity-core`: the UI
/// cannot hand the transport an arbitrary field name, so `Invalid field: …` becomes
/// unreachable from this client rather than merely unlikely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameField {
    First,
    Last,
}

impl NameField {
    /// The query parameter *and* the JSON body key — the server expects the same spelling
    /// for both.
    pub fn key(self) -> &'static str {
        match self {
            Self::First => "firstName",
            Self::Last => "lastName",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::First => "First name",
            Self::Last => "Last name",
        }
    }

    pub const ALL: [Self; 2] = [Self::First, Self::Last];
}

#[async_trait(?Send)]
pub trait UsersApi {
    /// Lists every account. The server takes a login in the path and then requires the
    /// caller be an admin, so this is admin-only however it is addressed.
    async fn list(&self, login: &str) -> Result<Vec<UserDto>, ClientError>;

    async fn update_name(
        &self,
        login: &str,
        field: NameField,
        value: &str,
    ) -> Result<(), ClientError>;

    async fn delete(&self, login: &str) -> Result<(), ClientError>;
}

/// Wallets and the expenses inside them.
///
/// Every method takes a `login` because the server scopes these routes by account and
/// checks self-or-admin on each one. Passing it explicitly keeps an admin able to act on
/// another account without the transport having to guess whose data is wanted.
#[async_trait(?Send)]
pub trait WalletsApi {
    async fn list(&self, login: &str) -> Result<Vec<WalletDto>, ClientError>;

    async fn create(&self, login: &str, wallet: WalletDto) -> Result<WalletDto, ClientError>;

    async fn update(
        &self,
        login: &str,
        wallet_id: i32,
        patch: UpdateWalletRequest,
    ) -> Result<WalletDto, ClientError>;

    async fn delete(&self, login: &str, wallet_id: i32) -> Result<(), ClientError>;

    async fn summary(
        &self,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<SummaryDto, ClientError>;

    /// `None` when the range holds no spending — the server answers a bare `null`.
    async fn highest_expense(
        &self,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<Option<ExpenseDto>, ClientError>;

    /// Number of entries per category, not a sum of amounts.
    async fn counted_categories(
        &self,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<std::collections::HashMap<String, i64>, ClientError>;

    async fn list_expenses(
        &self,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<Vec<ExpenseDto>, ClientError>;

    async fn create_expense(
        &self,
        login: &str,
        wallet_id: i32,
        expense: ExpenseInputRequest,
    ) -> Result<ExpenseDto, ClientError>;

    async fn update_expense(
        &self,
        login: &str,
        wallet_id: i32,
        expense_id: i32,
        patch: UpdateExpenseRequest,
    ) -> Result<ExpenseDto, ClientError>;

    async fn delete_expense(
        &self,
        login: &str,
        wallet_id: i32,
        expense_id: i32,
    ) -> Result<(), ClientError>;
}

/// Budgets are filtered by **two** independent windows, one constraining where a budget's
/// range starts and one where it ends — not the single `start`/`end` pair the wallet reads
/// use. The server spells them `start_min`/`start_max` and `end_min`/`end_max`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetFilter {
    pub start: DateRange,
    pub end: DateRange,
}

impl BudgetFilter {
    /// Both windows wide open, which is how the server reads absent parameters.
    pub fn unfiltered() -> Self {
        Self {
            start: DateRange::new(None, None),
            end: DateRange::new(None, None),
        }
    }
}

#[async_trait(?Send)]
pub trait PlanningApi {
    /// Returns `spent` and `left` alongside each budget, computed server-side.
    async fn list(
        &self,
        login: &str,
        filter: &BudgetFilter,
    ) -> Result<Vec<BudgetOutputDto>, ClientError>;

    async fn create(
        &self,
        login: &str,
        request: BudgetInputRequest,
    ) -> Result<BudgetDto, ClientError>;

    async fn update(
        &self,
        login: &str,
        budget_id: i32,
        patch: UpdateBudgetRequest,
    ) -> Result<BudgetDto, ClientError>;

    async fn delete(&self, login: &str, budget_id: i32) -> Result<(), ClientError>;
}
