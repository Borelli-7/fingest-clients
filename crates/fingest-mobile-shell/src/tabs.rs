use dioxus::prelude::*;
use fingest_client_view::app_context;

use crate::routes::Route;

/// Bottom-tab layout for every signed-in screen.
///
/// A bar rather than the web client's top nav: on a phone the reachable area is the bottom
/// of the screen. Guarding here rather than per screen means a route added under this
/// layout is protected by construction — the same reasoning as scoping `JwtAuth` to
/// `/resources/users` on the server instead of listing paths.
#[component]
pub fn Tabs() -> Element {
    let context = app_context();
    let session_signal = context.session;
    let navigator = use_navigator();

    use_effect(move || {
        if session_signal.read().is_none() {
            navigator.replace(Route::Login {});
        }
    });

    if session_signal.read().is_none() {
        return rsx! { section { class: "screen centred", p { class: "muted", "…" } } };
    }

    let entries = context.modules.nav(&context.capabilities);

    rsx! {
        div { class: "app",
            main { class: "screen", Outlet::<Route> {} }
            nav { class: "tabbar",
                for entry in entries {
                    a { key: "{entry.path}", class: "tab", href: "{entry.path}", "{entry.label}" }
                }
            }
        }
    }
}
