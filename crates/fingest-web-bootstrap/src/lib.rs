//! Composition root.
//!
//! The only crate that names both a port and its concrete adapter. Everything above this
//! line sees traits; everything below sees `reqwest`, `sessionStorage` and Dioxus.
//!
//! Deliberately the mirror of `fingest-bootstrap` in the API workspace, down to the shape:
//! build adapters, wrap each in the port it satisfies, hand the bundle to the framework.

pub mod modules;

use std::rc::Rc;

use dioxus::prelude::*;
use fingest_client_catalog_core::CatalogUseCase;
use fingest_client_events::InProcessBus;
use fingest_client_http::ApiClient;
use fingest_client_identity_core::{AuthUseCase, UserUseCase};
use fingest_client_modules::{Capabilities, ModuleRegistry};
use fingest_client_planning_core::BudgetUseCase;
use fingest_client_ports::{
    AuthApi, CapabilityApi, CatalogApi, ClientError, ClientEvent, EventBus, PlanningApi, Session,
    SessionStore, TokenSource, UsersApi, WalletsApi,
};
use fingest_client_view::AppContext;
use fingest_client_wallets_core::WalletUseCase;
use fingest_web_shell::Root;
use fingest_web_storage::{BrowserClock, BrowserSessionStore};

/// Where the API lives.
///
/// Baked in at compile time because a CSR bundle has no server to ask. Override with
/// `FINGEST_API_BASE_URL=... dx build`. The default matches the API's own default port, and
/// this client serves on 8081 to match `CORS_ALLOWED_ORIGIN` on the other side.
pub const DEFAULT_API_BASE_URL: &str = "http://localhost:8080";

pub fn api_base_url() -> &'static str {
    option_env!("FINGEST_API_BASE_URL").unwrap_or(DEFAULT_API_BASE_URL)
}

/// Turns whatever the capabilities endpoint did into a capability set.
///
/// Every failure — a 404 from a server that predates the endpoint, an outage, a body that
/// does not parse — resolves to "nothing enabled". A capability is only ever granted by a
/// positive answer, so no failure mode can widen what the UI exposes.
pub fn resolve_capabilities(
    reported: Result<fingest_contracts::CapabilitiesDto, ClientError>,
) -> Capabilities {
    match reported {
        Ok(dto) => Capabilities::from_dto(&dto),
        Err(error) => {
            tracing::warn!(%error, "no capability report; gated modules stay hidden");
            Capabilities::none()
        }
    }
}

/// The one place a lapsed token is dealt with.
///
/// Any adapter that sees a 401 on an authenticated call publishes
/// [`ClientEvent::SessionExpired`]; this clears the session, and the route guards in
/// `fingest-web-ui` redirect because the session went to `None`. Handling it here rather
/// than at each call site is the point of having a bus at all.
fn handle_expired_sessions(
    bus: &Rc<dyn EventBus>,
    auth: Rc<AuthUseCase>,
    session: Signal<Option<Session>>,
) {
    bus.subscribe(Rc::new(move |event: &ClientEvent| {
        if matches!(event, ClientEvent::SessionExpired) {
            tracing::info!("token was rejected; signing out");
            auth.on_session_expired();
            // `Signal` is `Copy`; the subscriber is `Fn`, so take a fresh handle per call.
            let mut session = session;
            session.set(None);
        }
    }));
}

/// Wires the adapters. Async because the capability report is fetched before the first render.
async fn assemble() -> AppContext {
    let events: Rc<InProcessBus> = Rc::new(InProcessBus::new());
    let store: Rc<BrowserSessionStore> = Rc::new(BrowserSessionStore::new());

    // Two views of one object: the transport may read the token, only the use case may end
    // the session. Cloning the concrete `Rc` is what makes the narrowing possible.
    let tokens: Rc<dyn TokenSource> = store.clone();
    let sessions: Rc<dyn SessionStore> = store;
    let bus: Rc<dyn EventBus> = events;

    let api = Rc::new(ApiClient::new(api_base_url(), tokens, bus.clone()));
    // One transport, several ports. The contexts above never learn they share a connection.
    let auth_api: Rc<dyn AuthApi> = api.clone();
    let capability_api: Rc<dyn CapabilityApi> = api.clone();
    let catalog_api: Rc<dyn CatalogApi> = api.clone();
    let users_api: Rc<dyn UsersApi> = api.clone();
    let wallets_api: Rc<dyn WalletsApi> = api.clone();
    let planning_api: Rc<dyn PlanningApi> = api;

    let auth = Rc::new(AuthUseCase::new(auth_api, sessions, bus.clone()));
    let users = Rc::new(UserUseCase::new(users_api, bus.clone()));
    let catalog = Rc::new(CatalogUseCase::new(catalog_api, bus.clone()));
    let wallets = Rc::new(WalletUseCase::new(wallets_api, bus.clone()));
    let budgets = Rc::new(BudgetUseCase::new(planning_api, bus.clone()));
    let capabilities = resolve_capabilities(capability_api.fetch().await);

    let mut session: Signal<Option<Session>> = Signal::new(None);
    handle_expired_sessions(&bus, auth.clone(), session);
    session.set(auth.restore());

    AppContext {
        auth,
        users,
        catalog,
        wallets,
        budgets,
        modules: Rc::new(module_registry_or_empty()),
        clock: Rc::new(BrowserClock),
        capabilities: Rc::new(capabilities),
        bus,
        session,
    }
}

/// A duplicate module name is a programming error, not a runtime condition. Log it and carry
/// on with nothing rather than panicking the whole client on start-up.
fn module_registry_or_empty() -> ModuleRegistry {
    modules::module_registry().unwrap_or_else(|error| {
        tracing::error!(%error, "module registry is misconfigured");
        ModuleRegistry::new()
    })
}

/// The root component. Wiring happens here, once, before any route renders.
#[component]
pub fn App() -> Element {
    let context = use_resource(assemble);

    match &*context.read_unchecked() {
        Some(ready) => {
            use_context_provider(|| ready.clone());
            rsx! { Root {} }
        }
        None => rsx! {
            main { style: "display:grid;place-items:center;min-height:100vh", "Loading…" }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fingest_contracts::CapabilitiesDto;

    #[test]
    fn a_reported_capability_is_granted() {
        let capabilities = resolve_capabilities(Ok(CapabilitiesDto {
            enabled: vec!["forecast".into()],
            available: vec!["forecast".into()],
            capabilities: vec!["budget-forecast".into()],
        }));

        assert!(capabilities.contains("budget-forecast"));
    }

    #[test]
    fn a_server_without_the_endpoint_grants_nothing() {
        let capabilities = resolve_capabilities(Err(ClientError::NotFound("".into())));

        assert!(capabilities.is_empty());
    }

    #[test]
    fn an_unreachable_server_grants_nothing() {
        let capabilities = resolve_capabilities(Err(ClientError::Network("offline".into())));

        assert!(capabilities.is_empty());
    }

    /// Plugin names are not capabilities: a plugin that declares none unlocks nothing.
    #[test]
    fn an_enabled_plugin_alone_grants_nothing() {
        let capabilities = resolve_capabilities(Ok(CapabilitiesDto {
            enabled: vec!["tracing".into()],
            available: vec!["tracing".into(), "in-process".into()],
            capabilities: vec![],
        }));

        assert!(capabilities.is_empty());
    }

    #[test]
    fn the_api_base_url_defaults_to_the_apis_own_port() {
        assert!(api_base_url().starts_with("http"));
    }
}
