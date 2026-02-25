# Review: flow lifecycle rename + update-wave unification

## What was implemented

- Renamed the headless build flow from `ship` to `build` (`implement → compress → gate → update-wave`).
- Renamed `design-ship-review` to `ship` (`design → build → review`).
- Updated dependent flows (`pair`, `grind`, `incident`, `ship-wave`, `ship-roadmap`, `scan`) to reference `build`.
- Removed `publish` flow and removed `consolidate` / `add-to-wave` steps.
- Updated plan flows (`wave-reduce`, `wave-polish`, `wave-expand`) to end with `update-wave`.
- Expanded `update-wave` step contract to own roadmap updates, scratch promotion, dedupe, and scratch cleanup.
- Changed default wave flow in API creation path from `ship` to `build`.
- Added loop ticker backlog check (`wave_backlog_empty`) to skip loop runs when `wave/<name>/` has no actionable markdown backlog.
- Updated Rust/Python/Swift tests, mock data, and UI previews for renamed flows.
- Updated user docs (`README.md`, built-in flow docs, and docs pages) to use the new flow names and lifecycle.

## Key choices

- **Single post-work step:** consolidate post-work behavior under `update-wave` instead of splitting across `consolidate` + `add-to-wave` + `publish`.
- **Default automation flow is `build`:** API-created waves now default to headless build behavior.
- **Loop stop signal is wave backlog state:** ticker checks canonical wave worktree backlog before starting a loop iteration.
- **No backward-compat shims:** references were migrated directly; old flow/step names were removed.

## How it fits together

Flow definitions now make naming match behavior: `build` is the autonomous implementation pipeline, while `ship` is the interactive design/build/review workflow. `update-wave` is the terminal reconciliation step across both code and plan flows. Loop triggering now consults `wave/<name>/` backlog state in the canonical wave worktree, so loop waves naturally idle when there are no actionable markdown items left.

## Risks and bottlenecks

- **Behavioral migration risk:** external scripts that still call old names (`ship` as headless flow, `design-ship-review`, `publish`, `consolidate`, `add-to-wave`) will break until updated.
- **Backlog semantics risk:** loop emptiness is based on top-level actionable `*.md` files (excluding `README.md`); misplaced items or non-markdown backlog files will not trigger runs.
- **Environment bottleneck in local full-suite runs:** two Docker startup tests require a live Docker socket; on hosts without `/var/run/docker.sock`, full `cargo test --all` fails unless those tests are skipped.

## What's not included

- No per-run/sidecar worktree architecture changes.
- No wave config schema changes.
- No DB-backed wave item tracking.
- No auto-pause behavior when backlog is empty (loop stimulus remains enabled; ticker just skips run creation).
