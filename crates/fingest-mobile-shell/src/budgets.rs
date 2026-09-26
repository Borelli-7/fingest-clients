use dioxus::prelude::*;
use fingest_client_ports::{BudgetFilter, ClientError, ClientEvent};
use fingest_client_view::{app_context, describe, format_money, use_event_refresh};
use fingest_contracts::BudgetOutputDto;

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

/// Budgets as progress cards rather than the web client's six-column table.
///
/// `spent` and `left` come from the server; a bar is the one thing a phone can show that a
/// table cannot do legibly at this width.
#[component]
pub fn Budgets() -> Element {
    let context = app_context();
    let session_signal = context.session;

    let use_case = context.budgets.clone();
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

    use_event_refresh(budgets, |event| {
        matches!(
            event,
            ClientEvent::BudgetChanged | ClientEvent::ExpenseChanged { .. }
        )
    });

    rsx! {
        header { class: "bar", h1 { "Budgets" } }

        match budget_screen_state(&budgets.read_unchecked()) {
            BudgetScreenState::Loading => rsx! { p { class: "muted", "Loading…" } },
            BudgetScreenState::Error(error) => rsx! {
                p { class: "error", role: "alert", "{describe(error)}" }
            },
            BudgetScreenState::Empty => rsx! {
                p { class: "muted", "No budgets yet." }
            },
            BudgetScreenState::Ready(list) => rsx! {
                ul { class: "cards",
                    for budget in list.iter().cloned() {
                        BudgetCard { key: "{budget.id:?}", budget }
                    }
                }
            },
        }
    }
}

#[component]
fn BudgetCard(budget: BudgetOutputDto) -> Element {
    let mut error = use_signal(|| None::<String>);

    let Some(budget_id) = budget.id else {
        return rsx! {
            li { class: "card",
                div { class: "card-link",
                    span { class: "card-title", "{budget.category.name}" }
                    span { class: "card-value", "{format_money(&budget.left)} left" }
                }
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
        li { class: "card",
            div { class: "card-link",
                span { class: "card-title", "{budget.category.name}" }
                span { class: "card-value", "{format_money(&budget.left)} left" }
            }
            div { class: "bar-track",
                div { class: "bar-fill", style: "width: {spent_percent(&budget)}%" }
            }
            div { class: "card-foot",
                span { class: "muted small",
                    "{format_money(&budget.spent)} of {format_money(&budget.total)}"
                }
                button { class: "link danger", onclick: remove, "Delete" }
            }
            p { class: "muted small",
                "{budget.date_range.start} → {budget.date_range.end}"
            }
            if let Some(message) = error() {
                p { class: "error", role: "alert", "{message}" }
            }
        }
    }
}

/// How much of the budget is used, clamped to 0–100 for the bar.
///
/// Overspend is real — `left` goes negative — but a bar wider than its track is a layout
/// bug, so the number is clamped for display only. The figures beside it are not.
fn spent_percent(budget: &BudgetOutputDto) -> i64 {
    use bigdecimal::ToPrimitive;

    let total = budget.total.amount.to_f64().unwrap_or(0.0);
    if total <= 0.0 {
        return 0;
    }
    let spent = budget.spent.amount.to_f64().unwrap_or(0.0);

    ((spent / total) * 100.0).clamp(0.0, 100.0) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;
    use fingest_kernel::{CategoryRef, Currency, DateRange, Money};
    use std::str::FromStr;

    fn money(amount: &str) -> Money {
        Money::new(BigDecimal::from_str(amount).unwrap(), Currency::default())
    }

    fn budget(total: &str, spent: &str, left: &str) -> BudgetOutputDto {
        BudgetOutputDto {
            id: Some(1),
            category: CategoryRef {
                name: "Food".into(),
                profit: false,
            },
            total: money(total),
            date_range: DateRange::new(None, None),
            spent: money(spent),
            left: money(left),
        }
    }

    #[test]
    fn an_untouched_budget_shows_nothing_spent() {
        assert_eq!(spent_percent(&budget("500", "0", "500")), 0);
    }

    #[test]
    fn a_half_spent_budget_shows_half() {
        assert_eq!(spent_percent(&budget("500", "250", "250")), 50);
    }

    /// Overspend is real, but a bar wider than its track is not.
    #[test]
    fn overspend_clamps_the_bar_without_hiding_the_figures() {
        let over = budget("500", "750", "-250");

        assert_eq!(spent_percent(&over), 100);
        assert_eq!(format_money(&over.left), "-250.00 PLN");
    }

    /// A zero total would otherwise divide by zero.
    #[test]
    fn a_zero_total_does_not_divide_by_zero() {
        assert_eq!(spent_percent(&budget("0", "0", "0")), 0);
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
    fn ready_state_exposes_the_received_cards() {
        let snapshot = Some(Ok(vec![budget("500", "100", "400")]));
        match budget_screen_state(&snapshot) {
            BudgetScreenState::Ready(list) => assert_eq!(list.len(), 1),
            _ => panic!("expected ready state"),
        }
    }

    #[test]
    fn negative_spent_is_clamped_to_zero_for_the_bar() {
        assert_eq!(spent_percent(&budget("500", "-10", "510")), 0);
    }
}
