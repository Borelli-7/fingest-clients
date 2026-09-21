use dioxus::prelude::*;
use fingest_client_ports::ClientEvent;
use fingest_client_view::{app_context, describe, format_money, use_event_refresh};
use fingest_contracts::{ExpenseDto, WalletDto};
use fingest_kernel::DateRange;

use crate::{entry_sheet::EntrySheet, routes::Route};

/// Wallets as cards, not a table. A table does not survive 390px.
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
        header { class: "bar", h1 { "Wallets" } }

        match &*wallets.read_unchecked() {
            None => rsx! { p { class: "muted", "Loading…" } },
            Some(Err(error)) => rsx! {
                p { class: "error", role: "alert", "{describe(error)}" }
            },
            Some(Ok(list)) if list.is_empty() => rsx! {
                p { class: "muted", "No wallets yet." }
            },
            Some(Ok(list)) => rsx! {
                ul { class: "cards",
                    for wallet in list.clone() {
                        WalletCard { key: "{wallet.id:?}", wallet }
                    }
                }
            },
        }
    }
}

#[component]
fn WalletCard(wallet: WalletDto) -> Element {
    let wallet_id = wallet.id.unwrap_or_default();

    rsx! {
        li { class: "card",
            Link { class: "card-link", to: Route::WalletDetail { wallet_id },
                span { class: "card-title", "{wallet.name}" }
                span { class: "card-value", "{format_money(&wallet.amount)}" }
            }
        }
    }
}

/// One wallet: balance, then its entries.
///
/// The web client puts the entry form inline above the list; here it belongs in a sheet
/// that the keyboard can push up without hiding the fields. That sheet lands in M4 — this
/// screen is the read-only half the walking skeleton needs.
#[component]
pub fn WalletDetail(wallet_id: i32) -> Element {
    let context = app_context();
    let session_signal = context.session;

    let use_case = context.wallets.clone();
    let wallet = use_resource(move || {
        let use_case = use_case.clone();
        let session = session_signal.read().clone();
        async move {
            let session = session?;
            let login = session.login().to_owned();
            use_case
                .list(&session, &login)
                .await
                .ok()?
                .into_iter()
                .find(|w| w.id == Some(wallet_id))
        }
    });

    use_event_refresh(wallet, |event| {
        matches!(
            event,
            ClientEvent::WalletChanged | ClientEvent::ExpenseChanged { .. }
        )
    });

    let current = wallet.read_unchecked().clone().flatten();
    let sheet_open = use_signal(|| false);

    rsx! {
        header { class: "bar",
            Link { class: "back", to: Route::Wallets {}, "‹" }
            match &current {
                Some(wallet) => rsx! { h1 { "{wallet.name}" } },
                None => rsx! { h1 { "…" } },
            }
        }

        match current {
            None => rsx! { p { class: "muted", "Loading…" } },
            Some(wallet) => rsx! {
                p { class: "balance", "{format_money(&wallet.amount)}" }
                Entries { wallet_id }
                button {
                    class: "fab",
                    "aria-label": "Record an entry",
                    onclick: move |_| {
                        let mut sheet_open = sheet_open;
                        sheet_open.set(true);
                    },
                    "+"
                }
                EntrySheet { wallet: wallet.clone(), open: sheet_open }
            },
        }
    }
}

#[component]
fn Entries(wallet_id: i32) -> Element {
    let context = app_context();
    let session_signal = context.session;

    let use_case = context.wallets.clone();
    let expenses = use_resource(move || {
        let use_case = use_case.clone();
        let session = session_signal.read().clone();
        async move {
            let Some(session) = session else {
                return Ok(Vec::new());
            };
            let login = session.login().to_owned();
            use_case
                .list_expenses(&session, &login, wallet_id, &DateRange::new(None, None))
                .await
        }
    });

    use_event_refresh(expenses, |event| {
        matches!(event, ClientEvent::ExpenseChanged { .. })
    });

    rsx! {
        match &*expenses.read_unchecked() {
            None => rsx! { p { class: "muted", "Loading…" } },
            Some(Err(error)) => rsx! {
                p { class: "error", role: "alert", "{describe(error)}" }
            },
            Some(Ok(list)) if list.is_empty() => rsx! {
                p { class: "muted", "Nothing recorded yet." }
            },
            Some(Ok(list)) => rsx! {
                ul { class: "cards",
                    for expense in list.clone() {
                        EntryRow { key: "{expense.id:?}", wallet_id, expense }
                    }
                }
            },
        }
    }
}

#[component]
fn EntryRow(wallet_id: i32, expense: ExpenseDto) -> Element {
    let mut error = use_signal(|| None::<String>);
    let expense_id = expense.id.unwrap_or_default();

    let remove = move |_| {
        let context = app_context();
        spawn(async move {
            let session = context.session.read().clone();
            let Some(session) = session else { return };
            let login = session.login().to_owned();

            // The server returns the amount to the balance; the event is what makes the
            // header above follow, without this row knowing the header exists.
            if let Err(failure) = context
                .wallets
                .delete_expense(&session, &login, wallet_id, expense_id)
                .await
            {
                error.set(Some(describe(&failure)));
            }
        });
    };

    rsx! {
        li { class: "card",
            div { class: "card-link",
                span { class: "card-title", "{expense.description}" }
                span { class: "card-value", "{format_money(&expense.amount)}" }
            }
            div { class: "card-foot",
                span { class: "muted small",
                    "{expense.date} · {expense.category.name}"
                    if expense.category.profit { " · income" }
                }
                button { class: "link danger", onclick: remove, "Delete" }
            }
            if let Some(message) = error() {
                p { class: "error", role: "alert", "{message}" }
            }
        }
    }
}
