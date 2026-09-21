//! Outbound HTTP adapter.
//!
//! The only crate that knows the API is reachable over a network. Everything above it sees
//! the ports in `fingest-web-ports` and could just as well be talking to an in-memory double
//! — which is exactly what the core tests do.

pub mod response;
pub mod transport;
pub mod url;

pub use transport::ApiClient;
