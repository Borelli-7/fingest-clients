use fingest_client_ports::ClientError;

/// Turns a failure into something worth showing a person.
///
/// Transport failures are the client's problem, not the user's, so they are reworded.
/// Everything else is the server's own wording, which is already user-facing — and in the
/// case of `"Invalid credentials"` is deliberately identical whether or not the login
/// exists. Paraphrasing it would risk reintroducing the distinction the server removed.
pub fn describe(error: &ClientError) -> String {
    match error {
        ClientError::Network(_) => "Could not reach the server. Check your connection.".to_owned(),
        ClientError::Decode(_) => "The server sent an unexpected response.".to_owned(),
        ClientError::Conflict(_) => "That is already in use, or already exists.".to_owned(),
        other => other.message().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_failures_are_reworded_for_a_human() {
        assert!(describe(&ClientError::Network("ECONNREFUSED".into())).contains("connection"));
        assert!(describe(&ClientError::Decode("expected `,`".into())).contains("unexpected"));
    }

    /// Deviation D11 turned a 500 into a 409; this explains what to do about it.
    #[test]
    fn a_conflict_explains_itself() {
        assert!(describe(&ClientError::Conflict("category in use".into())).contains("in use"));
    }

    #[test]
    fn the_servers_own_wording_survives_untouched() {
        assert_eq!(
            describe(&ClientError::Unauthenticated("Invalid credentials".into())),
            "Invalid credentials"
        );
        assert_eq!(
            describe(&ClientError::Forbidden("Not authorized.".into())),
            "Not authorized."
        );
    }

    /// A driver message must never reach a user; the server already made it generic.
    #[test]
    fn a_server_fault_shows_what_the_server_said_and_no_more() {
        assert_eq!(
            describe(&ClientError::Server("Internal server error".into())),
            "Internal server error"
        );
    }
}
