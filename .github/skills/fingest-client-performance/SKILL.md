---
name: fingest-client-performance
description: Trigger when changing Rust/Dioxus rendering, HTTP flow, wasm/mobile runtime behavior, or anything that can impact latency, render churn, allocation pressure, or cancellation behavior.
---

# FinGest Client Performance (repository-local)

Use this only in `fingest-clients` for proportional performance checks on web (`wasm32`) and Android (`aarch64`), without broad audits.

## Scope

- Repository-local only (`fingest-clients`), no parent-directory edits, no pushes/merges.
- Procedure-first: measure and report before proposing optimization.
- Prefer focused checks; do not claim benchmark wins without command output.

## Procedure

1. **Classify risk and choose depth (low/med/high)**
   - Use code path and fan-out to size effort:
     - **Low**: local rendering/text/state change.
     - **Medium**: list/resource refresh, form submission, response decoding.
     - **High**: transport timeouts/cancellation behavior, hot list screens, repeated event fan-out.
2. **Anchor to concrete hotspots**
   - Render/update guards and double-submit prevention:
     - `crates/fingest-client-view/src/busy.rs:59-123`
     - `crates/fingest-web-shell/src/wallets.rs:83-240`
     - `crates/fingest-mobile-shell/src/entry_sheet.rs:45-173`
   - Transport timeout bounds and network failure mapping:
     - `crates/fingest-client-http/src/transport.rs:45-67,120-132,532-553`
   - Event-driven refresh and fan-out:
     - `crates/fingest-client-view/src/context.rs:70-86`
     - `crates/fingest-client-events/src/lib.rs:30-42`
3. **Measure proportionally**
   - For UI churn: inspect rerender triggers (`use_resource`, `use_event_refresh`, busy flags) before changing code.
   - For wasm allocations: prefer reducing clone-heavy list redraws and repeated string formatting in hot paths.
   - For cancellation/lifetimes: ensure in-flight tasks release busy flags on all exits (`BusyGuard` drop path).
4. **Decide action**
   - Keep behavior if cost is negligible relative to feature impact.
   - Optimize only the confirmed hotspot; avoid speculative rewrites.

## Evidence requirements (mandatory)

- Include **file:line** evidence for each claim (at least 3 references).
- For each finding, record:
  - **Severity**: `LOW | MEDIUM | HIGH | CRITICAL`
  - **Confidence**: `1-10`
  - Why it matters (user-visible latency, jank risk, stuck UX risk).

## Focused validation commands

- `cargo test --locked --workspace`
- `cargo clippy --locked --workspace --all-targets`
- `cargo clippy --locked --target wasm32-unknown-unknown -p fingest-web -p fingest-web-storage`
- `cargo clippy --locked --target aarch64-linux-android -p fingest-mobile-storage`
- When touching dependency-boundary behavior:  
  `cargo test -p fingest-client-ports --test dependency_rule`

(Commands reflect `.github/workflows/ci.yml:25-60` and `.github/workflows/lockfile-consistency.yml:19`.)

## Handoff format (concise, required)

- `task_id`: `<id>`
- `revision`: `<branch/sha>`
- `changed_files`: `[path, ...]`
- `evidence`: `[path:line, ...]`
- `commands_run`: `[command]`
- `results`: `[pass/fail + key output]`
- `blockers`: `[none | concrete blocker]`
