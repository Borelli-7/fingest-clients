use dioxus::prelude::*;
use fingest_client_ports::ClientEvent;
use fingest_client_view::{
    NOT_SIGNED_IN, app_context,
    category::{find_category, option_value, picker},
    describe, hold, use_event_refresh,
};
use fingest_client_wallets_core::{NewExpense, parse_money};
use fingest_contracts::WalletDto;

/// Recording an entry, in a sheet rather than the web client's inline row.
///
/// Five inputs side by side is unusable at 390px, and a sheet is what the on-screen
/// keyboard can push up without hiding the field being typed into.
#[component]
pub fn EntrySheet(wallet: WalletDto, open: Signal<bool>) -> Element {
    let context = app_context();

    let catalog = context.catalog.clone();
    let categories = use_resource(move || {
        let catalog = catalog.clone();
        async move { catalog.list().await }
    });
    use_event_refresh(categories, |event| {
        matches!(event, ClientEvent::CategoryChanged)
    });

    let mut amount = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut date = use_signal(|| context.clock.today().to_string());
    let mut selected = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let busy = use_signal(|| false);

    let currency = wallet.amount.currency.to_string();
    let balance = wallet.amount.clone();
    let wallet_id = wallet.id.unwrap_or_default();
    let categories_state = picker(categories.read_unchecked().as_ref());
    let options = categories_state.options.clone();

    let submit = {
        let options = options.clone();
        let currency = currency.clone();
        move |event: FormEvent| {
            event.prevent_default();
            if busy() {
                return;
            }

            let Some(category) = find_category(&options, &selected()) else {
                error.set(Some("Choose a category".to_owned()));
                return;
            };

            // The wallet's own currency, so the D8 mismatch cannot be produced from here;
            // the check inside the use case is the backstop, not the gate.
            let money = match parse_money(&amount(), &currency) {
                Ok(money) => money,
                Err(failure) => {
                    error.set(Some(describe(&failure)));
                    return;
                }
            };

            let Ok(parsed_date) = date().parse() else {
                error.set(Some("The date is invalid".to_owned()));
                return;
            };

            let balance = balance.clone();
            let context = app_context();
            let Some(session) = context.session.read().clone() else {
                error.set(Some(NOT_SIGNED_IN.to_owned()));
                return;
            };
            error.set(None);
            let busy_guard = hold(busy);

            spawn(async move {
                let _busy = busy_guard;
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
                        // Nothing is refreshed here: `record_expense` published
                        // ExpenseChanged and the balance, list and breakdown each
                        // subscribed to it themselves.
                        amount.set(String::new());
                        description.set(String::new());
                        selected.set(String::new());
                        open.set(false);
                    }
                    Err(failure) => error.set(Some(describe(&failure))),
                }
            });
        }
    };

    if !open() {
        return rsx! {};
    }

    rsx! {
        div { class: "scrim", onclick: move |_| open.set(false) }
        section { class: "sheet", role: "dialog", "aria-label": "Record an entry",
            div { class: "sheet-grip" }
            h2 { "Record an entry" }

            form { class: "stack", onsubmit: submit,
                if let Some(message) = error() {
                    p { class: "error", role: "alert", "{message}" }
                }
                if let Some(message) = categories_state.error {
                    p { class: "error", role: "alert", "{message}" }
                }

                div { class: "field-row",
                    input {
                        "aria-label": "Amount",
                        placeholder: "0.00",
                        inputmode: "decimal",
                        value: "{amount}",
                        oninput: move |event| amount.set(event.value()),
                    }
                    span { class: "suffix", "{currency}" }
                }

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

                button { class: "button", r#type: "submit", disabled: busy() || !categories_state.ready,
                    if busy() { "Recording…" } else { "Record" }
                }
                button {
                    class: "button ghost",
                    r#type: "button",
                    onclick: move |_| open.set(false),
                    "Cancel"
                }
            }
        }
    }
}
