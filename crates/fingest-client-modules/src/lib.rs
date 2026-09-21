//! Which feature modules this client may render.
//!
//! The mirror of `fingest-plugins` on the server: that crate decides which plugins a binary
//! runs, this one decides which UI modules a session sees. The link between them is
//! `GET /api/capabilities`.
//!
//! Everything here fails closed. A capability that was not positively reported is absent,
//! so a network failure, an old server or a typo hides a feature rather than exposing one.

use std::collections::BTreeSet;

use fingest_contracts::CapabilitiesDto;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModuleError {
    #[error("Module registered twice: {0}")]
    Duplicate(String),
}

/// What the running server told us it can do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capabilities(BTreeSet<String>);

impl Capabilities {
    /// The only correct answer when the server could not be asked.
    pub fn none() -> Self {
        Self::default()
    }

    pub fn contains(&self, capability: &str) -> bool {
        self.0.contains(capability)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Reads the declared capabilities, not the plugin names.
    ///
    /// A plugin name is an implementation detail of the server's build; a capability is the
    /// contract. Gating on the name would couple the UI to how the feature is packaged.
    pub fn from_dto(dto: &CapabilitiesDto) -> Self {
        Self(dto.capabilities.iter().cloned().collect())
    }
}

impl FromIterator<String> for Capabilities {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavEntry {
    pub label: &'static str,
    pub path: &'static str,
}

/// One feature area of the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleDescriptor {
    pub name: &'static str,

    /// `None` marks a core module — auth, wallets, budgets. These are not plugins and are
    /// always present, so gating them on a server report would let a capabilities outage
    /// blank the application.
    pub required_capability: Option<&'static str>,

    pub nav: Option<NavEntry>,
}

impl ModuleDescriptor {
    pub const fn core(name: &'static str, nav: Option<NavEntry>) -> Self {
        Self {
            name,
            required_capability: None,
            nav,
        }
    }

    pub const fn gated(
        name: &'static str,
        required_capability: &'static str,
        nav: Option<NavEntry>,
    ) -> Self {
        Self {
            name,
            required_capability: Some(required_capability),
            nav,
        }
    }

    pub fn is_enabled(&self, capabilities: &Capabilities) -> bool {
        match self.required_capability {
            None => true,
            Some(required) => capabilities.contains(required),
        }
    }
}

/// Every module compiled into this client. Capabilities select which are reachable.
#[derive(Debug, Clone, Default)]
pub struct ModuleRegistry {
    modules: Vec<ModuleDescriptor>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rejects a duplicate name rather than shadowing, mirroring `PluginHost::register`.
    pub fn register(&mut self, module: ModuleDescriptor) -> Result<(), ModuleError> {
        if self.modules.iter().any(|m| m.name == module.name) {
            return Err(ModuleError::Duplicate(module.name.to_owned()));
        }
        self.modules.push(module);
        Ok(())
    }

    pub fn all(&self) -> &[ModuleDescriptor] {
        &self.modules
    }

    pub fn enabled(&self, capabilities: &Capabilities) -> Vec<ModuleDescriptor> {
        self.modules
            .iter()
            .filter(|m| m.is_enabled(capabilities))
            .copied()
            .collect()
    }

    /// Whether a named module may render.
    ///
    /// An unregistered name is *not* enabled. Route components call this before rendering,
    /// so a deep link into a disabled module lands on the not-found view — the observable
    /// behaviour of an unregistered route, which a compile-time `Routable` enum cannot give.
    pub fn is_enabled(&self, name: &str, capabilities: &Capabilities) -> bool {
        self.modules
            .iter()
            .find(|m| m.name == name)
            .is_some_and(|m| m.is_enabled(capabilities))
    }

    /// Navigation for the modules a session can actually reach, in registration order.
    pub fn nav(&self, capabilities: &Capabilities) -> Vec<NavEntry> {
        self.modules
            .iter()
            .filter(|m| m.is_enabled(capabilities))
            .filter_map(|m| m.nav)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAV: NavEntry = NavEntry {
        label: "Wallets",
        path: "/wallets",
    };

    fn capabilities(names: &[&str]) -> Capabilities {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    fn registry() -> ModuleRegistry {
        let mut registry = ModuleRegistry::new();
        registry
            .register(ModuleDescriptor::core("wallets", Some(NAV)))
            .unwrap();
        registry
            .register(ModuleDescriptor::gated(
                "forecast",
                "budget-forecast",
                Some(NavEntry {
                    label: "Forecast",
                    path: "/forecast",
                }),
            ))
            .unwrap();
        registry
    }

    #[test]
    fn a_core_module_renders_even_with_nothing_reported() {
        assert!(registry().is_enabled("wallets", &Capabilities::none()));
    }

    #[test]
    fn a_gated_module_needs_its_capability() {
        let registry = registry();

        assert!(!registry.is_enabled("forecast", &Capabilities::none()));
        assert!(registry.is_enabled("forecast", &capabilities(&["budget-forecast"])));
    }

    #[test]
    fn an_unrelated_capability_does_not_unlock_a_module() {
        assert!(!registry().is_enabled("forecast", &capabilities(&["audit-trail"])));
    }

    #[test]
    fn an_unregistered_module_is_never_enabled() {
        assert!(!registry().is_enabled("does-not-exist", &capabilities(&["budget-forecast"])));
    }

    #[test]
    fn navigation_hides_what_cannot_be_reached() {
        let registry = registry();

        assert_eq!(registry.nav(&Capabilities::none()), vec![NAV]);
        assert_eq!(registry.nav(&capabilities(&["budget-forecast"])).len(), 2);
    }

    #[test]
    fn registering_the_same_name_twice_is_rejected() {
        let mut registry = registry();

        let err = registry
            .register(ModuleDescriptor::core("wallets", None))
            .unwrap_err();

        assert_eq!(err, ModuleError::Duplicate("wallets".into()));
    }

    /// The gate reads `capabilities`; `enabled` and `available` are diagnostics only.
    #[test]
    fn plugin_names_do_not_leak_into_the_gate() {
        let dto = CapabilitiesDto {
            enabled: vec!["forecast".into()],
            available: vec!["forecast".into()],
            capabilities: vec![],
        };

        let capabilities = Capabilities::from_dto(&dto);

        assert!(capabilities.is_empty());
        assert!(!registry().is_enabled("forecast", &capabilities));
    }

    #[test]
    fn a_declared_capability_unlocks_its_module() {
        let dto = CapabilitiesDto {
            enabled: vec!["forecast".into()],
            available: vec!["forecast".into()],
            capabilities: vec!["budget-forecast".into()],
        };

        assert!(registry().is_enabled("forecast", &Capabilities::from_dto(&dto)));
    }

    /// An older server answers 404; the client must still boot with its core modules.
    #[test]
    fn an_absent_report_degrades_to_core_modules_only() {
        let registry = registry();
        let enabled = registry.enabled(&Capabilities::none());

        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "wallets");
    }
}
