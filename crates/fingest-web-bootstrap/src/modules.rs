use fingest_client_modules::{ModuleDescriptor, ModuleError, ModuleRegistry, NavEntry};

/// Every feature module compiled into this client.
///
/// The counterpart of `plugin_host()` in `fingest-bootstrap`: registration is compile-time,
/// and the server's reported capabilities decide which of these a session can reach.
///
/// Core modules carry no capability. Gating them would let a capabilities outage blank the
/// application, which is the opposite of failing closed.
pub fn module_registry() -> Result<ModuleRegistry, ModuleError> {
    let mut registry = ModuleRegistry::new();

    registry.register(ModuleDescriptor::core(
        "home",
        Some(NavEntry {
            label: "Home",
            path: "/",
        }),
    ))?;

    registry.register(ModuleDescriptor::core(
        "wallets",
        Some(NavEntry {
            label: "Wallets",
            path: "/wallets",
        }),
    ))?;

    registry.register(ModuleDescriptor::core(
        "categories",
        Some(NavEntry {
            label: "Categories",
            path: "/categories",
        }),
    ))?;

    registry.register(ModuleDescriptor::core(
        "budgets",
        Some(NavEntry {
            label: "Budgets",
            path: "/budgets",
        }),
    ))?;

    // No nav entry: the roster is admin-only, and the page says so rather than the bar
    // advertising a route most accounts cannot use.
    registry.register(ModuleDescriptor::core("accounts", None))?;

    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fingest_client_modules::Capabilities;

    #[test]
    fn the_registry_builds_without_duplicates() {
        assert!(module_registry().is_ok());
    }

    #[test]
    fn core_modules_survive_a_capabilities_outage() {
        let registry = module_registry().unwrap();

        assert_eq!(
            registry.enabled(&Capabilities::none()).len(),
            registry.all().len()
        );
    }
}
