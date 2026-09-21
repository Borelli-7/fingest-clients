//! Turns a response into either a value or a [`ClientError`].
//!
//! Split out from the transport so the mapping can be tested without a network: every rule
//! that decides what an error *means* lives here, in ordinary functions over a status code
//! and a body string.

use fingest_client_ports::ClientError;
use fingest_contracts::ErrorResponse;
use serde::de::DeserializeOwned;

/// Whether a 401 should end the session.
///
/// Only when the request carried a token. A 401 from `POST /api/auth/login` means the
/// password was wrong, not that a session lapsed — treating the two alike would fire
/// `SessionExpired` at someone who never had a session, and show them the wrong message.
pub fn is_session_expiry(status: u16, token_was_sent: bool) -> bool {
    status == 401 && token_was_sent
}

/// Builds the error for a non-2xx response.
pub fn error_from(status: u16, body: &str) -> ClientError {
    // The server always sends `{"status","message"}`, but a proxy or a CORS preflight
    // failure can produce a status with something else entirely.
    let parsed = serde_json::from_str::<ErrorResponse>(body).ok();

    if parsed.is_none() && !body.is_empty() {
        tracing::warn!(status, "error body did not match the contract");
    }

    ClientError::from_status(status, parsed)
}

/// Decodes a success body.
///
/// An empty body deserialises as `()` for the 204s the API returns from every `DELETE`.
pub fn decode<T: DeserializeOwned>(body: &str) -> Result<T, ClientError> {
    if body.trim().is_empty() {
        // `serde_json` parses "null" into `()` and `Option::None`, and rejects it for
        // anything else — which is exactly the distinction wanted here.
        return serde_json::from_str("null")
            .map_err(|_| ClientError::Decode("The server returned an empty body".to_owned()));
    }

    serde_json::from_str(body).map_err(|error| {
        tracing::error!(%error, "response did not match the contract");
        ClientError::Decode(format!("Unexpected response from the server: {error}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fingest_contracts::UserDto;

    #[test]
    fn a_contract_shaped_error_keeps_its_message() {
        let err = error_from(404, r#"{"status":"404","message":"Wallet not found"}"#);

        assert_eq!(err, ClientError::NotFound("Wallet not found".into()));
    }

    #[test]
    fn a_garbage_body_still_yields_the_right_category() {
        let err = error_from(409, "<html>502 Bad Gateway</html>");

        assert!(matches!(err, ClientError::Conflict(_)));
    }

    #[test]
    fn an_empty_error_body_is_tolerated() {
        assert!(matches!(error_from(500, ""), ClientError::Server(_)));
    }

    #[test]
    fn a_failed_login_is_not_a_lapsed_session() {
        assert!(!is_session_expiry(401, false));
        assert!(is_session_expiry(401, true));
    }

    #[test]
    fn only_a_401_ends_a_session() {
        assert!(!is_session_expiry(403, true));
        assert!(!is_session_expiry(200, true));
    }

    #[test]
    fn a_success_body_decodes_to_the_contract_type() {
        let user: UserDto =
            decode(r#"{"login":"bob","firstName":"Bob","lastName":null,"admin":false}"#).unwrap();

        assert_eq!(user.login, "bob");
        assert_eq!(user.first_name.as_deref(), Some("Bob"));
    }

    /// Every `DELETE` in the API answers 204 with no body.
    #[test]
    fn an_empty_success_body_decodes_as_unit() {
        assert_eq!(decode::<()>(""), Ok(()));
        assert_eq!(decode::<()>("   "), Ok(()));
    }

    #[test]
    fn an_empty_body_where_a_value_was_expected_is_an_error() {
        assert!(matches!(decode::<UserDto>(""), Err(ClientError::Decode(_))));
    }

    #[test]
    fn a_body_that_breaks_the_contract_is_reported_as_such() {
        let err = decode::<UserDto>(r#"{"login":42}"#).unwrap_err();

        assert!(matches!(err, ClientError::Decode(_)));
    }
}
