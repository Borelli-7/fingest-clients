# fingest-clients

Web and Android clients for the [fingest-rs-v2](https://github.com/Borelli-7/fingest-rs-v2) API,
sharing their domain, use-case and transport layers.

Both clients are Rust. The browser build targets `wasm32-unknown-unknown`; the Android build
compiles the same crates to `aarch64-linux-android` and renders through a WebView.

## Layout

```
crates/
  fingest-client-ports/        traits + DTO-facing types   (pure)
  fingest-client-*-core/       use cases                   (pure)
  fingest-client-modules/      capability-gated navigation (pure)
  fingest-client-view/         shared rendering helpers
  fingest-client-http/         reqwest transport           (adapter)
  fingest-web-storage/         sessionStorage              (adapter)
  fingest-mobile-storage/      Android Keystore            (adapter)
  fingest-web-shell/           browser screens
  fingest-mobile-shell/        device screens
  fingest-*-bootstrap/         composition roots
bins/
  fingest-web/  fingest-mobile/
```

## The dependency rule

Pure crates (`*-core`, `-ports`, `-modules`) must not reach for a framework, a transport or a
platform. That is not a convention — `crates/fingest-client-ports/tests/dependency_rule.rs` reads
every member manifest and fails the build if one of them names `dioxus`, `reqwest`, `web-sys`,
`jni` and friends.

The test is written to fail: introduce a violation deliberately and it will catch it. Run it with
`cargo test -p fingest-client-ports --test dependency_rule`, or as part of `cargo test --workspace`.

## Shared crates

`fingest-contracts` and `fingest-kernel` are pulled from the API repository over git rather than
copied, so a change to the wire shape breaks compilation here instead of surfacing as a runtime
mismatch. `Cargo.lock` pins the exact commit.

## Building

```bash
cargo test --workspace          # 142 tests
cargo clippy --workspace --all-targets

# web
cd bins/fingest-web && dx serve --web

# android — needs ANDROID_NDK_HOME
./scripts/build_android.sh release
```

`build_android.sh` runs two passes: `dx` generates the Gradle project, the script injects the
`androidx.security:security-crypto` dependency the Keystore adapter needs, then Gradle assembles.
`dx` regenerates that file each build, so the injection is re-applied every time.

The emulator needs `-gpu host`; a software renderer never paints. Point the app at the API with
`adb reverse tcp:8080 tcp:8080` — `127.0.0.1` is the only host the generated
`network_security_config.xml` allows over cleartext, which is why it is the default base URL.

## Status

Verified end to end in the browser: login and session restore, categories, users, wallets and
expenses. Verified on device: login, wallet list and the entry sheet.

Not yet verified at runtime, though compiling and unit-tested: the budgets screens on both
clients, and whether the Android Keystore adapter survives an app restart. The capability gate has
no gated module exercising it yet — every module currently ships as core.
