use dioxus::prelude::*;

use crate::routes::Route;
use fingest_client_view::{app_context, describe};

#[component]
pub fn Login() -> Element {
    let session_signal = app_context().session;
    let mut login = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut submitting = use_signal(|| false);

    let navigator = use_navigator();

    // Someone with a live session has no business on this page.
    use_effect(move || {
        if session_signal.read().is_some() {
            navigator.replace(Route::Home {});
        }
    });

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if submitting() {
            return;
        }

        let context = app_context();
        let mut session = context.session;
        spawn(async move {
            submitting.set(true);
            error.set(None);

            match context.auth.login(&login(), &password()).await {
                Ok(started) => {
                    session.set(Some(started));
                    password.set(String::new());
                    navigator.replace(Route::Home {});
                }
                Err(failure) => {
                    // The server says "Invalid credentials" whether or not the login exists;
                    // repeating it verbatim keeps that property intact.
                    error.set(Some(describe(&failure)));
                    password.set(String::new());
                }
            }

            submitting.set(false);
        });
    };

    rsx! {
        main { class: "centered",
            form { class: "card", onsubmit: submit,
                h1 { "FinGest" }
                p { class: "muted", "Sign in to continue" }

                if let Some(message) = error() {
                    p { class: "error", role: "alert", "{message}" }
                }

                label { r#for: "login", "Login" }
                input {
                    id: "login",
                    name: "login",
                    autocomplete: "username",
                    required: true,
                    value: "{login}",
                    oninput: move |event| login.set(event.value()),
                }

                label { r#for: "password", "Password" }
                input {
                    id: "password",
                    name: "password",
                    r#type: "password",
                    autocomplete: "current-password",
                    required: true,
                    value: "{password}",
                    oninput: move |event| password.set(event.value()),
                }

                button { r#type: "submit", disabled: submitting(),
                    if submitting() { "Signing in…" } else { "Sign in" }
                }
            }
        }
    }
}
