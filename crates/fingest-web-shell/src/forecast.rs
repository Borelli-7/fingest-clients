use dioxus::prelude::*;
use fingest_client_view::app_context;

use crate::not_found::NotFound;

#[component]
pub fn Forecast() -> Element {
    let context = app_context();

    if !context
        .modules
        .is_enabled("forecast", &context.capabilities)
    {
        return rsx! { NotFound { segments: vec!["forecast".to_owned()] } };
    }

    rsx! {
        main {
            h1 { "Forecast" }
            p { class: "muted", "Capability-gated module is enabled for this account." }
        }
    }
}
