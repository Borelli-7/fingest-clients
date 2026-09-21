use dioxus::prelude::*;

use crate::routes::Route;

#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    let path = segments.join("/");

    rsx! {
        main { class: "centered",
            div { class: "card",
                h1 { "Not found" }
                p { class: "muted", "/{path}" }
                Link { to: Route::Home {}, "Go back" }
            }
        }
    }
}
