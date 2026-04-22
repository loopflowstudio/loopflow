# PM Auth & Claim Safety Review

## What was implemented

- Removed PM API-key setup paths for Asana, Linear, and Notion from the Rust auth surface and the Python `lfq` auth surface.
- Changed PM token resolution to read stored credentials only and point missing credentials at `lf op auth <provider>`.
- Added branch-based claim checks for Linear and Asana so workers sharing one PM account do not both treat the same item as claimed.
- Added Asana `lf op pm init` setup for one workspace-level `Working branch` text custom field attached to each initialized project.
- Updated PM docs and user auth examples to describe OAuth-only PM auth and branch-locked claims.

## Key choices

- PM providers are gated narrowly. Claude, Codex, and OpenCode Zen keep API-key configure support.
- Stale stored PM API-key rows are not migrated or rejected. The branch only removes configure and env-var fallback entrypoints.
- Asana stores the claim lock in a shared workspace custom field named `Working branch`; comments remain as the activity log.
- Linear and Asana both check existing branch state before writing and verify branch state after writing. This avoids overwriting a branch already claimed by another worker and catches races where another worker writes between our write and verify.
- The old `scripts/setup-asana.py` PAT setup helper was deleted instead of kept as a compatibility path.

## How it fits together

`lf op pm init` now calls the provider's `init_project` hook after the read/write PM project is linked. Asana uses that hook to find/create the workspace field and attach it to the project; other providers no-op. During ingest, `pm_try_claim` still asks the provider client to claim an unassigned item, but Linear and Asana now use the branch name as the worker discriminator rather than only checking the PM account assignee.

## Risks and bottlenecks

- Asana and Linear claims are optimistic API writes, not provider-native compare-and-swap locks. Branch pre-check plus post-write verification covers normal same-account worker overlap, but an exact simultaneous write can still depend on provider response timing.
- Asana projects created before this branch need `lf op pm init <wave>` re-run to attach `Working branch` before branch-locked claims work.
- Notion remains best-effort; duplicates are still arbitrated later at PR time.
- OAuth client credentials still need to be present in the lfd environment; this branch does not solve secret injection.

## What's not included

- No PM credential migration or cleanup of existing stored API-key rows.
- No Notion claim lock.
- No provider capability declarations or typed Swift `AuthStep` model.
- No default OAuth client credential broker.

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `rg "ASANA_ACCESS_TOKEN|LINEAR_API_KEY" rust/` — no matches
- `rg "ASANA_ACCESS_TOKEN|LINEAR_API_KEY|store Linear API key|lfq auth linear.*API|lf op auth configure linear" -n README.md docs python scripts rust wave/pm/README.md` — no matches
