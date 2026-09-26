use dioxus::prelude::*;
use fingest_client_ports::ClientEvent;
use fingest_client_view::{
    app_context,
    category::{find_category, option_value},
    format_money, use_event_refresh,
};
use fingest_client_wallets_core::{NewExpense, parse_money};
use fingest_contracts::{ExpenseDto, WalletDto};
use fingest_kernel::DateRange;

use crate::routes::Route;

/// One wallet: its expenses, its summary, and the two analytics the API exposes.
#[component]
pub fn WalletDetail(wallet_id: i32) -> Element {
    let context = app_context();
    let session_signal = context.session;

    let wallets_use_case = context.wallets.clone();
    let wallet = use_resource(move || {
        let use_case = wallets_use_case.clone();
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

    rsx! {
        match current {
            None => rsx! {
                p { class: "muted", "Loading…" }
            },
            Some(wallet) => rsx! {
                h1 { "{wallet.name}" }
                p { class: "muted", "Balance {format_money(&wallet.amount)}" }
                Link { to: Route::Wallets {}, "All wallets" }

                ExpenseForm { wallet_id, wallet: wallet.clone() }
                ExpenseList { wallet_id }
                Analytics { wallet_id }
            },
        }
    }
}

#[component]
fn ExpenseForm(wallet_id: i32, wallet: WalletDto) -> Element {
    let context = app_context();

    let catalog = context.catalog.clone();
    let categories = use_resource(move || {
        let catalog = catalog.clone();
        async move { catalog.list().await.unwrap_or_default() }
    });
    use_event_refresh(categories, |event| {
        matches!(event, ClientEvent::CategoryChanged)
    });

    let mut amount = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut date = use_signal(|| context.clock.today().to_string());
    let mut selected = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let wallet_currency = wallet.amount.currency.to_string();
    let wallet_balance = wallet.amount.clone();

    let options = categories.read_unchecked().clone().unwrap_or_default();

    let submit = {
        let options = options.clone();
        let wallet_currency = wallet_currency.clone();
        move |event: FormEvent| {
            event.prevent_default();
            if busy() {
                return;
            }

            let Some(category) = find_category(&options, &selected()) else {
                error.set(Some("Choose a category".to_owned()));
                return;
            };

            // The wallet's own currency is used, so deviation D8 cannot be triggered from
            // this form at all — the check in the use case is the backstop, not the gate.
            let money = match parse_money(&amount(), &wallet_currency) {
                Ok(money) => money,
                Err(failure) => {
                    error.set(Some(failure.message().to_owned()));
                    return;
                }
            };

            let Ok(parsed_date) = date().parse() else {
                error.set(Some("The date is invalid".to_owned()));
                return;
            };

            let balance = wallet_balance.clone();
            let context = app_context();
            spawn(async move {
                busy.set(true);
                error.set(None);

                let session = context.session.read().clone();
                let Some(session) = session else { return };
                let login = session.login().to_owned();

                let result = context
                    .wallets
                    .record_expense(
                        &session,
                        &login,
                        wallet_id,
                        &balance,
                        NewExpense {
                            amount: money,
                            date: parsed_date,
                            description: description(),
                            category,
                        },
                    )
                    .await;

                match result {
                    Ok(_) => {
                        amount.set(String::new());
                        description.set(String::new());
                    }
                    Err(failure) => error.set(Some(failure.message().to_owned())),
                }

                busy.set(false);
            });
        }
    };

    rsx! {
        h2 { "Record an entry" }
        form { class: "row", onsubmit: submit,
            input {
                "aria-label": "Amount",
                placeholder: "0.00",
                inputmode: "decimal",
                value: "{amount}",
                oninput: move |event| amount.set(event.value()),
            }
            span { class: "muted", "{wallet_currency}" }
            input {
                "aria-label": "Description",
                placeholder: "Description",
                value: "{description}",
                oninput: move |event| description.set(event.value()),
            }
            input {
                "aria-label": "Date",
                r#type: "date",
                value: "{date}",
                oninput: move |event| date.set(event.value()),
            }
            select {
                "aria-label": "Category",
                value: "{selected}",
                onchange: move |event| selected.set(event.value()),
                option { value: "", "Category…" }
                for category in options.clone() {
                    option {
                        key: "{category.name}-{category.profit}",
                        value: "{option_value(&category)}",
                        "{category.name}"
                        if category.profit { " (income)" }
                    }
                }
            }
            button { r#type: "submit", disabled: busy(), "Record" }
        }
        if let Some(message) = error() {
            p { class: "error", role: "alert", "{message}" }
        }
    }
}

#[component]
fn ExpenseList(wallet_id: i32) -> Element {
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
        h2 { "Entries" }
        match &*expenses.read_unchecked() {
            None => rsx! { p { class: "muted", "Loading…" } },
            Some(Err(error)) => rsx! {
                p { class: "error", role: "alert", "{error.message()}" }
            },
            Some(Ok(list)) if list.is_empty() => rsx! {
                p { class: "muted", "Nothing recorded yet." }
            },
            Some(Ok(list)) => rsx! {
                table {
                    thead {
                        tr {
                            th { "Date" }
                            th { "Description" }
                            th { "Category" }
                            th { "Amount" }
                            th { "" }
                        }
                    }
                    tbody {
                        for expense in list.clone() {
                            ExpenseRow { key: "{expense.id:?}", wallet_id, expense }
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn ExpenseRow(wallet_id: i32, expense: ExpenseDto) -> Element {
    let mut error = use_signal(|| None::<String>);

    let Some(expense_id) = expense.id else {
        return rsx! {
            tr {
                td { class: "muted", "{expense.date}" }
                td { "{expense.description}" }
                td { class: "muted", "{expense.category.name}" }
                td { "{format_money(&expense.amount)}" }
                td {}
            }
        };
    };

    let remove = move |_| {
        let context = app_context();
        spawn(async move {
            let session = context.session.read().clone();
            let Some(session) = session else { return };
            let login = session.login().to_owned();

            // The server returns the amount to the balance; the event makes the header
            // above follow without this row knowing the header exists.
            if let Err(failure) = context
                .wallets
                .delete_expense(&session, &login, wallet_id, expense_id)
                .await
            {
                error.set(Some(failure.message().to_owned()));
            }
        });
    };

    rsx! {
        tr {
            td { class: "muted", "{expense.date}" }
            td {
                "{expense.description}"
                if let Some(message) = error() {
                    p { class: "error", role: "alert", "{message}" }
                }
            }
            td { class: "muted",
                "{expense.category.name}"
                if expense.category.profit { span { class: "tag", "income" } }
            }
            td { "{format_money(&expense.amount)}" }
            td { class: "actions",
                button { class: "link danger", onclick: remove, "Delete" }
            }
        }
    }
}

/// Deviations D5 and D6: v1 returned `{}` for counted categories and ignored the range in
/// the summary. Both now carry real figures.
#[component]
fn Analytics(wallet_id: i32) -> Element {
    let context = app_context();
    let session_signal = context.session;

    let use_case = context.wallets.clone();
    let analytics = use_resource(move || {
        let use_case = use_case.clone();
        let session = session_signal.read().clone();
        async move {
            let session = session?;
            let login = session.login().to_owned();
            let range = DateRange::new(None, None);

            let counts = use_case
                .counted_categories(&session, &login, wallet_id, &range)
                .await
                .ok()?;
            let highest = use_case
                .highest_expense(&session, &login, wallet_id, &range)
                .await
                .ok()?;

            Some((counts, highest))
        }
    });

    use_event_refresh(analytics, |event| {
        matches!(event, ClientEvent::ExpenseChanged { .. })
    });

    let loaded = analytics.read_unchecked().clone().flatten();

    rsx! {
        h2 { "Breakdown" }
        match loaded {
            None => rsx! { p { class: "muted", "Loading…" } },
            Some((counts, highest)) => {
                let mut rows: Vec<(String, i64)> = counts.into_iter().collect();
                rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

                rsx! {
                    if let Some(expense) = highest {
                        p { class: "muted",
                            "Largest entry: {expense.description} — {format_money(&expense.amount)}"
                        }
                    }
                    if rows.is_empty() {
                        p { class: "muted", "No entries to break down." }
                    } else {
                        ul {
                            for (name, count) in rows {
                                li { key: "{name}", "{name}" span { class: "tag", "{count}" } }
                            }
                        }
                    }
                }
            },
        }
    }
}
