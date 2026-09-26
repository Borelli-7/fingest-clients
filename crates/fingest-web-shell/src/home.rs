use dioxus::prelude::*;

use fingest_client_view::{app_context, describe};

#[component]
pub fn Home() -> Element {
    let context = app_context();

    // The only authenticated call in this phase. It is what turns a stale token into a 401,
    // which the transport announces and the composition root acts on — so the guard chain is
    // exercised on every visit rather than only once real data arrives.
    let auth = context.auth.clone();
    let token_status = use_resource(move || {
        let auth = auth.clone();
        async move { auth.verify().await }
    });

    // Hooks above, branches below: Dioxus pairs hooks by call order on every render.
    let Some(session) = context.session.read().clone() else {
        return rsx! {};
    };

    let modules = context.modules.enabled(&context.capabilities);

    rsx! {
        h1 { "Signed in as {session.user.login}" }
        p { class: "muted",
            if session.is_admin() { "Administrator" } else { "Standard account" }
        }

        section {
            h2 { "Token" }
            match &*token_status.read_unchecked() {
                None => rsx! { p { class: "muted", "Checking…" } },
                Some(Ok(status)) => rsx! { p { class: "muted", "Accepted for {status.login}" } },
                Some(Err(error)) => rsx! {
                    p { class: "error", role: "alert", "{describe(error)}" }
                },
            }
        }

        section {
            h2 { "Modules" }
            ul {
                for module in modules {
                    li { key: "{module.name}",
                        "{module.name}"
                        if let Some(capability) = module.required_capability {
                            span { class: "tag", "{capability}" }
                        }
                    }
                }
            }
        }

        section {
            h2 { "Server capabilities" }
            if context.capabilities.is_empty() {
                p { class: "muted",
                    "None reported. Gated modules stay hidden — the client fails closed."
                }
            }
        }
    }
}
