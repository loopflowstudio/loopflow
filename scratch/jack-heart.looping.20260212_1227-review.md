# Design Review: stimulus toggles, service management, collapse→combine

## What was implemented

Five related changes in one branch:

1. **Stimulus `enabled` flag** — Stimuli (loop/watch/cron) can be disabled without deletion. New `enabled` boolean column in the database, wired through DTO, HTTP, store, triggers, and Swift layers.

2. **`lfd service` commands** — `lfd install/uninstall/start/stop/status` manage lfd as a system service (launchd on macOS, systemd on Linux). Unsupported platforms get a clear error.

3. **Stop/run toggle semantics** — Stopping a wave disables its auto-stimuli and sets status to `Paused`. Running a wave re-enables all stimuli. This prevents tickers from restarting stopped waves.

4. **`collapse` → `combine`** — Renamed `collapse_prs` to `combine_prs` throughout. Simplified the implementation: removed templated body generation, streamlined git operations, added LLM-generated PR title/body via `generate_pr_message`.

5. **Swift UI cleanup** — Removed `collapse` references, simplified `WaveRunsTab` (removed inline PR display, relying on combine action), updated `Wave` model with `enabled` on `Stimulus`.

## Key choices

**Enabled flag vs. deletion:** Disabling preserves the stimulus configuration (cron expression, watch state) so re-enabling is seamless. Deletion would lose this state.

**Paused status on stop:** When a wave with auto-stimuli is stopped, it enters `Paused` (not `Failed`). This distinguishes "user stopped it" from "it crashed." Waves without stimuli go to `Failed` on stop (backward-compatible).

**Service management in the binary:** `lfd install` lives in the `lfd` binary itself rather than a separate install script. This means the plist/unit file always references the correct binary path and captures the current `PATH`.

**combine simplification:** The old `collapse` used a handcrafted PR body listing merged PRs. The new `combine` delegates to `generate_pr_message` for an LLM-generated title and body, matching `lf ops pr` behavior. Cherry-pick approach preserved.

## How it fits together

```
User stops wave → HTTP stop handler
  → kills agents, marks run failed
  → set_wave_stimuli_enabled(false, auto_only=true)
  → wave.status = Paused

User runs wave → HTTP run handler
  → set_wave_stimuli_enabled(true, auto_only=false)
  → creates wave run, spawns executor

Triggers (loop/watch/cron) → check stimulus.enabled
  → skip disabled stimuli
```

Service management:
```
lfd install → writes plist/unit, loads service
lfd start   → loads/starts existing service
lfd stop    → unloads/stops service
lfd status  → queries launchctl/systemctl
```

## Risks and bottlenecks

- **Migration 002** adds `enabled` column with default `1`. Safe for SQLite (instant). Postgres migration is also straightforward (`ALTER TABLE ADD COLUMN ... DEFAULT 1`).
- **Cherry-pick conflicts** in combine: if PR branches have overlapping changes, cherry-pick will fail. Error message is clear — user needs to resolve manually.
- **Service management** uses platform-specific commands (`launchctl`, `systemctl`). The unsupported platform fallback returns a descriptive error.

## What's not included

- No UI for toggling individual stimulus enabled/disabled — only the stop/run wave actions toggle all stimuli at once.
- No migration rollback mechanism (standard for this codebase).
- No Windows service support (not a target platform).

## Test coverage

| Suite | Result |
|-------|--------|
| Rust (224 unit + integration) | All pass |
| Python (26) | All pass |
| Swift (96 in 17 suites) | All pass |
| `cargo fmt` | Clean |
| `cargo clippy -- -D warnings` | Clean |

The combine tests (`combine_tests.rs`) cover the two key paths: successful combine with 2+ open PRs, and the error case for <2 PRs.
