use dioxus::prelude::*;

use crate::{
    budgets::Budgets, categories::Categories, forecast::Forecast, home::Home, login::Login, not_found::NotFound,
    shell::Shell, users::Users, wallet_detail::WalletDetail, wallets::Wallets,
};

/// Every path the client answers.
///
/// A `Routable` enum is compile-time, so a module gated off at runtime cannot literally be
/// unregistered. Its route component asks `ModuleRegistry::is_enabled` first and renders
/// [`NotFound`] when the answer is no, which gives a deep link the same observable outcome.
#[derive(Routable, Clone, PartialEq, Eq, Debug)]
#[rustfmt::skip]
pub enum Route {
    #[route("/login")]
    Login {},

    #[layout(Shell)]
        #[route("/")]
        Home {},

        #[route("/wallets")]
        Wallets {},

        #[route("/wallets/:wallet_id")]
        WalletDetail { wallet_id: i32 },

        #[route("/categories")]
        Categories {},

        #[route("/budgets")]
        Budgets {},

        #[route("/forecast")]
        Forecast {},

        #[route("/accounts")]
        Users {},

    #[end_layout]
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

impl Route {
    /// The screen a nav entry's path names, or `None` when only the catch-all would answer.
    pub fn for_nav(path: &str) -> Option<Self> {
        match path.parse::<Self>() {
            Ok(Self::NotFound { .. }) | Err(_) => None,
            Ok(route) => Some(route),
        }
    }
}
