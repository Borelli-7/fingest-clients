//! In-memory port doubles for identity.
//!
//! Generic doubles — the bus and the session store — live in `fingest_client_ports::testing`
//! and are re-exported here so a test needs one import. Only stubs for this context's own
//! API ports are defined here.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use fingest_client_ports::{AuthApi, CapabilityApi, ClientError, NameField, UsersApi};
use fingest_contracts::{
    CapabilitiesDto, LoginRequest, LoginResponse, RegisterRequest, TokenValidationResponse, UserDto,
};

pub use fingest_client_ports::testing::{InMemorySessionStore, RecordingBus};

enum AuthOutcome {
    Login(LoginResponse),
    Register(UserDto),
    Fail(ClientError),
}

/// An [`AuthApi`] with a fixed answer, which counts how often it was asked.
///
/// The count is what proves a validation rule short-circuited before the network.
pub struct StubAuthApi {
    outcome: AuthOutcome,
    calls: Cell<usize>,
}

impl StubAuthApi {
    pub fn returning_login(token: &str, user: UserDto) -> Rc<Self> {
        Rc::new(Self {
            outcome: AuthOutcome::Login(LoginResponse {
                token: token.to_owned(),
                user,
            }),
            calls: Cell::new(0),
        })
    }

    pub fn returning_register(user: UserDto) -> Rc<Self> {
        Rc::new(Self {
            outcome: AuthOutcome::Register(user),
            calls: Cell::new(0),
        })
    }

    pub fn failing(error: ClientError) -> Rc<Self> {
        Rc::new(Self {
            outcome: AuthOutcome::Fail(error),
            calls: Cell::new(0),
        })
    }

    pub fn calls(&self) -> usize {
        self.calls.get()
    }

    fn record(&self) {
        self.calls.set(self.calls.get() + 1);
    }
}

#[async_trait(?Send)]
impl AuthApi for StubAuthApi {
    async fn register(&self, _: RegisterRequest) -> Result<UserDto, ClientError> {
        self.record();
        match &self.outcome {
            AuthOutcome::Register(user) => Ok(user.clone()),
            AuthOutcome::Fail(err) => Err(err.clone()),
            AuthOutcome::Login(response) => Ok(response.user.clone()),
        }
    }

    async fn login(&self, _: LoginRequest) -> Result<LoginResponse, ClientError> {
        self.record();
        match &self.outcome {
            AuthOutcome::Login(response) => Ok(response.clone()),
            AuthOutcome::Fail(err) => Err(err.clone()),
            AuthOutcome::Register(_) => Err(ClientError::Server("stub has no login".into())),
        }
    }

    async fn verify(&self) -> Result<TokenValidationResponse, ClientError> {
        self.record();
        match &self.outcome {
            AuthOutcome::Login(response) => Ok(TokenValidationResponse {
                valid: true,
                login: response.user.login.clone(),
                admin: response.user.admin,
            }),
            AuthOutcome::Fail(err) => Err(err.clone()),
            AuthOutcome::Register(user) => Ok(TokenValidationResponse {
                valid: true,
                login: user.login.clone(),
                admin: user.admin,
            }),
        }
    }
}

/// A [`CapabilityApi`] that answers with a fixed report, or fails.
pub struct StubCapabilityApi(Result<CapabilitiesDto, ClientError>);

impl StubCapabilityApi {
    pub fn returning(dto: CapabilitiesDto) -> Rc<Self> {
        Rc::new(Self(Ok(dto)))
    }

    pub fn failing(error: ClientError) -> Rc<Self> {
        Rc::new(Self(Err(error)))
    }
}

#[async_trait(?Send)]
impl CapabilityApi for StubCapabilityApi {
    async fn fetch(&self) -> Result<CapabilitiesDto, ClientError> {
        self.0.clone()
    }
}

/// A [`UsersApi`] with a fixed roster, remembering the last edit it was handed.
pub struct StubUsersApi {
    outcome: Result<Vec<UserDto>, ClientError>,
    calls: Cell<usize>,
    last_edit: RefCell<Option<(String, NameField, String)>>,
}

impl StubUsersApi {
    pub fn returning(users: Vec<UserDto>) -> Rc<Self> {
        Rc::new(Self {
            outcome: Ok(users),
            calls: Cell::new(0),
            last_edit: RefCell::new(None),
        })
    }

    pub fn failing(error: ClientError) -> Rc<Self> {
        Rc::new(Self {
            outcome: Err(error),
            calls: Cell::new(0),
            last_edit: RefCell::new(None),
        })
    }

    pub fn calls(&self) -> usize {
        self.calls.get()
    }

    pub fn last_edit(&self) -> Option<(String, NameField, String)> {
        self.last_edit.borrow().clone()
    }

    fn record(&self) {
        self.calls.set(self.calls.get() + 1);
    }
}

#[async_trait(?Send)]
impl UsersApi for StubUsersApi {
    async fn list(&self, _: &str) -> Result<Vec<UserDto>, ClientError> {
        self.record();
        self.outcome.clone()
    }

    async fn update_name(
        &self,
        login: &str,
        field: NameField,
        value: &str,
    ) -> Result<(), ClientError> {
        self.record();
        *self.last_edit.borrow_mut() = Some((login.to_owned(), field, value.to_owned()));
        self.outcome.clone().map(|_| ())
    }

    async fn delete(&self, _: &str) -> Result<(), ClientError> {
        self.record();
        self.outcome.clone().map(|_| ())
    }
}
