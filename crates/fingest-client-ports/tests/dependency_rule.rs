//! Enforces the hexagonal dependency rule for the clients.
//!
//! The port and `*-core` crates are the interior of the hexagon: they may not reference a
//! transport, a storage mechanism or a UI framework. This scans declared dependencies, so a
//! violation fails the build rather than waiting to be spotted in review.
//!
//! Ported from `fingest-kernel/tests/dependency_rule.rs` in the API workspace, with two
//! rules the server has no need for:
//!   - a shell may not reach an adapter except through a port;
//!   - a `fingest-client-*` crate may not name a platform crate, so the shared layer stays
//!     shared when the mobile shell lands beside the web one.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// Adapter technologies. Any of these inside the hexagon means a port was bypassed.
///
/// The mobile entries are listed before a mobile crate exists on purpose: the rule should
/// already be in force the first time someone reaches for JNI.
const FORBIDDEN: &[&str] = &[
    // web
    "dioxus",
    "dioxus-router",
    "web-sys",
    "js-sys",
    "wasm-bindgen",
    "wasm-bindgen-futures",
    "gloo-storage",
    "gloo-net",
    "gloo-timers",
    // mobile
    "jni",
    "ndk",
    "ndk-context",
    "android-activity",
    "objc2",
    "objc2-foundation",
    // shared transports and runtimes
    "reqwest",
    "tokio",
];

/// Adapter crates in this workspace. A shell must not name one directly.
const ADAPTER_CRATES: &[&str] = &[
    "fingest-client-http",
    "fingest-client-events",
    "fingest-web-storage",
    "fingest-mobile-storage",
];

/// Shells. Nothing shared may depend on one.
const PLATFORM_CRATES: &[&str] = &[
    "fingest-web-shell",
    "fingest-web-storage",
    "fingest-web-bootstrap",
    "fingest-mobile-shell",
    "fingest-mobile-storage",
    "fingest-mobile-bootstrap",
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/<name>/ is two levels below the workspace root")
        .to_path_buf()
}

fn crate_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(workspace_root().join("crates"))
        .expect("crates/ should exist")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("Cargo.toml").is_file())
        .collect();
    dirs.sort();
    dirs
}

fn crate_name(dir: &Path) -> String {
    dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned()
}

/// Crates that form the interior of the hexagon.
fn is_pure(name: &str) -> bool {
    name == "fingest-client-ports" || name == "fingest-client-modules" || name.ends_with("-core")
}

/// Crates every client shares. `fingest-client-view` and `-http` are adapters, so they are
/// not pure, but they must still stay free of anything platform-specific.
fn is_shared(name: &str) -> bool {
    name.starts_with("fingest-client-")
}

/// Extracts every dependency name a manifest declares.
///
/// Handles both spellings, because they are equivalent to cargo and a rule that only sees
/// one is a rule that can be walked around by accident:
///   `dioxus = "0.7"` and `[dependencies.dioxus]`
///
/// Comments are stripped first, so a crate named in a `#` note does not trip the check.
fn declared_dependency_keys(manifest: &str) -> Vec<String> {
    manifest
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim().to_owned())
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            if let Some(section) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                // `[dependencies.x]`, `[dev-dependencies.x]`, `[target.'cfg(..)'.dependencies.x]`
                return section
                    .rsplit_once("dependencies.")
                    .map(|(_, name)| name.trim().to_owned());
            }
            line.split(['=', '.', ' ']).next().map(str::to_owned)
        })
        .filter(|key| !key.is_empty())
        .collect()
}

#[test]
fn pure_crates_declare_no_adapter_dependencies() {
    let dirs = crate_dirs();
    assert!(
        !dirs.is_empty(),
        "found no crates to check — the glob is wrong"
    );

    let checked: Vec<String> = dirs
        .iter()
        .map(|dir| crate_name(dir))
        .filter(|name| is_pure(name))
        .collect();
    assert!(
        checked.contains(&"fingest-client-ports".to_owned()),
        "the ports crate must be covered, found {checked:?}"
    );

    let mut violations = Vec::new();

    for dir in dirs.iter().filter(|d| is_pure(&crate_name(d))) {
        let manifest_path = dir.join("Cargo.toml");
        let contents = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", manifest_path.display()));

        for key in declared_dependency_keys(&contents) {
            if FORBIDDEN.contains(&key.as_str()) {
                violations.push(format!(
                    "{} declares forbidden dependency `{key}`",
                    manifest_path.display()
                ));
            }
        }
    }

    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

/// A shell drives the application through ports handed to it by the composition root. If it
/// could name an adapter directly it could construct one, and the seam would be decorative.
#[test]
fn shells_reach_adapters_only_through_ports() {
    let mut violations = Vec::new();

    for dir in crate_dirs() {
        let name = crate_name(&dir);
        if !name.ends_with("-shell") {
            continue;
        }

        let manifest_path = dir.join("Cargo.toml");
        let Ok(contents) = fs::read_to_string(&manifest_path) else {
            continue;
        };

        violations.extend(
            declared_dependency_keys(&contents)
                .into_iter()
                .filter(|key| ADAPTER_CRATES.contains(&key.as_str()))
                .map(|key| format!("{name} declares adapter dependency `{key}`")),
        );
    }

    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

/// The rule that keeps the shared layer shared.
///
/// Without it, one `fingest-client-wallets-core -> fingest-web-storage` edge would quietly
/// make the whole hexagon web-only again, and the mobile shell would not compile.
#[test]
fn shared_crates_name_no_platform_crate() {
    let mut violations = Vec::new();
    let mut checked = 0;

    for dir in crate_dirs().iter().filter(|d| is_shared(&crate_name(d))) {
        checked += 1;
        let name = crate_name(dir);
        let manifest_path = dir.join("Cargo.toml");
        let contents = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", manifest_path.display()));

        violations.extend(
            declared_dependency_keys(&contents)
                .into_iter()
                .filter(|key| PLATFORM_CRATES.contains(&key.as_str()))
                .map(|key| format!("{name} declares platform dependency `{key}`")),
        );
    }

    assert!(checked > 0, "no shared crates found — the prefix is wrong");
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

/// The rule is only worth having if it can fail. Guards against a typo in the scanner
/// silently turning both tests above into no-ops.
#[test]
fn the_scanner_actually_detects_a_violation() {
    let manifest = r#"
        [dependencies]
        serde = { workspace = true }
        dioxus = "0.7"
    "#;

    let keys = declared_dependency_keys(manifest);

    assert!(keys.contains(&"dioxus".to_owned()));
    assert!(FORBIDDEN.contains(&"dioxus"));
}

/// The table spelling means the same thing to cargo, so it must mean the same thing here.
#[test]
fn a_dependency_declared_as_its_own_table_is_still_seen() {
    let keys = declared_dependency_keys("[dependencies.dioxus]\nworkspace = true");

    assert!(keys.contains(&"dioxus".to_owned()));
}

#[test]
fn a_target_specific_dependency_table_is_still_seen() {
    let keys =
        declared_dependency_keys("[target.'cfg(target_arch = \"wasm32\")'.dependencies.reqwest]");

    assert!(keys.contains(&"reqwest".to_owned()));
}

#[test]
fn an_ordinary_section_header_is_not_a_dependency() {
    let keys = declared_dependency_keys("[package]\n[dependencies]\n[lints]");

    assert!(keys.is_empty(), "found {keys:?}");
}

/// A crate name mentioned only in a comment is not a dependency.
#[test]
fn a_commented_out_dependency_is_not_a_violation() {
    let keys = declared_dependency_keys("# reqwest = \"0.12\"\nserde = \"1.0\"");

    assert!(!keys.contains(&"reqwest".to_owned()));
    assert!(keys.contains(&"serde".to_owned()));
}
