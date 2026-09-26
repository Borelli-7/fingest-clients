use dioxus::prelude::*;
use fingest_client_ports::{ClientEvent, NameField};
use fingest_client_view::{app_context, describe, use_event_refresh};
use fingest_contracts::UserDto;

use crate::routes::Route;

/// The account roster. Admin-only, because the server checks self-or-admin *and* admin,
/// which only an admin satisfies.
#[component]
pub fn Users() -> Element {
    let context = app_context();
    let navigator = use_navigator();

    let session_signal = context.session;
    let users_use_case = context.users.clone();
    let accounts = use_resource(move || {
        let users = users_use_case.clone();
        let session = session_signal.read().clone();
        async move {
            match session {
                Some(session) => users.list(&session).await,
                None => Ok(Vec::new()),
            }
        }
    });

    use_event_refresh(accounts, |event| {
        matches!(
            event,
            ClientEvent::AccountChanged { .. } | ClientEvent::AccountRemoved { .. }
        )
    });

    // A non-admin is not shown a refusal; the route simply is not theirs.
    if !context.is_admin() {
        return rsx! {
            h1 { "Accounts" }
            p { class: "muted", "Administrator access is required to list accounts." }
            Link { to: Route::Home {}, "Go back" }
        };
    }

    let sign_out_after_self_delete = move |login: String| {
        let context = app_context();
        let is_self = context
            .session
            .read()
            .as_ref()
            .is_some_and(|session| session.login() == login);

        if is_self {
            let mut session = context.session;
            context.auth.logout();
            session.set(None);
            navigator.replace(Route::Login {});
        }
    };

    rsx! {
        h1 { "Accounts" }

        match &*accounts.read_unchecked() {
            None => rsx! { p { class: "muted", "Loading…" } },
            Some(Err(error)) => rsx! {
                p { class: "error", role: "alert", "{describe(error)}" }
            },
            Some(Ok(list)) => rsx! {
                table {
                    thead {
                        tr {
                            th { "Login" }
                            th { "First name" }
                            th { "Last name" }
                            th { "Role" }
                            th { "" }
                        }
                    }
                    tbody {
                        for account in list.clone() {
                            UserRow {
                                key: "{account.login}",
                                account,
                                on_deleted: sign_out_after_self_delete,
                            }
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn UserRow(account: UserDto, on_deleted: EventHandler<String>) -> Element {
    let mut error = use_signal(|| None::<String>);

    rsx! {
        tr {
            td { "{account.login}" }
            td {
                EditableName {
                    login: account.login.clone(),
                    field: NameField::First,
                    current: account.first_name.clone(),
                }
            }
            td {
                EditableName {
                    login: account.login.clone(),
                    field: NameField::Last,
                    current: account.last_name.clone(),
                }
            }
            td { class: "muted", if account.admin { "Admin" } else { "User" } }
            td { class: "actions",
                button {
                    class: "link danger",
                    onclick: {
                        let login = account.login.clone();
                        move |_| {
                            let context = app_context();
                            let login = login.clone();
                            spawn(async move {
                                let session = context.session.read().clone();
                                let Some(session) = session else { return };

                                match context.users.delete(&session, &login).await {
                                    Ok(()) => on_deleted.call(login),
                                    Err(failure) => error.set(Some(describe(&failure))),
                                }
                            });
                        }
                    },
                    "Delete"
                }
                if let Some(message) = error() {
                    p { class: "error", role: "alert", "{message}" }
                }
            }
        }
    }
}

#[component]
fn EditableName(login: String, field: NameField, current: Option<String>) -> Element {
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(|| current.clone().unwrap_or_default());
    let mut error = use_signal(|| None::<String>);

    let labelled = login.clone();
    let submit = move |event: FormEvent| {
        event.prevent_default();
        let context = app_context();
        let login = login.clone();

        spawn(async move {
            let session = context.session.read().clone();
            let Some(session) = session else { return };

            match context
                .users
                .update_name(&session, &login, field, &draft())
                .await
            {
                Ok(()) => {
                    editing.set(false);
                    error.set(None);
                }
                Err(failure) => error.set(Some(describe(&failure))),
            }
        });
    };

    if !editing() {
        return rsx! {
            button {
                class: "link",
                onclick: move |_| editing.set(true),
                match &current {
                    Some(value) if !value.is_empty() => value.clone(),
                    _ => "—".to_owned(),
                }
            }
        };
    }

    rsx! {
        form { onsubmit: submit,
            input {
                "aria-label": "{field.label()} for {labelled}",
                value: "{draft}",
                oninput: move |event| draft.set(event.value()),
            }
            button { class: "link", r#type: "submit", "Save" }
            button { class: "link", r#type: "button", onclick: move |_| editing.set(false), "Cancel" }
        }
        if let Some(message) = error() {
            p { class: "error", role: "alert", "{message}" }
        }
    }
}
