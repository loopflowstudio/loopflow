# v0.12.7

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.7 makes a verified published release the single unit of production Home upgrades. The same rule now governs release automation itself: generated release state is rebuilt when its source frontier changes, never rebased into something that only looks current. Wave agents also preserve more reusable prompt context and expose whether that reuse is paying off.

## Homes move between verified runtime generations

Production upgrades now activate a matched `lf`, `lfd`, Mac app, and schema generation from one pinned published release. The transition is durable and recoverable, so a Home cannot quietly split across source-built binaries, migrations, and application versions (#1186).

- `refresh` pins the latest published tag, verifies the installer, native archive, and Mac app against published SHA-256 checksums, then upgrades through the external installation path.
- The Home records each runtime generation, upgrade phase, affected Work, and generation-aware Run trigger before activation.
- Upgrades fence new work, drain active Runs, advance migrations, activate the matched artifacts, restart the configured keeper, and reconcile enabled Waves, Projects, and Tasks.
- Durable receipts and recovery guards let an interrupted transition resume or roll back instead of leaving mixed binaries and schemas active.
- `lfd` health now exposes runtime identity, making the active generation observable after restart.
- Source builds remain available for validation under `local-bin/`, but validation-only control-plane artifacts cannot be promoted into production.

## Release recovery rebuilds stale generated state

Release preparation now treats its output as immutable relative to the main revision that produced it. If main advances, or an existing release PR is dirty or behind, Loopflow starts preparation again from current main rather than rebasing already-generated migrations, manifests, or notes (#1187).

- Recovery fetches and verifies the exact remote release head before resetting controller-owned generated state.
- A prepared head that is already integrated can finalize without another rebase; a head that falls behind is rebuilt through every preparation step.
- Auto-merge is armed only for the observed exact head and can be re-armed safely if GitHub drops it.
- Cron publishing keeps private Home topology out of repository output while preserving the configured provider authority needed for publish preflight.

This closes the recovery path that allowed v0.12.6 to publish artifacts containing draft migrations. Installed promotion correctly rejected those artifacts; v0.12.7 prevents release recovery from producing that state again.

## Prompt reuse is higher and measurable

Wave prompts now place stable doctrine and goals ahead of frequently changing memory and wake content, increasing the prefix providers can reuse. Persisted OpenCode conversations also survive harness restarts when the stored session remains available, with a clean fallback when it does not (#1185).

- `lf usage --cached --days 7` reports cache writes and prompt-cache hit percentage by repository and provider.
- `lf usage --days 7 --json` includes cache-write tokens in the shared usage record.
- Cache-write accounting now persists through SQLite and the shared Rust/Swift usage DTO.

## Operational notes

Use `uv run python scripts/install.py local` to build validation-only artifacts. Use `uv run python scripts/install.py refresh` to pin, verify, and activate the latest published generation on the active Home; direct source-build promotion is no longer the production upgrade path.
