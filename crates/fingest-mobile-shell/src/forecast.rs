use dioxus::prelude::*;
use fingest_client_planning_core::project;
use fingest_client_ports::{BudgetFilter, ClientError, ClientEvent};
use fingest_client_view::{app_context, describe, format_money, use_event_refresh};

use crate::routes::NotFound;

/// Budgets whose period is running, extended at their current pace to the period's end.
#[component]
pub fn Forecast() -> Element {
    let context = app_context();
    let session_signal = context.session;

    let use_case = context.budgets.clone();
    let budgets = use_resource(move || {
        let use_case = use_case.clone();
        let session = session_signal.read().clone();
        async move {
            let Some(session) = session else {
                return Err(ClientError::Unauthenticated(
                    "You are not signed in".to_owned(),
                ));
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

    if !context
        .modules
        .is_enabled("forecast", &context.capabilities)
    {
        return rsx! { NotFound { segments: vec!["forecast".to_owned()] } };
    }

    let today = context.clock.today();

    rsx! {
        header { class: "bar", h1 { "Forecast" } }

        match &*budgets.read_unchecked() {
            None => rsx! { p { class: "muted", "Loading…" } },
            Some(Err(error)) => rsx! {
                p { class: "error", role: "alert", "{describe(error)}" }
            },
            Some(Ok(list)) => {
                let rows: Vec<_> = list
                    .iter()
                    .filter_map(|budget| project(budget, today).map(|p| (budget.clone(), p)))
                    .collect();

                if rows.is_empty() {
                    rsx! { p { class: "muted", "No budget period is running today." } }
                } else {
                    rsx! {
                        ul { class: "cards",
                            for (budget, projection) in rows {
                                li { key: "{budget.id:?}", class: "card",
                                    div { class: "card-link",
                                        span { class: "card-title", "{budget.category.name}" }
                                        span { class: if projection.overspends() { "card-value error" } else { "card-value" },
                                            "{format_money(&projection.projected_left)} left"
                                        }
                                    }
                                    p { class: "muted small",
                                        "Day {projection.days_elapsed} of {projection.days_total} · "
                                        "{format_money(&projection.projected_spent)} projected of "
                                        "{format_money(&budget.total)}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
