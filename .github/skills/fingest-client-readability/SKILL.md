---
name: fingest-client-readability
description: Trigger when editing user-facing flows, errors, or shared client logic to keep behavior and wording clear while preserving contracts pinned from fingest-rs-v2.
---

# FinGest Client Readability (repository-local)

Use this for clear error UX, readable intent, and contract-safe client code.

## Scope

- `fingest-clients` only.
- Improve clarity without changing domain behavior unless required.
- Keep wording and error mapping consistent across web/mobile shells.

## Procedure

1. **Preserve clear error UX**
   - Keep technical parser/transport details in logs, show actionable user copy:
     - `crates/fingest-client-http/src/response.rs:36-46,116-121`
     - `crates/fingest-client-view/src/wording.rs` (shared user-facing wording)
   - Keep alerts explicit in shells (`role="alert"`):
     - `crates/fingest-web-shell/src/wallets.rs:45-47,148-150,231-233`
     - `crates/fingest-mobile-shell/src/wallets.rs:42-44,251-253`
2. **Guard readability with shared helpers**
   - Prefer `describe`, `format_money`, `start_action` over ad-hoc UI logic:
     - `crates/fingest-client-view/src/lib.rs:12-15`
3. **Keep contract readability tied to pinned source**
   - Contract crates are pinned by git revision; don’t silently fork DTO semantics:
     - `Cargo.toml:13-19`
   - If wire expectations change, route via `fingest-contracts` and update focused decode/error tests:
     - `crates/fingest-client-http/src/response.rs:57-121`
4. **Retain architecture readability**
   - Keep adapter boundaries enforced and understandable:
     - `README.md:25-36`
     - `crates/fingest-client-ports/tests/dependency_rule.rs:1-213`

## Evidence requirements (mandatory)

- Provide `file:line` citations for each readability/UX claim.
- For each issue/change:
  - **Severity**: `LOW | MEDIUM | HIGH | CRITICAL`
  - **Confidence**: `1-10`

## Focused validation commands

- `cargo test --locked --workspace`
- `cargo fmt --all -- --check`
- If wording/error mapping changed:
  - `cargo test -p fingest-client-http`
  - relevant shell crate tests if added/updated

## Handoff format (concise, required)

- `task_id`, `revision`, `changed_files`
- `readability_evidence` (`file:line`)
- `commands_run`, `results`
- `blockers`
