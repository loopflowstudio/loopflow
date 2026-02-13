# Design Review: Stimulus Toggles, Service Management, and Combine

## What was implemented

Three features across Rust, Swift, and Python:

1. **Stimulus enabled flag** — Stimuli can be disabled without deletion. An `enabled` boolean column persists in the database (migration 002). All three trigger pollers (loop, watch, cron) skip disabled stimuli. Stopping a wave disables its auto-stimuli; running re-enables them. Swift decoding defaults to `true` for backwards compatibility.

2. **lfd service management** — `lfd install|uninstall|start|stop|status` manages the daemon as a platform service. macOS uses LaunchAgent plists with `launchctl`. Linux uses systemd user units with `systemctl`. An `unsupported` fallback returns a clear error on other platforms.

3. **Combine PRs** — Renamed `collapse` to `combine`. Collects commits from open wave PRs, cherry-picks them onto a new combined branch, creates a combined PR (with LLM-generated title/body), and closes the originals. Exposed via `POST /waves/:id/combine` and the Swift UI.

## Key choices

| Decision | Why | Alternatives rejected |
|----------|-----|----------------------|
| `enabled` column with `DEFAULT 1` | Simple, backwards-compatible migration. No data loss on upgrade. | Separate "disabled_stimuli" table — over-engineered for a boolean flag. |
| Disable stimuli on stop, re-enable on run | Prevents auto-restart while paused. Clean start when user explicitly runs. | Delete stimuli on stop — destructive, loses cron expressions. |
| Platform dispatch via `cfg` modules | Zero-cost — unused platform code isn't compiled. Clean separation. | Trait-based dispatch — runtime indirection for no benefit. |
| Cherry-pick (not merge) for combine | Preserves individual commit authorship. Avoids merge commit noise. | Squash-merge — loses commit granularity. Merge — adds merge commits. |
| Renamed collapse → combine | "Combine" is clearer and matches the UI label "Combine PRs". | Keep "collapse" — less intuitive for users. |

## How it fits together

```
lfd binary
  ├── service::install/start/stop  (platform-specific, runs before tokio)
  └── serve (tokio runtime)
       ├── triggers/loop_ticker.rs  ─┐
       ├── triggers/watch.rs        ─┼── all check stimulus.enabled before activating
       ├── triggers/cron.rs         ─┘
       └── http/routes/waves.rs
            ├── run_wave_handler    → re-enables stimuli
            ├── stop_wave_handler   → disables auto-stimuli, sets Paused
            └── combine_wave_handler → ops::combine_prs()
```

Database: `stimuli.enabled` (INTEGER, DEFAULT 1) added by migration 002.
Swift: `Stimulus.enabled` decoded with `decodeIfPresent` defaulting to `true`.

## Risks and bottlenecks

- **Service path stability**: `install` captures `current_exe()` path. Moving the binary breaks the service. Documented behavior — users re-run `lfd install` after relocating.
- **Cherry-pick conflicts**: `combine_prs` aborts cleanly on conflict, cleans up the branch, and returns an error. No partial state.
- **No individual stimulus toggle endpoint**: Bulk enable/disable only. Adequate for current UI (stop/run wave controls). Individual toggle can be added when needed.

## What's not included

- No HTTP endpoint for toggling a single stimulus (`PATCH /stimulus/:id`). The current UI only needs wave-level stop/run.
- No tests for service management (platform-dependent, requires launchctl/systemctl).
- No scheduled-restart or health-check for the installed service beyond launchd/systemd's built-in restart-on-failure.
