//! Presentation logic shared by every client.
//!
//! A component cannot be shared across form factors — a five-input inline row is unusable
//! at 390px — but everything *behind* one can: which ports a screen may reach, how money
//! reads, how a failure is worded, how a category resolves. Those live here, with tests,
//! and each shell renders them its own way.

pub mod busy;
pub mod category;
pub mod context;
pub mod money;
pub mod wording;

pub use busy::hold;
pub use context::{AppContext, app_context, use_event_refresh};
pub use money::format_money;
pub use wording::{NOT_SIGNED_IN, describe};
