use std::rc::Rc;

use fingest_client_ports::{ClientError, ClientEvent, EventBus, NameField, Session, UsersApi};
use fingest_contracts::UserDto;

/// Account administration, as opposed to authentication.
///
/// The server's policy is duplicated here so the UI hides what it would refuse. That
/// duplication is deliberate and one-directional: the server remains the authority, and
/// every method here would still be rejected there if the check were removed.
pub struct UserUseCase {
    api: Rc<dyn UsersApi>,
    events: Rc<dyn EventBus>,
}

impl UserUseCase {
    pub fn new(api: Rc<dyn UsersApi>, events: Rc<dyn EventBus>) -> Self {
        Self { api, events }
    }

    /// Listing is admin-only: the server checks self-or-admin *and* admin, which only an
    /// admin can satisfy.
    pub async fn list(&self, session: &Session) -> Result<Vec<UserDto>, ClientError> {
        if !session.is_admin() {
            return Err(forbidden("Admin privileges required"));
        }

        self.api.list(session.login()).await
    }

    pub async fn update_name(
        &self,
        session: &Session,
        login: &str,
        field: NameField,
        value: &str,
    ) -> Result<(), ClientError> {
        if !session.may_act_on(login) {
            return Err(forbidden("You can only edit your own account"));
        }

        let value = value.trim();
        if value.is_empty() {
            // The server reads the value out of the body and rejects a missing one; an
            // all-whitespace name passes that check but is not a name.
            return Err(ClientError::BadRequest(format!(
                "{} is required",
                field.label()
            )));
        }

        self.api.update_name(login, field, value).await?;
        self.events.publish(ClientEvent::AccountChanged {
            login: login.to_owned(),
        });

        Ok(())
    }

    pub async fn delete(&self, session: &Session, login: &str) -> Result<(), ClientError> {
        if !session.may_act_on(login) {
            return Err(forbidden("You can only delete your own account"));
        }

        self.api.delete(login).await?;
        self.events.publish(ClientEvent::AccountRemoved {
            login: login.to_owned(),
        });

        Ok(())
    }
}

/// Worded as the server words it, so a client-side refusal and a server-side one read alike.
fn forbidden(reason: &str) -> ClientError {
    ClientError::Forbidden(format!("Not authorized. {reason}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{RecordingBus, StubUsersApi};
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

    fn use_case(api: Rc<StubUsersApi>) -> (UserUseCase, Rc<RecordingBus>) {
        let events = Rc::new(RecordingBus::default());
        (UserUseCase::new(api, events.clone()), events)
    }

    #[test]
    fn only_an_admin_can_list_accounts() {
        let api = StubUsersApi::returning(vec![]);
        let (users, _) = use_case(api.clone());

        assert!(block_on(users.list(&session("root", true))).is_ok());

        let err = block_on(users.list(&session("bob", false))).unwrap_err();
        assert!(matches!(err, ClientError::Forbidden(_)));
        assert_eq!(api.calls(), 1, "the refused call never left the browser");
    }

    #[test]
    fn a_user_may_rename_themselves() {
        let api = StubUsersApi::returning(vec![]);
        let (users, events) = use_case(api.clone());

        block_on(users.update_name(&session("bob", false), "bob", NameField::First, "Bob"))
            .unwrap();

        assert_eq!(
            api.last_edit(),
            Some(("bob".into(), NameField::First, "Bob".into()))
        );
        assert_eq!(
            events.recorded(),
            vec![ClientEvent::AccountChanged {
                login: "bob".into()
            }]
        );
    }

    #[test]
    fn a_user_may_not_rename_someone_else() {
        let api = StubUsersApi::returning(vec![]);
        let (users, events) = use_case(api.clone());

        let err =
            block_on(users.update_name(&session("bob", false), "alice", NameField::Last, "Smith"))
                .unwrap_err();

        assert!(matches!(err, ClientError::Forbidden(_)));
        assert_eq!(api.calls(), 0);
        assert!(events.recorded().is_empty());
    }

    #[test]
    fn an_admin_may_rename_anyone() {
        let api = StubUsersApi::returning(vec![]);
        let (users, _) = use_case(api.clone());

        block_on(users.update_name(&session("root", true), "bob", NameField::Last, "Builder"))
            .unwrap();

        assert_eq!(api.calls(), 1);
    }

    #[test]
    fn a_whitespace_only_name_never_reaches_the_network() {
        let api = StubUsersApi::returning(vec![]);
        let (users, events) = use_case(api.clone());

        let err =
            block_on(users.update_name(&session("bob", false), "bob", NameField::First, "  "))
                .unwrap_err();

        assert!(matches!(err, ClientError::BadRequest(_)));
        assert_eq!(api.calls(), 0);
        assert!(events.recorded().is_empty());
    }

    #[test]
    fn a_name_is_trimmed_before_it_is_sent() {
        let api = StubUsersApi::returning(vec![]);
        let (users, _) = use_case(api.clone());

        block_on(users.update_name(&session("bob", false), "bob", NameField::First, " Bob "))
            .unwrap();

        assert_eq!(api.last_edit().unwrap().2, "Bob");
    }

    #[test]
    fn deleting_someone_else_is_refused_before_the_network() {
        let api = StubUsersApi::returning(vec![]);
        let (users, _) = use_case(api.clone());

        assert!(block_on(users.delete(&session("bob", false), "alice")).is_err());
        assert_eq!(api.calls(), 0);
    }

    #[test]
    fn a_failed_delete_announces_nothing() {
        let api = StubUsersApi::failing(ClientError::NotFound("user does not exist".into()));
        let (users, events) = use_case(api);

        assert!(block_on(users.delete(&session("bob", false), "bob")).is_err());
        assert!(events.recorded().is_empty());
    }
}
