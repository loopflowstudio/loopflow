# PM bootstrap + lifecycle sync review

## What was implemented

- Added provider-role PM config: one read/write provider plus zero or more export providers.
- Added `lf ops pm init` to bootstrap a wave into a working PM state by linking existing roadmap items, creating missing remote/local items, and writing provider IDs back to wave YAML/frontmatter.
- Added `lf ops pm status` to report linked waves with per-provider local/remote counts.
- Wired PR-oriented wave runs to import from the read/write provider at run start and export back to the read/write + export providers at run completion.
- Updated config parsing and wave PM parsing to understand `rw_provider` / `export_providers` and multiple per-provider project IDs.

## Key choices

- **Provider roles instead of a single provider switch.** Linear stays the canonical read/write source; Asana can mirror updates without driving local state.
- **Bootstrap stays conservative.** `pm init` matches by provider ID first, then normalized title, creates missing items, and avoids deleting anything.
- **Lifecycle sync is best-effort.** Import/export failures log warnings and never fail a wave run.
- **Run-start import / run-end export only for PR-oriented runs.** Repair runs on existing PR branches do not create extra PM churn.
- **Preserve local content during bootstrap matches.** Gate fixed a destructive edge case where matched read/write items were overwriting local markdown bodies.

## How it fits together

Repo config and wave YAML now declare PM provider roles plus per-provider project IDs. `lf ops pm init` uses that role map to construct the right clients, reconcile roadmap files with remote items, and persist the resulting IDs. During normal executor runs, PR-oriented flows call the existing import/export ops paths automatically so the roadmap pulls from the read/write provider at the start of work and pushes back out at the end.

## Risks and bottlenecks

- Live end-to-end validation still depends on real Linear/Asana credentials and projects; this gate covered the Rust behavior with automated tests but not a live provider round-trip.
- Item-level PR/merge comments/completion are still intentionally deferred because run state does not yet retain a stable roadmap-item identity after ingest.
- Wave-level export currently writes to every configured provider role; if a future wave needs to opt out of repo-default export providers, the config model may need a more explicit per-wave override.

## What's not included

- Import from export-only providers.
- Destructive multi-source reconciliation.
- Item-level PR-open / PR-merge / run-failed PM comments.
- Additional provider-specific command trees beyond `lf ops pm init/status/import/export/sync`.

## Validation

### Commands run

```bash
cargo fmt --check
cargo fmt
cargo clippy -- -D warnings
cargo test --all
```

### Results

- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅ (`845 passed; 0 failed; 2 ignored` in the main Rust test binary, plus all cargo integration/bin/doc tests)

### Done-when check

- **Repo can be configured with Linear as RW and Asana as export-only** — implemented via `.lf/config.yaml`, `Config`, and `WavePmConfig` support for `rw_provider` / `export_providers`.
- **Bootstrap gets the system into a working state without push/pull/union prompts** — implemented by `lf ops pm init`.
- **Starting PR-oriented work syncs from Linear** — implemented in `WaveExecutor::execute()` run-start import hook.
- **Finishing PR-oriented work writes back to Linear and exports to Asana** — implemented in `WaveExecutor::execute()` completion export hook.
- **Roadmap items carry both provider IDs where applicable** — supported by `RoadmapItemFrontmatter` and exercised by bootstrap/export paths.
- **Automatic PM behavior centered over manual editing** — achieved for wave-level import/export hooks; item-level comments/completion remain a documented follow-up.
