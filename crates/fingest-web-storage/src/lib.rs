//! Session persistence.
//!
//! Uses `sessionStorage`, not `localStorage`. Both are readable by any script on the origin,
//! so neither is safe against XSS — but `sessionStorage` dies with the tab, which bounds how
//! long a stolen token stays useful. The structurally safe option, an httpOnly cookie, is
//! not available: `POST /api/auth/login` returns the JWT in the response body and the API has
//! no refresh endpoint. Revisit if that changes.

use std::cell::RefCell;

use chrono::NaiveDate;
use fingest_client_ports::{Clock, Session, SessionStore, TokenSource};
use gloo_storage::{SessionStorage, Storage};

/// Namespaced so it cannot collide with anything else served from the same origin.
const KEY: &str = "fingest.session";

/// `sessionStorage`, with an in-memory mirror in front of it.
///
/// Storage can be unavailable — private browsing, quota exhausted, a policy that blocks it.
/// The mirror means that degrades to "you are logged out when you reload" rather than
/// "you cannot log in at all".
#[derive(Default)]
pub struct BrowserSessionStore {
    memory: RefCell<Option<Session>>,
}

impl BrowserSessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionStore for BrowserSessionStore {
    fn load(&self) -> Option<Session> {
        if let Some(session) = self.memory.borrow().clone() {
            return Some(session);
        }

        match SessionStorage::get::<Session>(KEY) {
            Ok(session) => {
                *self.memory.borrow_mut() = Some(session.clone());
                Some(session)
            }
            Err(error) => {
                // Absent is the common case and not worth a warning; anything else means a
                // shape change or a corrupt entry, so drop it rather than retry every load.
                tracing::debug!(%error, "no usable stored session");
                SessionStorage::delete(KEY);
                None
            }
        }
    }

    fn save(&self, session: &Session) {
        *self.memory.borrow_mut() = Some(session.clone());

        if let Err(error) = SessionStorage::set(KEY, session) {
            tracing::warn!(%error, "session will not survive a reload");
        }
    }

    fn clear(&self) {
        *self.memory.borrow_mut() = None;
        SessionStorage::delete(KEY);
    }
}

/// The store is also the transport's token source: it can read the token but, through this
/// narrower trait, cannot end the session behind a use case's back.
impl TokenSource for BrowserSessionStore {
    fn token(&self) -> Option<String> {
        self.load().map(|session| session.token)
    }
}

/// No persistence at all. Used by tests and by any build that must not touch web storage.
#[derive(Default)]
pub struct MemorySessionStore(RefCell<Option<Session>>);
impl MemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionStore for MemorySessionStore {
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

impl TokenSource for MemorySessionStore {
    fn token(&self) -> Option<String> {
        self.load().map(|session| session.token)
    }
}

/// The browser's local date.
///
/// `chrono` reaches the host clock through `wasm-bindgen` here, which is why this is an
/// adapter and not something a use case may call directly. Local rather than UTC: a date
/// typed into a form means the user's today, not Greenwich's.
pub struct BrowserClock;

impl Clock for BrowserClock {
    fn today(&self) -> NaiveDate {
        chrono::Local::now().date_naive()
    }
}

/// A clock frozen at one date, for tests that must not drift.
pub struct FixedClock(pub NaiveDate);

impl Clock for FixedClock {
    fn today(&self) -> NaiveDate {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fingest_contracts::UserDto;

    fn session() -> Session {
        Session {
            token: "token-abc".into(),
            user: UserDto {
                login: "bob".into(),
                first_name: Some("Bob".into()),
                last_name: None,
                admin: false,
            },
        }
    }

    #[test]
    fn the_memory_store_round_trips() {
        let store = MemorySessionStore::new();
        assert!(store.load().is_none());

        store.save(&session());
        assert_eq!(store.load(), Some(session()));

        store.clear();
        assert!(store.load().is_none());
    }

    #[test]
    fn saving_twice_replaces_rather_than_accumulates() {
        let store = MemorySessionStore::new();
        store.save(&session());

        let mut replacement = session();
        replacement.token = "token-xyz".into();
        store.save(&replacement);

        assert_eq!(store.load().unwrap().token, "token-xyz");
    }
}

/// Browser-only: these need a real `Window`, so they run under `wasm-pack test`.
#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use fingest_contracts::UserDto;
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    wasm_bindgen_test_configure!(run_in_browser);

    fn session() -> Session {
        Session {
            token: "token-abc".into(),
            user: UserDto {
                login: "bob".into(),
                first_name: None,
                last_name: None,
                admin: true,
            },
        }
    }

    #[wasm_bindgen_test]
    fn a_saved_session_survives_a_new_store_instance() {
        BrowserSessionStore::new().clear();

        BrowserSessionStore::new().save(&session());

        // A fresh instance has an empty mirror, so this can only come from sessionStorage.
        assert_eq!(BrowserSessionStore::new().load(), Some(session()));
    }

    #[wasm_bindgen_test]
    fn clearing_removes_it_from_storage_too() {
        let store = BrowserSessionStore::new();
        store.save(&session());
        store.clear();

        assert!(BrowserSessionStore::new().load().is_none());
    }

    #[wasm_bindgen_test]
    fn a_corrupt_entry_reads_as_logged_out_and_is_discarded() {
        SessionStorage::set(KEY, "not a session").unwrap();

        assert!(BrowserSessionStore::new().load().is_none());
        assert!(SessionStorage::get::<Session>(KEY).is_err());
    }
}
