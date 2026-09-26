use dioxus::prelude::*;
use fingest_client_view::{app_context, describe, hold};

use crate::routes::Route;

/// Sign-in screen.
///
/// The same use case the web client drives, rendered for a thumb: full-width fields,
/// stacked rather than inline, and `inputmode`/`autocomplete` hints so the on-screen
/// keyboard shows the right layout.
#[component]
pub fn Login() -> Element {
    let session_signal = app_context().session;
    let mut login = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let submitting = use_signal(|| false);

    let navigator = use_navigator();

    use_effect(move || {
        if session_signal.read().is_some() {
            navigator.replace(Route::Wallets {});
        }
    });

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if submitting() {
            return;
        }

        let context = app_context();
        let mut session = context.session;
        error.set(None);
        let busy_guard = hold(submitting);

        spawn(async move {
            let _busy = busy_guard;

            match context.auth.login(&login(), &password()).await {
                Ok(started) => {
                    session.set(Some(started));
                    password.set(String::new());
                    navigator.replace(Route::Wallets {});
                }
                Err(failure) => {
                    error.set(Some(describe(&failure)));
                    password.set(String::new());
                }
            }
        });
    };

    rsx! {
        section { class: "screen centred",
            form { class: "stack", onsubmit: submit,
                h1 { "FinGest" }
                p { class: "muted", "Sign in to continue" }

                if let Some(message) = error() {
                    p { class: "error", role: "alert", "{message}" }
                }

                input {
                    "aria-label": "Login",
                    placeholder: "Login",
                    autocomplete: "username",
                    autocapitalize: "none",
                    required: true,
                    value: "{login}",
                    oninput: move |event| login.set(event.value()),
                }
                input {
                    "aria-label": "Password",
                    placeholder: "Password",
                    r#type: "password",
                    autocomplete: "current-password",
                    required: true,
                    value: "{password}",
                    oninput: move |event| password.set(event.value()),
                }
                button { class: "button", r#type: "submit", disabled: submitting(),
                    if submitting() { "Signing in…" } else { "Sign in" }
                }
            }
        }
    }
}
