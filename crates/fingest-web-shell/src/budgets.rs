use dioxus::prelude::*;
use fingest_client_planning_core::BudgetUseCase;
use fingest_client_ports::{BudgetFilter, ClientError, ClientEvent};
use fingest_client_view::{
    NOT_SIGNED_IN, app_context,
    category::{find_category, option_value, picker},
    describe, format_money, hold, use_event_refresh,
};
use fingest_client_wallets_core::parse_money;
use fingest_contracts::BudgetOutputDto;
use fingest_kernel::DateRange;
use std::rc::Rc;

enum BudgetScreenState<'a> {
    Loading,
    Error(&'a ClientError),
    Empty,
    Ready(&'a [BudgetOutputDto]),
}

fn budget_screen_state(
    snapshot: &Option<Result<Vec<BudgetOutputDto>, ClientError>>,
) -> BudgetScreenState<'_> {
    match snapshot {
        None => BudgetScreenState::Loading,
        Some(Err(error)) => BudgetScreenState::Error(error),
        Some(Ok(list)) if list.is_empty() => BudgetScreenState::Empty,
        Some(Ok(list)) => BudgetScreenState::Ready(list.as_slice()),
    }
}

fn parse_period(start: &str, end: &str) -> Result<DateRange, String> {
    let (Ok(from), Ok(to)) = (start.parse(), end.parse()) else {
        return Err("The dates are invalid".to_owned());
    };

    if to < from {
        return Err("The period end must be on or after the start".to_owned());
    }

    Ok(DateRange::new(Some(from), Some(to)))
}

/// Budgets for the signed-in account.
///
/// `spent` and `left` come from the server, which derives them from the expenses inside
/// each budget's period. Recomputing them here would be a second source of truth for money.
#[component]
pub fn Budgets() -> Element {
    let context = app_context();
    let session_signal = context.session;

    let use_case: Rc<BudgetUseCase> = context.budgets.clone();
    let budgets = use_resource(move || {
        let use_case = use_case.clone();
        let session = session_signal.read().clone();
        async move {
            let Some(session) = session else {
                return Ok(Vec::new());
            };
            let login = session.login().to_owned();
            use_case
                .list(&session, &login, &BudgetFilter::unfiltered())
                .await
        }
    });

    // An expense inside a budget's period moves `spent`, so either event makes this stale.
    use_event_refresh(budgets, |event| {
        matches!(
            event,
            ClientEvent::BudgetChanged | ClientEvent::ExpenseChanged { .. }
        )
    });

    rsx! {
        h1 { "Budgets" }
        BudgetForm {}

        match budget_screen_state(&budgets.read_unchecked()) {
            BudgetScreenState::Loading => rsx! { p { class: "muted", "Loading…" } },
            BudgetScreenState::Error(error) => rsx! {
                p { class: "error", role: "alert", "{describe(error)}" }
            },
            BudgetScreenState::Empty => rsx! {
                p { class: "muted", "No budgets yet." }
            },
            BudgetScreenState::Ready(list) => rsx! {
                table {
                    thead {
                        tr {
                            th { "Category" }
                            th { "Period" }
                            th { "Total" }
                            th { "Spent" }
                            th { "Left" }
                            th { "" }
                        }
                    }
                    tbody {
                        for budget in list.iter().cloned() {
                            BudgetRow { key: "{budget.id:?}", budget }
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn BudgetForm() -> Element {
    let context = app_context();

    let catalog = context.catalog.clone();
    let categories = use_resource(move || {
        let catalog = catalog.clone();
        async move { catalog.list().await }
    });
    use_event_refresh(categories, |event| {
        matches!(event, ClientEvent::CategoryChanged)
    });

    let mut total = use_signal(String::new);
    let mut currency = use_signal(|| "PLN".to_owned());
    let mut selected = use_signal(String::new);
    let mut start = use_signal(|| context.clock.today().to_string());
    let mut end = use_signal(|| context.clock.today().to_string());
    let mut error = use_signal(|| None::<String>);
    let busy = use_signal(|| false);

    let categories_state = picker(categories.read_unchecked().as_ref());
    let options = categories_state.options.clone();

    let submit = {
        let options = options.clone();
        move |event: FormEvent| {
            event.prevent_default();
            if busy() {
                return;
            }

            let Some(category) = find_category(&options, &selected()) else {
                error.set(Some("Choose a category".to_owned()));
                return;
            };

            let amount = match parse_money(&total(), &currency()) {
                Ok(money) => money,
                Err(failure) => {
                    error.set(Some(describe(&failure)));
                    return;
                }
            };

            let period = match parse_period(&start(), &end()) {
                Ok(period) => period,
                Err(failure) => {
                    error.set(Some(failure));
                    return;
                }
            };

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
                    .budgets
                    .create(&session, &login, category, amount, period)
                    .await;

                match result {
                    Ok(_) => total.set(String::new()),
                    Err(failure) => error.set(Some(describe(&failure))),
                }
            });
        }
    };

    rsx! {
        form { class: "row", onsubmit: submit,
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
            input {
                "aria-label": "Total",
                placeholder: "0.00",
                inputmode: "decimal",
                value: "{total}",
                oninput: move |event| total.set(event.value()),
            }
            input {
                "aria-label": "Currency",
                placeholder: "PLN",
                maxlength: 3,
                size: 3,
                value: "{currency}",
                oninput: move |event| currency.set(event.value()),
            }
            input {
                "aria-label": "Period start",
                r#type: "date",
                value: "{start}",
                oninput: move |event| start.set(event.value()),
            }
            input {
                "aria-label": "Period end",
                r#type: "date",
                value: "{end}",
                oninput: move |event| end.set(event.value()),
            }
            button { r#type: "submit", disabled: busy() || !categories_state.ready, "Add budget" }
        }
        if let Some(message) = categories_state.error {
            p { class: "error", role: "alert", "{message}" }
        }
        if let Some(message) = error() {
            p { class: "error", role: "alert", "{message}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fingest_kernel::{CategoryRef, Money};

    fn money(amount: &str) -> Money {
        parse_money(amount, "PLN").unwrap()
    }

    fn budget() -> BudgetOutputDto {
        BudgetOutputDto {
            id: Some(1),
            category: CategoryRef {
                name: "Food".into(),
                profit: false,
            },
            total: money("100"),
            date_range: DateRange::new(None, None),
            spent: money("10"),
            left: money("90"),
        }
    }

    #[test]
    fn loading_state_is_reported_before_data_arrives() {
        let snapshot: Option<Result<Vec<BudgetOutputDto>, ClientError>> = None;
        assert!(matches!(
            budget_screen_state(&snapshot),
            BudgetScreenState::Loading
        ));
    }

    #[test]
    fn empty_state_is_reported_for_an_empty_list() {
        let snapshot = Some(Ok(Vec::<BudgetOutputDto>::new()));
        assert!(matches!(
            budget_screen_state(&snapshot),
            BudgetScreenState::Empty
        ));
    }

    #[test]
    fn ready_state_exposes_the_received_rows() {
        let snapshot = Some(Ok(vec![budget()]));
        match budget_screen_state(&snapshot) {
            BudgetScreenState::Ready(list) => assert_eq!(list.len(), 1),
            _ => panic!("expected ready state"),
        }
    }

    #[test]
    fn invalid_date_strings_are_rejected_before_submit() {
        let failure = parse_period("2026-02-31", "2026-03-01").unwrap_err();
        assert_eq!(failure, "The dates are invalid");
    }

    #[test]
    fn an_inverted_period_is_rejected_before_submit() {
        let failure = parse_period("2026-03-02", "2026-03-01").unwrap_err();
        assert_eq!(failure, "The period end must be on or after the start");
    }

    #[test]
    fn a_valid_period_is_accepted() {
        let period = parse_period("2026-03-01", "2026-03-31").unwrap();
        assert_eq!(period.start.to_string(), "2026-03-01");
        assert_eq!(period.end.to_string(), "2026-03-31");
    }
}

#[component]
fn BudgetRow(budget: BudgetOutputDto) -> Element {
    let mut error = use_signal(|| None::<String>);

    let Some(budget_id) = budget.id else {
        return rsx! {
            tr {
                td { "{budget.category.name}" }
                td { class: "muted", "{budget.date_range.start} → {budget.date_range.end}" }
                td { "{format_money(&budget.total)}" }
                td { "{format_money(&budget.spent)}" }
                td { "{format_money(&budget.left)}" }
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

            if let Err(failure) = context.budgets.delete(&session, &login, budget_id).await {
                error.set(Some(describe(&failure)));
            }
        });
    };

    rsx! {
        tr {
            td {
                "{budget.category.name}"
                if budget.category.profit { span { class: "tag", "income" } }
                if let Some(message) = error() {
                    p { class: "error", role: "alert", "{message}" }
                }
            }
            td { class: "muted", "{budget.date_range.start} → {budget.date_range.end}" }
            td { "{format_money(&budget.total)}" }
            td { "{format_money(&budget.spent)}" }
            td { "{format_money(&budget.left)}" }
            td { class: "actions",
                button { class: "link danger", onclick: remove, "Delete" }
            }
        }
    }
}
