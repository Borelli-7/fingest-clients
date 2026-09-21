//! Mobile shell.
//!
//! Owns every screen the app renders and nothing else. The rules behind them —
//! authorisation, currency, validation, cache invalidation — are the same crates the web
//! client uses, which is why this crate holds no tests: there is no logic here to test.

pub mod budgets;
pub mod categories;
pub mod entry_sheet;
pub mod login;
pub mod routes;
pub mod tabs;
pub mod wallets;

pub use fingest_client_view::AppContext;
pub use routes::Route;

use dioxus::prelude::*;

const STYLES: Asset = asset!("/assets/mobile.css");

/// Renders the router. Wiring happened before this component existed — the composition
/// root provides [`AppContext`] and this only consumes it.
#[component]
pub fn Root() -> Element {
    rsx! {
        document::Stylesheet { href: STYLES }
        Router::<Route> {}
    }
}
