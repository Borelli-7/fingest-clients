---
name: fingest-client-engineering
description: Trigger when implementing or refactoring Rust client code paths where async resource lifetimes, UI responsiveness, capability gating, or adapter boundaries can regress engineering quality.
---

# FinGest Client Engineering (repository-local)

Use this for best-practice implementation decisions in this workspace, especially around async lifetimes and non-blocking UI behavior.

## Scope

- `fingest-clients` only.
- Preserve architecture boundaries: pure crates vs adapters/shells.
- No broad policy rewrites; apply surgical, behavior-preserving changes.

## Procedure

1. **Confirm boundary correctness first**
   - Workspace split and dependency rule:
     - `README.md:9-36`
     - `crates/fingest-client-ports/tests/dependency_rule.rs:20-213`
2. **Apply async lifetime rules**
   - Use guard-on-drop for all spawned form actions:
     - `crates/fingest-client-view/src/busy.rs:6-60,115-156,217-259`
   - Start action synchronously before `spawn` (prevent silent no-op tasks):
     - `crates/fingest-client-view/src/busy.rs:115-133`
3. **Avoid UI blocking**
   - Keep network calls inside async resources/tasks (`use_resource`, `spawn`), never in render path.
   - Check representative screen patterns:
     - `crates/fingest-web-shell/src/wallets.rs:19-42,83-106`
     - `crates/fingest-mobile-shell/src/wallets.rs:17-29,216-230`
4. **Keep failures explicit and user-facing**
   - Convert technical failures via shared wording, route-visible alerts:
     - `crates/fingest-client-http/src/response.rs:36-46`
     - `crates/fingest-web-shell/src/wallets.rs:43-66,148-244`
5. **Respect contract source of truth**
   - Do not hand-roll wire structs where `fingest-contracts` already defines shape:
     - `Cargo.toml:13-19`

## Evidence requirements (mandatory)

- Cite concrete `file:line` proof for boundary, lifecycle, and UX decisions.
- Record each issue/change with:
  - **Severity**: `LOW | MEDIUM | HIGH | CRITICAL`
  - **Confidence**: `1-10`

## Focused validation commands

- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets`
- `cargo test --locked --workspace`
- If boundary-related: `cargo test -p fingest-client-ports --test dependency_rule`

## Handoff format (concise, required)

- `task_id`, `revision`, `changed_files`
- `evidence` (`file:line`)
- `commands_run` and `results`
- `blockers` (or `none`)
