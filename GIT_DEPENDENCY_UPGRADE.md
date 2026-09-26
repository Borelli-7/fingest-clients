# Git dependency upgrade playbook

This workspace pins git dependencies to immutable revisions (`rev`) for reproducible builds.

Current pinned dependencies:

- `fingest-contracts`
- `fingest-kernel`

Both point at `https://github.com/Borelli-7/fingest-rs-v2.git`.

## When to bump

Bump the pinned revision only when one of these is true:

1. Wire contract/domain changes are intentionally required by this client.
2. A security fix or critical bug fix in the upstream crate is needed.
3. A coordinated release requires a specific upstream commit.

## How to bump

1. Choose a reviewed upstream commit hash.
2. Update both entries in `Cargo.toml` under `[workspace.dependencies]`:
   - set `rev = "<commit-sha>"`
3. Regenerate lock metadata:
   - `cargo check --workspace`
4. Validate with lock enforcement:
   - `cargo check --workspace --locked`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets`
5. Include in PR description:
   - old rev
   - new rev
   - reason for bump
   - validation results

## CI guard

CI runs `cargo check --workspace --locked` to fail when `Cargo.toml` and `Cargo.lock` drift.
