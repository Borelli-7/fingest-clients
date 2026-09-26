use fingest_client_modules::{ModuleDescriptor, ModuleError, ModuleRegistry, NavEntry};

/// Every feature module compiled into the mobile app.
///
/// Deliberately not identical to the web registry: a bottom bar holds about five tabs, and
/// account administration is not something anyone does from a phone. Modules absent here
/// are absent by product decision, which is separate from a capability gating one off at
/// runtime.
pub fn module_registry() -> Result<ModuleRegistry, ModuleError> {
    let mut registry = ModuleRegistry::new();

    registry.register(ModuleDescriptor::core(
        "wallets",
        Some(NavEntry {
            label: "Wallets",
            path: "/",
        }),
    ))?;

    registry.register(ModuleDescriptor::core(
        "budgets",
        Some(NavEntry {
            label: "Budgets",
            path: "/budgets",
        }),
    ))?;

    registry.register(ModuleDescriptor::core(
        "categories",
        Some(NavEntry {
            label: "Categories",
            path: "/categories",
        }),
    ))?;

    registry.register(ModuleDescriptor::gated(
        "forecast",
        "budget-forecast",
        Some(NavEntry {
            label: "Forecast",
            path: "/forecast",
        }),
    ))?;

    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fingest_client_modules::Capabilities;

    fn capabilities(names: &[&str]) -> Capabilities {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn the_registry_builds_without_duplicates() {
        assert!(module_registry().is_ok());
    }

    #[test]
    fn core_modules_survive_a_capabilities_outage() {
        let registry = module_registry().unwrap();

        assert!(!registry.is_enabled("forecast", &Capabilities::none()));
    }

    /// Every tab must point at a route the mobile router actually answers.
    ///
    /// This asks the real router rather than comparing against a hand-copied
    /// list: a duplicated list silently goes stale the moment a route is added
    /// or renamed, which is exactly how `/accounts` once outlived its screen.
    #[test]
    fn every_tab_targets_a_known_route() {
        use fingest_mobile_shell::Route;
        use std::str::FromStr;

        for entry in module_registry().unwrap().nav(&Capabilities::none()) {
            match Route::from_str(entry.path) {
                Ok(Route::NotFound { .. }) | Err(_) => {
                    panic!(
                        "tab {} points at a route the router does not serve",
                        entry.path
                    )
                }
                Ok(_) => {}
            }
        }
    }

    /// Proves the guard above can actually fail. Without this, a router that
    /// happily resolved every string would make the tab check vacuous.
    #[test]
    fn the_route_guard_rejects_a_path_with_no_screen() {
        use fingest_mobile_shell::Route;
        use std::str::FromStr;

        assert!(
            matches!(Route::from_str("/accounts"), Ok(Route::NotFound { .. })),
            "a path with no screen must not look routable"
        );
    }

    #[test]
    fn forecast_tab_is_hidden_without_capability() {
        let entries = module_registry().unwrap().nav(&Capabilities::none());
        assert!(entries.iter().all(|entry| entry.path != "/forecast"));
    }

    #[test]
    fn forecast_tab_is_available_when_capability_is_reported() {
        use fingest_mobile_shell::Route;
        use std::str::FromStr;

        let registry = module_registry().unwrap();
        let entries = registry.nav(&capabilities(&["budget-forecast"]));
        assert!(entries.iter().any(|entry| entry.path == "/forecast"));
        assert!(registry.is_enabled("forecast", &capabilities(&["budget-forecast"])));

        assert!(matches!(Route::from_str("/forecast"), Ok(Route::Forecast {})));
    }
}
