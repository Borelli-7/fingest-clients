use dioxus::prelude::*;
use fingest_client_ports::ClientEvent;
use fingest_client_view::{app_context, describe, hold, use_event_refresh};

/// Shared reference data. Listing is public on the server; mutating is admin-only, and the
/// controls follow that so a non-admin is never offered an action that would 403.
#[component]
pub fn Categories() -> Element {
    let context = app_context();
    let is_admin = context.is_admin();

    let catalog = context.catalog.clone();
    let categories = use_resource(move || {
        let catalog = catalog.clone();
        async move { catalog.list().await }
    });

    use_event_refresh(categories, |event| {
        matches!(event, ClientEvent::CategoryChanged)
    });

    rsx! {
        header { class: "bar", h1 { "Categories" } }

        if is_admin {
            CategoryForm {}
        }

        match &*categories.read_unchecked() {
            None => rsx! { p { class: "muted", "Loading…" } },
            Some(Err(error)) => rsx! {
                p { class: "error", role: "alert", "{describe(error)}" }
            },
            Some(Ok(list)) => rsx! {
                ul { class: "cards",
                    for category in list.clone() {
                        li { key: "{category.name}-{category.profit}", class: "card",
                            div { class: "card-link",
                                span { class: "card-title", "{category.name}" }
                                span { class: "muted small",
                                    if category.profit { "Income" } else { "Expense" }
                                }
                            }
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn CategoryForm() -> Element {
    let mut name = use_signal(String::new);
    let mut profit = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let busy = use_signal(|| false);

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }

        let context = app_context();
        error.set(None);
        let busy_guard = hold(busy);

        spawn(async move {
            let _busy = busy_guard;
            match context.catalog.create(&name(), profit()).await {
                Ok(_) => name.set(String::new()),
                Err(failure) => error.set(Some(describe(&failure))),
            }
        });
    };

    rsx! {
        form { class: "stack", onsubmit: submit,
            if let Some(message) = error() {
                p { class: "error", role: "alert", "{message}" }
            }
            input {
                "aria-label": "New category name",
                placeholder: "New category",
                value: "{name}",
                oninput: move |event| name.set(event.value()),
            }
            label { class: "inline",
                input {
                    r#type: "checkbox",
                    checked: profit(),
                    onchange: move |event| profit.set(event.checked()),
                }
                "Income"
            }
            button { class: "button", r#type: "submit", disabled: busy(), "Add" }
        }
    }
}
