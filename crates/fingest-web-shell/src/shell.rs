use dioxus::prelude::*;

use crate::routes::Route;
use fingest_client_view::app_context;

/// Layout for every authenticated route.
///
/// Guards here rather than in each page: a new route added under this layout is protected by
/// construction. The same reasoning as scoping `JwtAuth` to `/resources/users` on the server
/// instead of listing paths.
#[component]
pub fn Shell() -> Element {
    let context = app_context();
    let session_signal = context.session;
    let navigator = use_navigator();

    use_effect(move || {
        if session_signal.read().is_none() {
            navigator.replace(Route::Login {});
        }
    });

    // Nothing renders until the redirect above has run, so a signed-out user never sees a
    // flash of authenticated chrome.
    let Some(session) = session_signal.read().clone() else {
        return rsx! { main { class: "centered", p { "Redirecting…" } } };
    };

    let entries = context.modules.nav(&context.capabilities);

    let sign_out = move |_| {
        let context = app_context();
        let mut session = context.session;
        context.auth.logout();
        session.set(None);
        navigator.replace(Route::Login {});
    };

    rsx! {
        header { class: "bar",
            span { class: "brand", "FinGest" }
            nav {
                for entry in entries {
                    if let Some(route) = Route::for_nav(entry.path) {
                        Link { key: "{entry.path}", to: route, "{entry.label}" }
                    }
                }
            }
            span { class: "spacer" }
            span { class: "muted", "{session.user.login}" }
            button { class: "link", onclick: sign_out, "Sign out" }
        }
        main { class: "page", Outlet::<Route> {} }
    }
}
