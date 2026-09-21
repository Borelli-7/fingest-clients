use std::rc::Rc;

use fingest_client_ports::{AuthApi, ClientError, ClientEvent, EventBus, Session, SessionStore};
use fingest_contracts::{LoginRequest, RegisterRequest, TokenValidationResponse, UserDto};

/// Mirrors `Password::MIN_LEN` in `fingest-identity-core`.
///
/// Duplicated on purpose: it saves a round trip on an obviously bad password. The server
/// still enforces it — this is a courtesy, not a control.
pub const PASSWORD_MIN_LEN: usize = 8;

/// Everything that starts, restores or ends a session.
///
/// Holds ports, never adapters. The same instance is exercised by the browser and by the
/// host tests below, with different things plugged in behind the traits.
pub struct AuthUseCase {
    api: Rc<dyn AuthApi>,
    sessions: Rc<dyn SessionStore>,
    events: Rc<dyn EventBus>,
}

impl AuthUseCase {
    pub fn new(
        api: Rc<dyn AuthApi>,
        sessions: Rc<dyn SessionStore>,
        events: Rc<dyn EventBus>,
    ) -> Self {
        Self {
            api,
            sessions,
            events,
        }
    }

    /// Rehydrates a session left behind by a previous page load.
    ///
    /// Does not call the server: the first authenticated request will 401 if the token has
    /// expired, and the transport turns that into [`ClientEvent::SessionExpired`]. Verifying
    /// eagerly would add a blocking round trip to every cold start to learn the same thing.
    pub fn restore(&self) -> Option<Session> {
        let session = self.sessions.load()?;

        self.events.publish(ClientEvent::SessionStarted {
            login: session.user.login.clone(),
            admin: session.user.admin,
        });

        Some(session)
    }

    pub async fn login(&self, login: &str, password: &str) -> Result<Session, ClientError> {
        let (login, password) = (login.trim(), password);
        if login.is_empty() || password.is_empty() {
            return Err(ClientError::BadRequest(
                "Login and password are required".to_owned(),
            ));
        }

        let response = self
            .api
            .login(LoginRequest {
                login: login.to_owned(),
                password: password.to_owned(),
            })
            .await?;

        let session = Session {
            token: response.token,
            user: response.user,
        };
        self.sessions.save(&session);
        self.events.publish(ClientEvent::SessionStarted {
            login: session.user.login.clone(),
            admin: session.user.admin,
        });

        Ok(session)
    }

    pub async fn register(&self, request: RegisterRequest) -> Result<UserDto, ClientError> {
        if request.login.trim().is_empty() {
            return Err(ClientError::BadRequest("Login is required".to_owned()));
        }
        if request.password.chars().count() < PASSWORD_MIN_LEN {
            return Err(ClientError::BadRequest(format!(
                "Password must be at least {PASSWORD_MIN_LEN} characters"
            )));
        }

        let created = self.api.register(request).await?;
        self.events.publish(ClientEvent::AccountRegistered {
            login: created.login.clone(),
        });

        // Registration does not log you in: the server returns a UserDto, not a token.
        Ok(created)
    }

    pub fn logout(&self) {
        self.sessions.clear();
        self.events.publish(ClientEvent::SessionEnded);
    }

    /// Confirms the stored token is still accepted.
    ///
    /// A 401 here needs no special handling: the transport publishes
    /// [`ClientEvent::SessionExpired`] on the way out, and whoever subscribed to that owns
    /// the consequences.
    pub async fn verify(&self) -> Result<TokenValidationResponse, ClientError> {
        self.api.verify().await
    }

    /// Reaction to a token that stopped being accepted.
    ///
    /// Clears, but deliberately publishes nothing: the transport already announced the
    /// expiry, and re-publishing from the handler would loop.
    pub fn on_session_expired(&self) {
        self.sessions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{RecordingBus, StubAuthApi};
    use futures::executor::block_on;

    struct MemorySessions(std::cell::RefCell<Option<Session>>);

    impl MemorySessions {
        fn empty() -> Rc<Self> {
            Rc::new(Self(std::cell::RefCell::new(None)))
        }
    }

    impl SessionStore for MemorySessions {
        fn load(&self) -> Option<Session> {
            self.0.borrow().clone()
        }
        fn save(&self, session: &Session) {
            *self.0.borrow_mut() = Some(session.clone());
        }
        fn clear(&self) {
            *self.0.borrow_mut() = None;
        }
    }

    fn user(login: &str, admin: bool) -> UserDto {
        UserDto {
            login: login.into(),
            first_name: None,
            last_name: None,
            admin,
        }
    }

    struct Fixture {
        use_case: AuthUseCase,
        sessions: Rc<MemorySessions>,
        events: Rc<RecordingBus>,
    }

    fn fixture(api: Rc<StubAuthApi>) -> Fixture {
        let sessions = MemorySessions::empty();
        let events = Rc::new(RecordingBus::default());
        Fixture {
            use_case: AuthUseCase::new(api, sessions.clone(), events.clone()),
            sessions,
            events,
        }
    }

    #[test]
    fn a_successful_login_stores_the_session_and_announces_it() {
        let api = StubAuthApi::returning_login("token-abc", user("bob", false));
        let f = fixture(api);

        let session = block_on(f.use_case.login("bob", "secret123")).unwrap();

        assert_eq!(session.token, "token-abc");
        assert_eq!(f.sessions.load().unwrap().token, "token-abc");
        assert_eq!(
            f.events.recorded(),
            vec![ClientEvent::SessionStarted {
                login: "bob".into(),
                admin: false
            }]
        );
    }

    #[test]
    fn a_failed_login_leaves_no_session_behind() {
        let api = StubAuthApi::failing(ClientError::Unauthenticated("Invalid credentials".into()));
        let f = fixture(api);

        let err = block_on(f.use_case.login("bob", "wrong")).unwrap_err();

        assert_eq!(err.message(), "Invalid credentials");
        assert!(f.sessions.load().is_none());
        assert!(f.events.recorded().is_empty());
    }

    #[test]
    fn empty_credentials_never_reach_the_network() {
        let api = StubAuthApi::returning_login("token", user("bob", false));
        let f = fixture(api.clone());

        assert!(block_on(f.use_case.login("", "secret123")).is_err());
        assert!(block_on(f.use_case.login("bob", "")).is_err());
        assert_eq!(api.calls(), 0);
    }

    #[test]
    fn a_whitespace_only_login_is_rejected_like_an_empty_one() {
        let api = StubAuthApi::returning_login("token", user("bob", false));
        let f = fixture(api.clone());

        assert!(block_on(f.use_case.login("   ", "secret123")).is_err());
        assert_eq!(api.calls(), 0);
    }

    #[test]
    fn a_short_password_is_rejected_before_registering() {
        let api = StubAuthApi::returning_register(user("bob", false));
        let f = fixture(api.clone());

        let err = block_on(f.use_case.register(RegisterRequest {
            login: "bob".into(),
            first_name: None,
            last_name: None,
            password: "short".into(),
            admin: None,
        }))
        .unwrap_err();

        assert!(err.message().contains("8"));
        assert_eq!(api.calls(), 0);
    }

    #[test]
    fn registering_does_not_start_a_session() {
        let api = StubAuthApi::returning_register(user("bob", false));
        let f = fixture(api);

        block_on(f.use_case.register(RegisterRequest {
            login: "bob".into(),
            first_name: None,
            last_name: None,
            password: "secret123".into(),
            admin: None,
        }))
        .unwrap();

        assert!(f.sessions.load().is_none());
        assert_eq!(
            f.events.recorded(),
            vec![ClientEvent::AccountRegistered {
                login: "bob".into()
            }]
        );
    }

    #[test]
    fn restoring_a_stored_session_announces_it_without_calling_the_server() {
        let api = StubAuthApi::failing(ClientError::Network("must not be called".into()));
        let f = fixture(api.clone());
        f.sessions.save(&Session {
            token: "token-abc".into(),
            user: user("root", true),
        });

        let restored = f.use_case.restore().unwrap();

        assert!(restored.is_admin());
        assert_eq!(api.calls(), 0);
        assert_eq!(
            f.events.recorded(),
            vec![ClientEvent::SessionStarted {
                login: "root".into(),
                admin: true
            }]
        );
    }

    #[test]
    fn restoring_with_nothing_stored_is_not_an_error() {
        let f = fixture(StubAuthApi::failing(ClientError::Network("x".into())));

        assert!(f.use_case.restore().is_none());
        assert!(f.events.recorded().is_empty());
    }

    #[test]
    fn logging_out_and_expiring_both_clear_but_say_different_things() {
        let session = Session {
            token: "t".into(),
            user: user("bob", false),
        };

        let f = fixture(StubAuthApi::failing(ClientError::Network("x".into())));
        f.sessions.save(&session);
        f.use_case.logout();
        assert!(f.sessions.load().is_none());
        assert_eq!(f.events.recorded(), vec![ClientEvent::SessionEnded]);

        let f = fixture(StubAuthApi::failing(ClientError::Network("x".into())));
        f.sessions.save(&session);
        f.use_case.on_session_expired();
        assert!(f.sessions.load().is_none());
        assert!(
            f.events.recorded().is_empty(),
            "the transport already announced it; re-publishing would loop"
        );
    }
}
