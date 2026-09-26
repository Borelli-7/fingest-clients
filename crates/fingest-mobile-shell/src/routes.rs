use dioxus::prelude::*;

use crate::{
    budgets::Budgets, categories::Categories, forecast::Forecast, login::Login, tabs::Tabs,
    wallets::WalletDetail, wallets::Wallets,
};

/// Every screen the app answers.
///
/// Flatter than the web router on purpose: a phone navigates by tab and by push, so deep
/// hierarchies are reached by drilling in rather than by URL shape. The paths still match
/// the web client's so a future deep link resolves the same on both.
#[derive(Routable, Clone, PartialEq, Eq, Debug)]
#[rustfmt::skip]
pub enum Route {
    #[route("/login")]
    Login {},

    #[layout(Tabs)]
        #[route("/")]
        Wallets {},

        #[route("/wallets/:wallet_id")]
        WalletDetail { wallet_id: i32 },

        #[route("/budgets")]
        Budgets {},

        #[route("/forecast")]
        Forecast {},

        #[route("/categories")]
        Categories {},

    #[end_layout]
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    rsx! {
        section { class: "screen centred",
            h1 { "Not found" }
            p { class: "muted", "/{segments.join(\"/\")}" }
            Link { class: "button", to: Route::Wallets {}, "Go back" }
        }
    }
}
