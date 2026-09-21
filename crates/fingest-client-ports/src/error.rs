use fingest_contracts::ErrorResponse;

/// Every way a call to the API can fail, from the client's point of view.
///
/// The first six variants mirror `ApiError` in `fingest-http` one-for-one, so a handler here
/// can reason in the same categories the server used. The last two have no server-side
/// counterpart: they are the failure modes of the wire itself.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClientError {
    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    Unauthenticated(String),

    #[error("{0}")]
    Forbidden(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Conflict(String),

    #[error("{0}")]
    Server(String),

    /// The request never produced a response: offline, DNS, CORS, TLS.
    #[error("{0}")]
    Network(String),

    /// A response arrived but did not match the contract.
    #[error("{0}")]
    Decode(String),
}

impl ClientError {
    /// Maps an HTTP status and a parsed `{"status","message"}` body onto a category.
    ///
    /// The body is optional because a proxy, a CORS failure or a panicking server can
    /// produce a status with no contract-shaped body at all. Falling back to a generic
    /// message keeps the category correct even then.
    pub fn from_status(status: u16, body: Option<ErrorResponse>) -> Self {
        let message = body
            .map(|b| b.message)
            .unwrap_or_else(|| format!("Request failed with status {status}"));

        match status {
            400 => Self::BadRequest(message),
            401 => Self::Unauthenticated(message),
            403 => Self::Forbidden(message),
            404 => Self::NotFound(message),
            409 => Self::Conflict(message),
            _ => Self::Server(message),
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::BadRequest(m)
            | Self::Unauthenticated(m)
            | Self::Forbidden(m)
            | Self::NotFound(m)
            | Self::Conflict(m)
            | Self::Server(m)
            | Self::Network(m)
            | Self::Decode(m) => m,
        }
    }

    /// Whether this failure should end the session.
    pub fn is_unauthenticated(&self) -> bool {
        matches!(self, Self::Unauthenticated(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(message: &str) -> Option<ErrorResponse> {
        Some(ErrorResponse::new(0, message))
    }

    #[test]
    fn each_status_maps_to_the_category_the_server_used() {
        let cases = [
            (400, ClientError::BadRequest("m".into())),
            (401, ClientError::Unauthenticated("m".into())),
            (403, ClientError::Forbidden("m".into())),
            (404, ClientError::NotFound("m".into())),
            (409, ClientError::Conflict("m".into())),
            (500, ClientError::Server("m".into())),
        ];

        for (status, expected) in cases {
            assert_eq!(ClientError::from_status(status, body("m")), expected);
        }
    }

    /// v1's error body types `status` as a string; only the HTTP status decides the category.
    #[test]
    fn the_body_supplies_the_message_not_the_category() {
        let parsed: ErrorResponse =
            serde_json::from_str(r#"{"status":"404","message":"Wallet not found"}"#).unwrap();

        let err = ClientError::from_status(404, Some(parsed));

        assert_eq!(err, ClientError::NotFound("Wallet not found".into()));
    }

    #[test]
    fn a_bodyless_failure_still_lands_in_the_right_category() {
        let err = ClientError::from_status(403, None);

        assert!(matches!(err, ClientError::Forbidden(_)));
        assert!(err.message().contains("403"));
    }

    #[test]
    fn an_unrecognised_status_is_treated_as_a_server_fault() {
        assert!(matches!(
            ClientError::from_status(418, None),
            ClientError::Server(_)
        ));
    }

    #[test]
    fn only_a_401_ends_the_session() {
        assert!(ClientError::from_status(401, None).is_unauthenticated());
        assert!(!ClientError::from_status(403, None).is_unauthenticated());
        assert!(!ClientError::Network("offline".into()).is_unauthenticated());
    }
}
