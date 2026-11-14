---
name: fingest-client-scalability
description: Trigger when changing list/data-loading flows, HTTP concurrency, duplicate submission guards, or introducing pagination/large-result handling for web and mobile shells.
---

# FinGest Client Scalability (repository-local)

Use this to keep client behavior stable under larger data volume and higher user interaction rates.

## Scope

- `fingest-clients` only.
- Prioritize bounded parallelism and deterministic UI behavior.
- Keep changes local and measurable.

## Procedure

1. **Bound request behavior**
   - Confirm native connect/request timeouts remain explicit:
     - `crates/fingest-client-http/src/transport.rs:45-67`
   - Confirm timeout behavior remains covered by test:
     - `crates/fingest-client-http/src/transport.rs:532-553`
2. **Prevent duplicate submissions**
   - Ensure submit/delete actions use busy/start guards and disabled buttons:
     - `crates/fingest-client-view/src/busy.rs:86-133`
     - `crates/fingest-web-shell/src/wallets.rs:83-106,146,175-240`
     - `crates/fingest-mobile-shell/src/entry_sheet.rs:45,172-173`
3. **Control fan-out refresh**
   - Validate `use_event_refresh` predicates are narrow (no global over-refresh):
     - `crates/fingest-client-view/src/context.rs:70-86`
     - `crates/fingest-web-shell/src/wallets.rs:31-37`
4. **Pagination/large list handling**
   - Current expense reads request full range by default (`DateRange::new(None, None)`):
     - `crates/fingest-web-shell/src/wallet_detail.rs:227,347`
     - `crates/fingest-mobile-shell/src/wallets.rs:168`
   - For high-volume flows, introduce bounded window or page params in port + transport + shells together (do not patch only UI).
5. **Avoid unbounded parallel HTTP from one interaction**
   - Keep one user action -> one in-flight mutation unless explicitly aggregated.

## Evidence requirements (mandatory)

- Include `file:line` evidence for concurrency, refresh, and list-window behavior.
- Rate each issue/change:
  - **Severity**: `LOW | MEDIUM | HIGH | CRITICAL`
  - **Confidence**: `1-10`

## Focused validation commands

- `cargo test --locked --workspace`
- `cargo clippy --locked --workspace --all-targets`
- If touching transport timeout/behavior:
  - `cargo test -p fingest-client-http a_server_that_never_answers_times_out_as_a_network_error`

## Handoff format (concise, required)

- `task_id`, `revision`, `changed_files`
- `scalability_evidence` (`file:line`)
- `commands_run` and `results`
- `blockers`
