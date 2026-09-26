//! The transport.
//!
//! One type implements every API port, because they all share the same base URL, the same
//! bearer token and the same error mapping. Splitting per context would duplicate all three.

use std::rc::Rc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use async_trait::async_trait;
use fingest_client_ports::{
    AuthApi, BudgetFilter, CapabilityApi, CatalogApi, ClientError, ClientEvent, EventBus,
    NameField, PlanningApi, TokenSource, UsersApi, WalletsApi,
};
use fingest_contracts::{
    BudgetDto, BudgetInputRequest, BudgetOutputDto, CapabilitiesDto, CategoryDto,
    CreateCategoryRequest, ExpenseDto, ExpenseInputRequest, LoginRequest, LoginResponse,
    RegisterRequest, SummaryDto, TokenValidationResponse, UpdateBudgetRequest,
    UpdateCategoryRequest, UpdateExpenseRequest, UpdateWalletRequest, UserDto, WalletDto,
};
use fingest_kernel::DateRange;
use serde::de::DeserializeOwned;

use crate::{response, url::segment};

/// The server reads `start` and `end` from the query string and widens an absent bound to a
/// sentinel. Sending both explicitly keeps "no filter" and "full range" the same request.
fn range_query(range: &DateRange) -> [(&'static str, String); 2] {
    [
        ("start", range.start.to_string()),
        ("end", range.end.to_string()),
    ]
}

/// Whether a request carries the bearer token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Auth {
    Anonymous,
    Bearer,
}

/// Without limits a stalled connection on a phone never resolves, so the screen and its
/// form wait forever.
#[cfg(not(target_arch = "wasm32"))]
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(not(target_arch = "wasm32"))]
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(not(target_arch = "wasm32"))]
fn http_client() -> reqwest::Client {
    native_client(CONNECT_TIMEOUT, REQUEST_TIMEOUT)
}

/// `reqwest` has no timeout knob over `fetch`; the browser applies its own limits.
#[cfg(target_arch = "wasm32")]
fn http_client() -> reqwest::Client {
    reqwest::Client::new()
}

#[cfg(not(target_arch = "wasm32"))]
fn native_client(connect: Duration, total: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(connect)
        .timeout(total)
        .build()
        .unwrap_or_else(|error| {
            tracing::error!(%error, "HTTP client could not be configured; requests have no timeout");
            reqwest::Client::new()
        })
}

pub struct ApiClient {
    base_url: String,
    http: reqwest::Client,
    tokens: Rc<dyn TokenSource>,
    events: Rc<dyn EventBus>,
}

impl ApiClient {
    pub fn new(
        base_url: impl Into<String>,
        tokens: Rc<dyn TokenSource>,
        events: Rc<dyn EventBus>,
    ) -> Self {
        Self::with_client(base_url, tokens, events, http_client())
    }

    fn with_client(
        base_url: impl Into<String>,
        tokens: Rc<dyn TokenSource>,
        events: Rc<dyn EventBus>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            // Trailing slashes would double up against the leading slash on every path.
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http,
            tokens,
            events,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    async fn send<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        auth: Auth,
    ) -> Result<T, ClientError> {
        let (request, token_was_sent) = match auth {
            Auth::Anonymous => (request, false),
            Auth::Bearer => {
                let token = self.tokens.token().ok_or_else(|| {
                    // Short-circuit rather than let the server answer 401: an anonymous call
                    // to a protected route would publish SessionExpired at someone who was
                    // never signed in.
                    ClientError::Unauthenticated("You are not signed in".to_owned())
                })?;
                (request.bearer_auth(token), true)
            }
        };

        let response = request.send().await.map_err(|error| {
            // Detail stays in the console; the user gets something actionable. Same reasoning
            // as deviation D10 on the server, one tier out.
            tracing::error!(%error, "request never completed");
            ClientError::Network("Could not reach the server".to_owned())
        })?;

        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();

        if (200..300).contains(&status) {
            return response::decode(&body);
        }

        if response::is_session_expiry(status, token_was_sent) {
            self.events.publish(ClientEvent::SessionExpired);
        }

        Err(response::error_from(status, &body))
    }
}

#[async_trait(?Send)]
impl AuthApi for ApiClient {
    async fn register(&self, request: RegisterRequest) -> Result<UserDto, ClientError> {
        self.send(
            self.http
                .post(self.url("/api/auth/register"))
                .json(&request),
            Auth::Anonymous,
        )
        .await
    }

    async fn login(&self, request: LoginRequest) -> Result<LoginResponse, ClientError> {
        self.send(
            self.http.post(self.url("/api/auth/login")).json(&request),
            Auth::Anonymous,
        )
        .await
    }

    async fn verify(&self) -> Result<TokenValidationResponse, ClientError> {
        self.send(self.http.get(self.url("/api/auth/verify")), Auth::Bearer)
            .await
    }
}

#[async_trait(?Send)]
impl CapabilityApi for ApiClient {
    /// Anonymous on purpose: the login screen needs to know what to render before there is
    /// a token. Degrading a 404 into "nothing enabled" is the caller's decision, not the
    /// transport's — this layer reports what happened.
    async fn fetch(&self) -> Result<CapabilitiesDto, ClientError> {
        self.send(
            self.http.get(self.url("/api/capabilities")),
            Auth::Anonymous,
        )
        .await
    }
}

#[async_trait(?Send)]
impl CatalogApi for ApiClient {
    /// Public reference data — the one `/resources/*` route that needs no token.
    async fn list(&self) -> Result<Vec<CategoryDto>, ClientError> {
        self.send(
            self.http.get(self.url("/resources/categories")),
            Auth::Anonymous,
        )
        .await
    }

    async fn create(&self, request: CreateCategoryRequest) -> Result<CategoryDto, ClientError> {
        self.send(
            self.http
                .post(self.url("/resources/categories"))
                .json(&request),
            Auth::Bearer,
        )
        .await
    }

    async fn rename(
        &self,
        name: &str,
        profit: bool,
        new_name: &str,
    ) -> Result<CategoryDto, ClientError> {
        let path = format!("/resources/categories/{}/{profit}", segment(name)?);

        self.send(
            self.http.put(self.url(&path)).json(&UpdateCategoryRequest {
                new_name: new_name.to_owned(),
            }),
            Auth::Bearer,
        )
        .await
    }

    async fn delete(&self, name: &str, profit: bool) -> Result<(), ClientError> {
        let path = format!("/resources/categories/{}/{profit}", segment(name)?);

        self.send(self.http.delete(self.url(&path)), Auth::Bearer)
            .await
    }
}

#[async_trait(?Send)]
impl UsersApi for ApiClient {
    async fn list(&self, login: &str) -> Result<Vec<UserDto>, ClientError> {
        let path = format!("/resources/users/{}", segment(login)?);

        self.send(self.http.get(self.url(&path)), Auth::Bearer)
            .await
    }

    async fn update_name(
        &self,
        login: &str,
        field: NameField,
        value: &str,
    ) -> Result<(), ClientError> {
        let path = format!("/resources/users/{}", segment(login)?);
        // The server reads the new value from the body key named after the query parameter.
        let body = serde_json::json!({ field.key(): value });

        self.send(
            self.http
                .put(self.url(&path))
                .query(&[("field", field.key())])
                .json(&body),
            Auth::Bearer,
        )
        .await
    }

    async fn delete(&self, login: &str) -> Result<(), ClientError> {
        let path = format!("/resources/users/{}", segment(login)?);

        self.send(self.http.delete(self.url(&path)), Auth::Bearer)
            .await
    }
}

impl ApiClient {
    fn wallets_path(login: &str, wallet_id: i32) -> Result<String, ClientError> {
        Ok(format!(
            "/resources/users/{}/wallets/{wallet_id}",
            segment(login)?
        ))
    }
}

#[async_trait(?Send)]
impl WalletsApi for ApiClient {
    async fn list(&self, login: &str) -> Result<Vec<WalletDto>, ClientError> {
        let path = format!("/resources/users/{}/wallets", segment(login)?);

        self.send(self.http.get(self.url(&path)), Auth::Bearer)
            .await
    }

    async fn create(&self, login: &str, wallet: WalletDto) -> Result<WalletDto, ClientError> {
        let path = format!("/resources/users/{}/wallets", segment(login)?);

        self.send(self.http.post(self.url(&path)).json(&wallet), Auth::Bearer)
            .await
    }

    async fn update(
        &self,
        login: &str,
        wallet_id: i32,
        patch: UpdateWalletRequest,
    ) -> Result<WalletDto, ClientError> {
        let path = Self::wallets_path(login, wallet_id)?;

        self.send(self.http.put(self.url(&path)).json(&patch), Auth::Bearer)
            .await
    }

    async fn delete(&self, login: &str, wallet_id: i32) -> Result<(), ClientError> {
        let path = Self::wallets_path(login, wallet_id)?;

        self.send(self.http.delete(self.url(&path)), Auth::Bearer)
            .await
    }

    async fn summary(
        &self,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<SummaryDto, ClientError> {
        let path = format!("{}/summary", Self::wallets_path(login, wallet_id)?);

        self.send(
            self.http.get(self.url(&path)).query(&range_query(range)),
            Auth::Bearer,
        )
        .await
    }

    async fn highest_expense(
        &self,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<Option<ExpenseDto>, ClientError> {
        let path = format!("{}/highest_expense", Self::wallets_path(login, wallet_id)?);

        // A bare `null` is a legitimate answer here, not an empty body.
        self.send(
            self.http.get(self.url(&path)).query(&range_query(range)),
            Auth::Bearer,
        )
        .await
    }

    async fn counted_categories(
        &self,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<std::collections::HashMap<String, i64>, ClientError> {
        let path = format!(
            "{}/counted_categories",
            Self::wallets_path(login, wallet_id)?
        );

        self.send(
            self.http.get(self.url(&path)).query(&range_query(range)),
            Auth::Bearer,
        )
        .await
    }

    async fn list_expenses(
        &self,
        login: &str,
        wallet_id: i32,
        range: &DateRange,
    ) -> Result<Vec<ExpenseDto>, ClientError> {
        let path = format!("{}/expenses", Self::wallets_path(login, wallet_id)?);

        self.send(
            self.http.get(self.url(&path)).query(&range_query(range)),
            Auth::Bearer,
        )
        .await
    }

    async fn create_expense(
        &self,
        login: &str,
        wallet_id: i32,
        expense: ExpenseInputRequest,
    ) -> Result<ExpenseDto, ClientError> {
        let path = format!("{}/expenses", Self::wallets_path(login, wallet_id)?);

        self.send(self.http.post(self.url(&path)).json(&expense), Auth::Bearer)
            .await
    }

    async fn update_expense(
        &self,
        login: &str,
        wallet_id: i32,
        expense_id: i32,
        patch: UpdateExpenseRequest,
    ) -> Result<ExpenseDto, ClientError> {
        let path = format!(
            "{}/expenses/{expense_id}",
            Self::wallets_path(login, wallet_id)?
        );

        self.send(self.http.put(self.url(&path)).json(&patch), Auth::Bearer)
            .await
    }

    async fn delete_expense(
        &self,
        login: &str,
        wallet_id: i32,
        expense_id: i32,
    ) -> Result<(), ClientError> {
        let path = format!(
            "{}/expenses/{expense_id}",
            Self::wallets_path(login, wallet_id)?
        );

        self.send(self.http.delete(self.url(&path)), Auth::Bearer)
            .await
    }
}

#[async_trait(?Send)]
impl PlanningApi for ApiClient {
    async fn list(
        &self,
        login: &str,
        filter: &BudgetFilter,
    ) -> Result<Vec<BudgetOutputDto>, ClientError> {
        let path = format!("/resources/users/{}/budgets", segment(login)?);
        // Two windows, not one: `start_*` bounds where a budget begins, `end_*` where it ends.
        let query = [
            ("start_min", filter.start.start.to_string()),
            ("start_max", filter.start.end.to_string()),
            ("end_min", filter.end.start.to_string()),
            ("end_max", filter.end.end.to_string()),
        ];

        self.send(self.http.get(self.url(&path)).query(&query), Auth::Bearer)
            .await
    }

    async fn create(
        &self,
        login: &str,
        request: BudgetInputRequest,
    ) -> Result<BudgetDto, ClientError> {
        let path = format!("/resources/users/{}/budgets", segment(login)?);

        self.send(self.http.post(self.url(&path)).json(&request), Auth::Bearer)
            .await
    }

    async fn update(
        &self,
        login: &str,
        budget_id: i32,
        patch: UpdateBudgetRequest,
    ) -> Result<BudgetDto, ClientError> {
        let path = format!("/resources/users/{}/budgets/{budget_id}", segment(login)?);

        self.send(self.http.put(self.url(&path)).json(&patch), Auth::Bearer)
            .await
    }

    async fn delete(&self, login: &str, budget_id: i32) -> Result<(), ClientError> {
        let path = format!("/resources/users/{}/budgets/{budget_id}", segment(login)?);

        self.send(self.http.delete(self.url(&path)), Auth::Bearer)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fingest_client_identity_core::testing::{InMemorySessionStore, RecordingBus};
    use fingest_client_ports::Session;

    struct FixedToken(Option<&'static str>);

    impl TokenSource for FixedToken {
        fn token(&self) -> Option<String> {
            self.0.map(str::to_owned)
        }
    }

    fn client(token: Option<&'static str>) -> (ApiClient, Rc<RecordingBus>) {
        let events = Rc::new(RecordingBus::default());
        (
            ApiClient::new(
                "http://localhost:8080",
                Rc::new(FixedToken(token)),
                events.clone(),
            ),
            events,
        )
    }

    #[test]
    fn paths_join_cleanly_whether_or_not_the_base_url_has_a_slash() {
        let (with, _) = client(None);
        assert_eq!(
            with.url("/api/auth/login"),
            "http://localhost:8080/api/auth/login"
        );

        let events = Rc::new(RecordingBus::default());
        let trailing = ApiClient::new("http://localhost:8080/", Rc::new(FixedToken(None)), events);
        assert_eq!(
            trailing.url("/api/auth/login"),
            "http://localhost:8080/api/auth/login"
        );
    }

    #[test]
    fn a_protected_call_without_a_token_never_leaves_the_browser() {
        let (client, events) = client(None);

        let result = futures::executor::block_on(client.verify());

        assert!(matches!(result, Err(ClientError::Unauthenticated(_))));
        assert!(
            events.recorded().is_empty(),
            "nobody was signed in, so nothing expired"
        );
    }

    /// A listener that accepts and never answers is what a dead Wi-Fi or a stale
    /// `adb reverse` looks like from the phone.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_server_that_never_answers_times_out_as_a_network_error() {
        use std::net::TcpListener;
        use std::time::Instant;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let held = listener.accept();
            std::thread::sleep(Duration::from_secs(5));
            drop(held);
        });

        let client = ApiClient::with_client(
            format!("http://{address}"),
            Rc::new(FixedToken(None)),
            Rc::new(RecordingBus::default()),
            native_client(Duration::from_millis(200), Duration::from_millis(300)),
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let started = Instant::now();
        let result = runtime.block_on(CapabilityApi::fetch(&client));

        assert!(matches!(result, Err(ClientError::Network(_))));
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "the request was not bounded"
        );
    }

    /// `/resources/users/../wallets` would be sent as `/resources/wallets`.
    #[test]
    fn a_dot_only_login_is_refused_before_any_request() {
        let (client, events) = client(Some("token"));

        let wallets = futures::executor::block_on(WalletsApi::list(&client, ".."));
        let delete = futures::executor::block_on(CatalogApi::delete(&client, ".", false));

        assert!(matches!(wallets, Err(ClientError::BadRequest(_))));
        assert!(matches!(delete, Err(ClientError::BadRequest(_))));
        assert!(events.recorded().is_empty());
    }

    #[test]
    fn the_token_comes_from_the_session_store() {
        let store = InMemorySessionStore::with(Session {
            token: "token-abc".into(),
            user: UserDto {
                login: "bob".into(),
                first_name: None,
                last_name: None,
                admin: false,
            },
        });

        assert_eq!(store.token().as_deref(), Some("token-abc"));
    }
}
