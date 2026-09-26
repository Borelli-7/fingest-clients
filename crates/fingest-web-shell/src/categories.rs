use dioxus::prelude::*;
use fingest_client_ports::ClientEvent;
use fingest_client_view::{app_context, describe, use_event_refresh};
use fingest_contracts::CategoryDto;

/// Shared reference data. Listing is public on the server; everything else is admin-only,
/// and the controls follow that so a non-admin is never offered an action that would 403.
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
        h1 { "Categories" }

        if is_admin {
            CategoryForm {}
        } else {
            p { class: "muted", "Only an administrator can change these." }
        }

        match &*categories.read_unchecked() {
            None => rsx! { p { class: "muted", "Loading…" } },
            Some(Err(error)) => rsx! {
                p { class: "error", role: "alert", "{describe(error)}" }
            },
            Some(Ok(list)) if list.is_empty() => rsx! {
                p { class: "muted", "No categories yet." }
            },
            Some(Ok(list)) => rsx! {
                table {
                    thead {
                        tr {
                            th { "Name" }
                            th { "Kind" }
                            if is_admin { th { "" } }
                        }
                    }
                    tbody {
                        for category in list.clone() {
                            CategoryRow {
                                key: "{category.name}-{category.profit}",
                                category,
                                is_admin,
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
    let mut busy = use_signal(|| false);

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }

        let catalog = app_context().catalog;
        spawn(async move {
            busy.set(true);
            error.set(None);

            match catalog.create(&name(), profit()).await {
                // The list refreshes because `create` published CategoryChanged, not
                // because this handler told it to.
                Ok(_) => name.set(String::new()),
                Err(failure) => error.set(Some(describe(&failure))),
            }

            busy.set(false);
        });
    };

    rsx! {
        form { class: "row", onsubmit: submit,
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
            button { r#type: "submit", disabled: busy(), "Add" }
        }
        if let Some(message) = error() {
            p { class: "error", role: "alert", "{message}" }
        }
    }
}

#[component]
fn CategoryRow(category: CategoryDto, is_admin: bool) -> Element {
    let mut renaming = use_signal(|| false);
    let mut draft = use_signal(|| category.name.clone());
    let mut error = use_signal(|| None::<String>);

    let name = category.name.clone();
    let profit = category.profit;

    let commit_rename = move |event: FormEvent| {
        event.prevent_default();
        let catalog = app_context().catalog;
        let name = name.clone();

        spawn(async move {
            match catalog.rename(&name, profit, &draft()).await {
                Ok(_) => {
                    renaming.set(false);
                    error.set(None);
                }
                Err(failure) => error.set(Some(describe(&failure))),
            }
        });
    };

    let name_for_delete = category.name.clone();
    let delete = move |_| {
        let catalog = app_context().catalog;
        let name = name_for_delete.clone();

        spawn(async move {
            if let Err(failure) = catalog.delete(&name, profit).await {
                error.set(Some(describe(&failure)));
            }
        });
    };

    rsx! {
        tr {
            td {
                if renaming() {
                    form { onsubmit: commit_rename,
                        input {
                            "aria-label": "New name for {category.name}",
                            value: "{draft}",
                            oninput: move |event| draft.set(event.value()),
                        }
                    }
                } else {
                    "{category.name}"
                }
                if let Some(message) = error() {
                    p { class: "error", role: "alert", "{message}" }
                }
            }
            td { class: "muted", if category.profit { "Income" } else { "Expense" } }
            if is_admin {
                td { class: "actions",
                    button { class: "link", onclick: move |_| renaming.toggle(),
                        if renaming() { "Cancel" } else { "Rename" }
                    }
                    button { class: "link danger", onclick: delete, "Delete" }
                }
            }
        }
    }
}
