//! Identity bounded context, client side.
//!
//! Owns what it means to be logged in: starting a session, restoring one across a reload,
//! and ending one — deliberately or because the token stopped working.

pub mod auth;
pub mod users;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use auth::{AuthUseCase, PASSWORD_MIN_LEN};
pub use users::UserUseCase;
