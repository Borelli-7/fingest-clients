//! Web shell.
//!
//! Owns every screen the browser renders, and nothing else. Presentation logic that a
//! phone would also need lives in `fingest-client-view`; business rules live below that
//! again, which is why a `*-core` test needs neither a browser nor a device.

pub mod budgets;
pub mod categories;
pub mod home;
pub mod login;
pub mod not_found;
pub mod routes;
pub mod shell;
pub mod users;
pub mod wallet_detail;
pub mod wallets;

pub use fingest_client_view::AppContext;
pub use routes::Route;

use dioxus::prelude::*;

const STYLES: Asset = asset!("/assets/main.css");

/// Renders the router. Dependency wiring happened before this component existed — the
/// composition root provides [`AppContext`] and this only consumes it.
#[component]
pub fn Root() -> Element {
    rsx! {
        document::Stylesheet { href: STYLES }
        Router::<Route> {}
    }
}
