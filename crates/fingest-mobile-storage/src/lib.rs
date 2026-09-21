//! Mobile adapters.
//!
//! The web client keeps its token in `sessionStorage` and documents the XSS exposure as
//! accepted, because a browser leaves no better option. A phone does: there is no injected
//! script to read it, and the OS ships a hardware-backed secret store. That is the whole
//! reason this crate exists rather than reusing the web one.
//!
//! [`KeystoreSessionStore`] is the seam for it. The JNI/Keychain calls land in phase M6;
//! until then it composes over whatever backend it is given, and the in-memory backend
//! keeps the walking skeleton honest — a session that does not survive a restart is
//! obviously incomplete, whereas a plaintext file would quietly look finished.

use std::cell::RefCell;

use chrono::NaiveDate;
use fingest_client_ports::{Clock, Session, SessionStore, TokenSource};

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "android")]
pub use android::AndroidKeystore;

/// The best secret store this build can reach.
///
/// On Android that is the Keystore. Anywhere else — the browser used to develop the
/// screens, or a host test — it is memory, which loses the session on restart. It is never
/// a plaintext file: that would look finished while being the exposure this crate exists
/// to avoid.
#[cfg(target_os = "android")]
pub fn platform_secrets() -> AndroidKeystore {
    AndroidKeystore::new()
}

#[cfg(not(target_os = "android"))]
pub fn platform_secrets() -> InMemorySecretStore {
    InMemorySecretStore::default()
}

/// Where the encrypted blob actually lives.
///
/// Separated from [`KeystoreSessionStore`] so the platform call is the only thing that
/// changes per OS, and so the store's own logic stays testable on the host.
pub trait SecretStore {
    fn read(&self) -> Option<String>;
    fn write(&self, value: &str) -> Result<(), SecretError>;
    fn clear(&self);
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretError {
    /// The device is locked, the key was invalidated, or the OS refused.
    #[error("secure storage is unavailable: {0}")]
    Unavailable(String),
}

/// Holds nothing across a restart. Correct for tests, and the deliberate default until the
/// platform backends land.
#[derive(Default)]
pub struct InMemorySecretStore(RefCell<Option<String>>);

impl SecretStore for InMemorySecretStore {
    fn read(&self) -> Option<String> {
        self.0.borrow().clone()
    }

    fn write(&self, value: &str) -> Result<(), SecretError> {
        *self.0.borrow_mut() = Some(value.to_owned());
        Ok(())
    }

    fn clear(&self) {
        *self.0.borrow_mut() = None;
    }
}

/// The session, kept wherever the platform keeps secrets.
pub struct KeystoreSessionStore<S: SecretStore> {
    secrets: S,
    /// Reading a keystore entry can prompt or take a lock, so the decrypted value is held
    /// for the process lifetime rather than fetched on every request.
    cache: RefCell<Option<Session>>,
}

impl<S: SecretStore> KeystoreSessionStore<S> {
    pub fn new(secrets: S) -> Self {
        Self {
            secrets,
            cache: RefCell::new(None),
        }
    }
}

impl<S: SecretStore> SessionStore for KeystoreSessionStore<S> {
    fn load(&self) -> Option<Session> {
        if let Some(session) = self.cache.borrow().clone() {
            return Some(session);
        }

        let raw = self.secrets.read()?;
        match serde_json::from_str::<Session>(&raw) {
            Ok(session) => {
                *self.cache.borrow_mut() = Some(session.clone());
                Some(session)
            }
            Err(error) => {
                // A shape change or a corrupt entry: drop it rather than retry every load.
                tracing::debug!(%error, "stored session is not usable");
                self.secrets.clear();
                None
            }
        }
    }

    fn save(&self, session: &Session) {
        *self.cache.borrow_mut() = Some(session.clone());

        let Ok(raw) = serde_json::to_string(session) else {
            tracing::error!("session could not be serialised");
            return;
        };

        if let Err(error) = self.secrets.write(&raw) {
            // Fail *closed*, not down: the session stays usable for this run, and the user
            // signs in again next launch. Never fall back to unencrypted storage.
            tracing::warn!(%error, "session will not survive a restart");
        }
    }

    fn clear(&self) {
        *self.cache.borrow_mut() = None;
        self.secrets.clear();
    }
}

impl<S: SecretStore> TokenSource for KeystoreSessionStore<S> {
    fn token(&self) -> Option<String> {
        self.load().map(|session| session.token)
    }
}

/// The device's local date. Local rather than UTC: a date typed into a form means the
/// user's today.
pub struct DeviceClock;

impl Clock for DeviceClock {
    fn today(&self) -> NaiveDate {
        chrono::Local::now().date_naive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fingest_contracts::UserDto;

    fn session(token: &str) -> Session {
        Session {
            token: token.to_owned(),
            user: UserDto {
                login: "bob".into(),
                first_name: None,
                last_name: None,
                admin: false,
            },
        }
    }

    fn store() -> KeystoreSessionStore<InMemorySecretStore> {
        KeystoreSessionStore::new(InMemorySecretStore::default())
    }

    #[test]
    fn a_session_round_trips_through_the_secret_store() {
        let store = store();
        assert!(store.load().is_none());

        store.save(&session("token-abc"));

        assert_eq!(store.load(), Some(session("token-abc")));
        assert_eq!(store.token().as_deref(), Some("token-abc"));
    }

    #[test]
    fn clearing_removes_it_from_the_secret_store_too() {
        let store = store();
        store.save(&session("token-abc"));

        store.clear();

        assert!(store.load().is_none());
        assert!(store.secrets.read().is_none());
    }

    #[test]
    fn saving_twice_replaces_rather_than_accumulates() {
        let store = store();
        store.save(&session("first"));
        store.save(&session("second"));

        assert_eq!(store.load().unwrap().token, "second");
    }

    /// A shape change between releases must read as "signed out", not crash the launch.
    #[test]
    fn a_corrupt_entry_reads_as_logged_out_and_is_discarded() {
        let secrets = InMemorySecretStore::default();
        secrets.write("not a session").unwrap();
        let store = KeystoreSessionStore::new(secrets);

        assert!(store.load().is_none());
        assert!(store.secrets.read().is_none());
    }

    struct RefusingStore;

    impl SecretStore for RefusingStore {
        fn read(&self) -> Option<String> {
            None
        }
        fn write(&self, _: &str) -> Result<(), SecretError> {
            Err(SecretError::Unavailable("device locked".into()))
        }
        fn clear(&self) {}
    }

    /// The security property this crate exists for: an unavailable keystore costs the user
    /// a re-login, never a plaintext copy of the token.
    #[test]
    fn a_refusing_keystore_keeps_the_session_for_this_run_only() {
        let store = KeystoreSessionStore::new(RefusingStore);

        store.save(&session("token-abc"));

        assert_eq!(store.load().unwrap().token, "token-abc");
        // Nothing was persisted, so a fresh process starts signed out.
        assert!(KeystoreSessionStore::new(RefusingStore).load().is_none());
    }
}
