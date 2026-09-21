//! Mobile composition root.
//!
//! The only mobile crate that names both a port and its concrete adapter. Line for line
//! it is the web root with three substitutions — the keystore instead of `sessionStorage`,
//! the device clock instead of the browser's, and a base URL that cannot be `localhost`.
//! That the rest is identical is the point: it is all the same use cases underneath.

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
use fingest_mobile_shell::Root;
use fingest_mobile_storage::{DeviceClock, KeystoreSessionStore, platform_secrets};

/// Where the API lives, as seen *from the device*.
///
/// Two rules collide here. `localhost` is the handset, not the development machine — and
/// Android blocks cleartext from API 28 except where the network security config allows it,
/// which for a dx-generated app is `127.0.0.1` and nothing else. So `http://10.0.2.2:8080`,
/// the emulator's alias for the host loopback, is reachable but *blocked*.
///
/// `adb reverse tcp:8080 tcp:8080` resolves both: the host's API appears on the device's own
/// loopback, which is the one address cleartext is permitted on. Production is HTTPS, where
/// none of this applies.
pub const DEFAULT_API_BASE_URL: &str = "http://127.0.0.1:8080";

pub fn api_base_url() -> &'static str {
    option_env!("FINGEST_API_BASE_URL").unwrap_or(DEFAULT_API_BASE_URL)
}

/// Turns whatever the capabilities endpoint did into a capability set.
///
/// Every failure resolves to "nothing enabled". A capability is only ever granted by a
/// positive answer, so no failure mode can widen what the app exposes.
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

/// The one place a lapsed token is dealt with, exactly as on web.
fn handle_expired_sessions(
    bus: &Rc<dyn EventBus>,
    auth: Rc<AuthUseCase>,
    session: Signal<Option<Session>>,
) {
    bus.subscribe(Rc::new(move |event: &ClientEvent| {
        if matches!(event, ClientEvent::SessionExpired) {
            tracing::info!("token was rejected; signing out");
            auth.on_session_expired();
            let mut session = session;
            session.set(None);
        }
    }));
}

async fn assemble() -> AppContext {
    let events: Rc<InProcessBus> = Rc::new(InProcessBus::new());

    // Android Keystore where there is one; memory everywhere else. Never a plaintext file:
    // losing the session on restart is obvious, whereas a readable token is not.
    let store = Rc::new(KeystoreSessionStore::new(platform_secrets()));

    let tokens: Rc<dyn TokenSource> = store.clone();
    let sessions: Rc<dyn SessionStore> = store;
    let bus: Rc<dyn EventBus> = events;

    let api = Rc::new(ApiClient::new(api_base_url(), tokens, bus.clone()));
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
        clock: Rc::new(DeviceClock),
        capabilities: Rc::new(capabilities),
        bus,
        session,
    }
}

/// A duplicate module name is a programming error, not a runtime condition. Log it and
/// carry on with nothing rather than failing the launch.
fn module_registry_or_empty() -> ModuleRegistry {
    modules::module_registry().unwrap_or_else(|error| {
        tracing::error!(%error, "module registry is misconfigured");
        ModuleRegistry::new()
    })
}

#[component]
pub fn App() -> Element {
    let context = use_resource(assemble);

    match &*context.read_unchecked() {
        Some(ready) => {
            use_context_provider(|| ready.clone());
            rsx! { Root {} }
        }
        None => rsx! {
            section { class: "screen centred", p { class: "muted", "Loading…" } }
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
    fn an_unreachable_server_grants_nothing() {
        assert!(resolve_capabilities(Err(ClientError::Network("offline".into()))).is_empty());
    }

    /// Android drops cleartext to anything the network security config does not name, and
    /// the generated config names only `127.0.0.1`. A plain-http default pointing anywhere
    /// else fails at runtime as a connection error, which reads exactly like the API being
    /// down — a slow thing to diagnose on a device.
    #[test]
    fn the_default_base_url_survives_androids_cleartext_policy() {
        let url = api_base_url();

        assert!(
            url.starts_with("https://") || url.starts_with("http://127.0.0.1"),
            "{url} would be blocked as cleartext; use https or 127.0.0.1 with `adb reverse`"
        );
    }
}
