use dioxus::prelude::*;
use fingest_client_ports::ClientEvent;
use fingest_client_view::{app_context, format_money, use_event_refresh};
use fingest_client_wallets_core::parse_money;
use fingest_contracts::WalletDto;

use crate::routes::Route;

/// The signed-in account's wallets.
///
/// Scoped to the session's own login. An admin acting on another account is supported by
/// the use case and the server, but no UI exposes it yet.
#[component]
pub fn Wallets() -> Element {
    let context = app_context();
    let session_signal = context.session;

    let use_case = context.wallets.clone();
    let wallets = use_resource(move || {
        let use_case = use_case.clone();
        let session = session_signal.read().clone();
        async move {
            let Some(session) = session else {
                return Ok(Vec::new());
            };
            let login = session.login().to_owned();
            use_case.list(&session, &login).await
        }
    });

    // An expense changes a balance, so this list is stale for either reason.
    use_event_refresh(wallets, |event| {
        matches!(
            event,
            ClientEvent::WalletChanged | ClientEvent::ExpenseChanged { .. }
        )
    });

    rsx! {
        h1 { "Wallets" }
        WalletForm {}

        match &*wallets.read_unchecked() {
            None => rsx! { p { class: "muted", "Loading…" } },
            Some(Err(error)) => rsx! {
                p { class: "error", role: "alert", "{error.message()}" }
            },
            Some(Ok(list)) if list.is_empty() => rsx! {
                p { class: "muted", "No wallets yet. Add one above." }
            },
            Some(Ok(list)) => rsx! {
                table {
                    thead {
                        tr {
                            th { "Name" }
                            th { "Balance" }
                            th { "" }
                        }
                    }
                    tbody {
                        for wallet in list.clone() {
                            WalletRow { key: "{wallet.id:?}", wallet }
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn WalletForm() -> Element {
    let mut name = use_signal(String::new);
    let mut amount = use_signal(String::new);
    let mut currency = use_signal(|| "PLN".to_owned());
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }

        // Parsed with the kernel's own types, so a bad currency or a negative figure is
        // refused here on exactly the rule the server would apply.
        let money = match parse_money(&amount(), &currency()) {
            Ok(money) => money,
            Err(failure) => {
                error.set(Some(failure.message().to_owned()));
                return;
            }
        };

        let context = app_context();
        spawn(async move {
            busy.set(true);
            error.set(None);

            let session = context.session.read().clone();
            let Some(session) = session else { return };
            let login = session.login().to_owned();

            match context
                .wallets
                .create(&session, &login, &name(), money)
                .await
            {
                Ok(_) => {
                    name.set(String::new());
                    amount.set(String::new());
                }
                Err(failure) => error.set(Some(failure.message().to_owned())),
            }

            busy.set(false);
        });
    };

    rsx! {
        form { class: "row", onsubmit: submit,
            input {
                "aria-label": "Wallet name",
                placeholder: "Wallet name",
                value: "{name}",
                oninput: move |event| name.set(event.value()),
            }
            input {
                "aria-label": "Opening balance",
                placeholder: "0.00",
                inputmode: "decimal",
                value: "{amount}",
                oninput: move |event| amount.set(event.value()),
            }
            input {
                "aria-label": "Currency",
                placeholder: "PLN",
                maxlength: 3,
                size: 3,
                value: "{currency}",
                oninput: move |event| currency.set(event.value()),
            }
            button { r#type: "submit", disabled: busy(), "Add wallet" }
        }
        if let Some(message) = error() {
            p { class: "error", role: "alert", "{message}" }
        }
    }
}

#[component]
fn WalletRow(wallet: WalletDto) -> Element {
    let mut renaming = use_signal(|| false);
    let mut draft = use_signal(|| wallet.name.clone());
    let mut error = use_signal(|| None::<String>);

    // Without an id nothing on the server can be addressed, so nothing is offered.
    let Some(wallet_id) = wallet.id else {
        return rsx! {
            tr {
                td { "{wallet.name}" }
                td { "{format_money(&wallet.amount)}" }
                td {}
            }
        };
    };

    let commit = move |event: FormEvent| {
        event.prevent_default();
        let context = app_context();

        spawn(async move {
            let session = context.session.read().clone();
            let Some(session) = session else { return };
            let login = session.login().to_owned();

            match context
                .wallets
                .rename(&session, &login, wallet_id, &draft())
                .await
            {
                Ok(_) => {
                    renaming.set(false);
                    error.set(None);
                }
                Err(failure) => error.set(Some(failure.message().to_owned())),
            }
        });
    };

    let remove = move |_| {
        let context = app_context();
        spawn(async move {
            let session = context.session.read().clone();
            let Some(session) = session else { return };
            let login = session.login().to_owned();

            if let Err(failure) = context.wallets.delete(&session, &login, wallet_id).await {
                error.set(Some(failure.message().to_owned()));
            }
        });
    };

    rsx! {
        tr {
            td {
                if renaming() {
                    form { onsubmit: commit,
                        input {
                            "aria-label": "New name for {wallet.name}",
                            value: "{draft}",
                            oninput: move |event| draft.set(event.value()),
                        }
                    }
                } else {
                    Link { to: Route::WalletDetail { wallet_id }, "{wallet.name}" }
                }
                if let Some(message) = error() {
                    p { class: "error", role: "alert", "{message}" }
                }
            }
            td { "{format_money(&wallet.amount)}" }
            td { class: "actions",
                button { class: "link", onclick: move |_| renaming.toggle(),
                    if renaming() { "Cancel" } else { "Rename" }
                }
                button { class: "link danger", onclick: remove, "Delete" }
            }
        }
    }
}
