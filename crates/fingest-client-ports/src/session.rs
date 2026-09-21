use fingest_contracts::UserDto;
use serde::{Deserialize, Serialize};

/// The authenticated identity, as far as the browser is concerned.
///
/// The JWT is opaque here: claims are never parsed client-side, because a client-parsed
/// claim is a client-controlled claim. Authorisation decisions in the UI are cosmetic —
/// the server re-checks every one of them.
///
/// Serialisable so a store can survive a reload; this is not a wire contract and nothing
/// on the server reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub token: String,
    pub user: UserDto,
}

impl Session {
    pub fn login(&self) -> &str {
        &self.user.login
    }

    /// Cosmetic only: hides controls the server would reject anyway.
    pub fn is_admin(&self) -> bool {
        self.user.admin
    }

    /// Mirrors `AuthenticatedUser::require_self_or_admin` so the UI hides exactly what the
    /// server would refuse. Duplicated deliberately — the server remains the authority.
    pub fn may_act_on(&self, login: &str) -> bool {
        self.is_admin() || self.login() == login
    }
}

/// Where the session lives between renders, and across a reload.
pub trait SessionStore {
    fn load(&self) -> Option<Session>;
    fn save(&self, session: &Session);
    fn clear(&self);
}

/// The narrow slice the HTTP adapter needs.
///
/// Separate from [`SessionStore`] so the transport can read the token but cannot end the
/// session behind the use case's back.
pub trait TokenSource {
    fn token(&self) -> Option<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(login: &str, admin: bool) -> Session {
        Session {
            token: "opaque".into(),
            user: UserDto {
                login: login.into(),
                first_name: None,
                last_name: None,
                admin,
            },
        }
    }

    #[test]
    fn a_user_may_act_on_themselves() {
        assert!(session("bob", false).may_act_on("bob"));
    }

    #[test]
    fn an_admin_may_act_on_anyone() {
        assert!(session("root", true).may_act_on("bob"));
    }

    #[test]
    fn a_user_may_not_act_on_someone_else() {
        assert!(!session("bob", false).may_act_on("alice"));
    }
}
