//! Port definitions.
//!
//! Every trait the interior of the hexagon is allowed to know about. Nothing here names a
//! transport, a storage mechanism or a UI framework — those live in the adapter crates and
//! are bound to these traits by `fingest-web-bootstrap`.
//!
//! The mirror image of `fingest-*-core` in the API workspace, where ports are declared
//! next to the use cases that drive them.

pub mod api;
pub mod clock;
pub mod error;
pub mod events;
pub mod session;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use api::{
    AuthApi, BudgetFilter, CapabilityApi, CatalogApi, NameField, PlanningApi, UsersApi, WalletsApi,
};
pub use clock::Clock;
pub use error::ClientError;
pub use events::{ClientEvent, EventBus, Subscriber, SubscriptionId};
pub use session::{Session, SessionStore, TokenSource};
