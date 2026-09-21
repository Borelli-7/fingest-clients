use std::rc::Rc;

use dioxus::prelude::*;
use fingest_client_catalog_core::CatalogUseCase;
use fingest_client_identity_core::{AuthUseCase, UserUseCase};
use fingest_client_modules::{Capabilities, ModuleRegistry};
use fingest_client_planning_core::BudgetUseCase;
use fingest_client_ports::{ClientEvent, Clock, EventBus, Session};
use fingest_client_wallets_core::WalletUseCase;

/// Everything a component is allowed to reach for.
///
/// This is the seam: each shell's composition root puts one of these in context, and no
/// component ever constructs an adapter. It is the client-side counterpart of
/// `Dependencies` in `fingest-bootstrap`, which registers `web::Data` for the same reason.
///
/// Shared rather than per-platform because none of these fields is platform-specific —
/// the web and mobile shells differ in what they *render*, not in what they may reach.
#[derive(Clone)]
pub struct AppContext {
    pub auth: Rc<AuthUseCase>,
    pub users: Rc<UserUseCase>,
    pub catalog: Rc<CatalogUseCase>,
    pub wallets: Rc<WalletUseCase>,
    pub budgets: Rc<BudgetUseCase>,
    pub modules: Rc<ModuleRegistry>,

    /// A port, not `chrono::Local` directly: date defaults stay testable.
    pub clock: Rc<dyn Clock>,

    /// Resolved once at boot. Empty when the server could not be asked — fail closed.
    pub capabilities: Rc<Capabilities>,

    /// Exposed so a view can react to something it did not cause.
    pub bus: Rc<dyn EventBus>,

    /// The live session. `None` means signed out; every guard reads this.
    pub session: Signal<Option<Session>>,
}

impl AppContext {
    pub fn is_signed_in(&self) -> bool {
        self.session.read().is_some()
    }

    pub fn is_admin(&self) -> bool {
        self.session.read().as_ref().is_some_and(Session::is_admin)
    }

    /// The login whose data a screen is showing.
    pub fn current_login(&self) -> Option<String> {
        self.session
            .read()
            .as_ref()
            .map(|session| session.login().to_owned())
    }
}

/// Reads the context. Panics only on a wiring bug, which is not a runtime condition to
/// handle — the root provides it before any route renders.
pub fn app_context() -> AppContext {
    use_context::<AppContext>()
}

/// Refetches `resource` whenever a matching event is published.
///
/// This is what the bus is for: the category form does not know the category list exists,
/// and neither knows about the account list. Each view declares what would make its data
/// stale, and is refreshed by whoever makes it so.
pub fn use_event_refresh<T: 'static>(resource: Resource<T>, matches: fn(&ClientEvent) -> bool) {
    let bus = app_context().bus;

    let id = use_hook({
        let bus = bus.clone();
        move || {
            bus.subscribe(Rc::new(move |event: &ClientEvent| {
                if matches(event) {
                    // `Resource` is `Copy`; the subscriber is `Fn`, so take a fresh handle.
                    let mut resource = resource;
                    resource.restart();
                }
            }))
        }
    });

    // Not a Drop guard inside `use_hook`: that hook hands back a *clone* on every render,
    // and each clone's drop would cancel the subscription the first time the view redrew.
    use_drop(move || bus.unsubscribe(id));
}
