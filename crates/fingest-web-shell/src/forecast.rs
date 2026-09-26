use dioxus::prelude::*;
use fingest_client_planning_core::project;
use fingest_client_ports::{BudgetFilter, ClientError, ClientEvent};
use fingest_client_view::{app_context, describe, format_money, use_event_refresh};

use crate::not_found::NotFound;

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
        h1 { "Forecast" }
        p { class: "muted", "Where each running budget ends up if spending keeps its current pace." }

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
                        table {
                            thead {
                                tr {
                                    th { "Category" }
                                    th { "Day" }
                                    th { "Spent" }
                                    th { "Projected" }
                                    th { "Projected left" }
                                }
                            }
                            tbody {
                                for (budget, projection) in rows {
                                    tr { key: "{budget.id:?}",
                                        td { "{budget.category.name}" }
                                        td { class: "muted",
                                            "{projection.days_elapsed} of {projection.days_total}"
                                        }
                                        td { "{format_money(&budget.spent)}" }
                                        td { "{format_money(&projection.projected_spent)}" }
                                        td { class: if projection.overspends() { "error" } else { "" },
                                            "{format_money(&projection.projected_left)}"
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
}
