---
name: fingest-client-security
description: Trigger when touching auth/session/token handling, HTTP auth flows, Android keystore paths, logging, or storage behavior; follow this as a security procedure (not a request to run a full security audit).
---

# FinGest Client Security Procedure (repository-local)

This is a **procedure skill**, not a command to launch a security audit agent.

## Scope

- Repository-local (`fingest-clients`) only.
- No secret output, no approval bypass, no permission broadening.
- Focus on concrete client security controls in this repo.

## Procedure

1. **Auth/session flow checks**
   - Verify 401 handling only expires real authenticated sessions:
     - `crates/fingest-client-http/src/response.rs:12-18,75-83`
     - `crates/fingest-client-http/src/transport.rs:111-139`
   - Verify sign-out propagation from event bus:
     - `crates/fingest-web-bootstrap/src/lib.rs:59-75`
     - `crates/fingest-mobile-bootstrap/src/lib.rs:62-73`
2. **Storage controls**
   - Web: token in `sessionStorage` with explicit XSS tradeoff, no fake guarantees:
     - `crates/fingest-web-storage/src/lib.rs:3-7,18-25`
   - Android: keystore-backed storage; fail closed when unavailable:
     - `crates/fingest-mobile-storage/src/android.rs:1-6,157-198`
     - `crates/fingest-mobile-storage/src/lib.rs:132-140`
     - `scripts/build_android.sh:8-14`
3. **Secrets and logging hygiene**
   - No token/PII values in logs.
   - Preserve existing pattern of logging error context, not credentials/session payload:
     - `crates/fingest-client-http/src/transport.rs:128`
     - `crates/fingest-web-storage/src/lib.rs:48-59`
     - `crates/fingest-mobile-storage/src/lib.rs:121-139`
4. **Contract integrity**
   - Keep API contracts pinned to `fingest-rs-v2` revisions:
     - `Cargo.toml:13-19`

## Evidence requirements (mandatory)

- Provide exact `file:line` references for each security claim.
- For each finding:
  - **Severity**: `LOW | MEDIUM | HIGH | CRITICAL`
  - **Confidence**: `1-10`
  - User impact and exploitability note.

## Focused validation commands

- `cargo test --locked --workspace`
- `cargo clippy --locked --workspace --all-targets`
- `cargo clippy --locked --target aarch64-linux-android -p fingest-mobile-storage`
- `cargo check --workspace --locked`

## Handoff format (concise, required)

- `task_id`, `revision`, `changed_files`
- `security_evidence` (`file:line`)
- `commands_run` and real `results`
- `open_risks` and `blockers`
