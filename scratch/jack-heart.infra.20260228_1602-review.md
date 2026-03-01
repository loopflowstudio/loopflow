# Review: Cron trigger simplification + daemon integrity design

## What was implemented

Simplified `should_activate_cron` in `cron.rs` by removing the unused `last_triggered` parameter. The parameter was always `None`, making the grace-period fallback the only path — the function now says what it does: check if any cron schedule in the past 24 hours is due.

Added `scratch/infra-daemon-integrity.md` — the design doc for the daemon integrity wave item covering transactional SQLite migrations, output log pruning, and resource leak cleanup.

## Key choices

**Removed parameter instead of implementing it.** The `last_triggered` parameter existed as a placeholder for future tracking, but it was always `None`. Removing it avoids a misleading API — if cron-specific tracking is needed later, it can be added when the storage exists. Dead parameters teach the wrong thing about what a function does.

**Kept the 24h grace window.** The function checks `schedule.after(now - 24h)` to find any due schedule. This is the same behavior as before (since `last_triggered.unwrap_or(now - 24h)` was always the fallback). The 24h window means a daemon that was down will pick up missed cron jobs when it restarts, as long as the outage was under 24 hours.

**`is_some_and` over if-let.** More idiomatic for a simple predicate check on an Option.

## How it fits together

The cron poller runs every 30 seconds, iterating waves with cron stimuli. `should_activate_cron` is the pure predicate that decides whether a cron expression is currently due. The simplification removes one layer of indirection without changing runtime behavior.

The design doc (`scratch/infra-daemon-integrity.md`) captures the broader daemon integrity work — most of which was already shipped in #513 (transactional migrations, output log pruning, resource leak fixes). This branch addresses the remaining cron cleanup.

## Risks and bottlenecks

**Low risk.** Behavior is identical — `last_triggered` was always `None`, so the only code path was already the 24h grace window. Clippy and tests pass.

**No cron-specific tests exist.** The function is private and simple, but the 24h grace window logic is worth testing if cron behavior becomes more sophisticated. Not blocking for this change.

## What's not included

- Per-wave `last_cron_triggered_at` tracking (would require schema changes; the 24h grace window is sufficient for now)
- Changes to the 30-second polling interval
- The rest of the daemon integrity work — transactional migrations, OutputHub cleanup, log pruning, and queue lock cleanup were shipped in #513
