# Default Stimuli & Integrate Flow

## What was implemented

Every wave now gets two default stimuli at creation time:

- **Watch** (flow: `integrate`) — triggers rebase + integrate-upstream when main advances
- **CiFailure** (flow: `ci-fix`) — triggers CI remediation when checks fail

If the wave's config already provides a stimulus for either signal, the default is skipped (no duplicates).

New builtin step `integrate-upstream` and flow `integrate` (rebase → integrate-upstream) give waves an automatic path to stay current with main.

Wave plan updated: old `04-concerto-ui` replaced by `01-signal-cleanup` and `02-concerto-ui`, establishing the sequence for signal model cleanup then UI work.

## Key choices

**Defaults via code, not migration.** Default stimuli are created in `create_wave_handler` rather than via a SQL migration that backfills existing waves. This means only newly-created waves get defaults. Existing waves are unaffected — they can add stimuli manually. This avoids a migration that touches live data.

**Dedup by signal type.** If config provides `kind: watch`, the default Watch stimulus is skipped. Checked via `stimuli.iter().any(|s| s.signal == signal)`. Simple and correct — a wave shouldn't have two Watch stimuli.

**`Stimulus::new()` constructor.** Removed `#[allow(dead_code)]` since it's now used in production. Fields set post-construction via struct mutation (`s.flow = ...`). This matches the existing pattern for `Stimulus` — the constructor handles defaults, callers override what they need.

**Integrate flow is two steps.** `rebase` (existing ops step) handles git mechanics. `integrate-upstream` (new code step) handles semantic assessment — does the rebase matter to this wave? Most of the time it doesn't, and the agent no-ops quickly.

## How it fits together

```
wave creation
  └─ create_wave_handler
       ├─ parse config stimulus (if any)
       ├─ create wave
       ├─ ensure workspace
       └─ create stimuli
            ├─ config stimulus (if provided)
            ├─ Watch → flow: integrate (default)
            └─ CiFailure → flow: ci-fix (default)

watch trigger fires (main advanced)
  └─ flow: integrate
       ├─ step 1: rebase (git rebase onto main)
       └─ step 2: integrate-upstream (assess + adapt)
```

Stimuli cascade-delete with the wave (SQL `ON DELETE CASCADE`), so the error path that deletes the wave on stimulus creation failure correctly cleans up all previously-created stimuli.

## Risks and bottlenecks

- **Agent cost per main advance.** Every main push triggers Watch on every wave, even when changes are irrelevant. The integrate-upstream step should no-op quickly, but it still costs an agent invocation per wave. Wave plan notes this as a known risk.
- **No backfill.** Existing waves don't get default stimuli. Acceptable for now — users can add them manually or recreate waves.
- **`parse_stimulus` still accepts "loop" and "once".** These signal types are planned for removal in 01-signal-cleanup but remain valid in this branch. No code change needed here.

## What's not included

- **Signal cleanup** (Loop/Once → WaveMode) — planned as 01-signal-cleanup, separate branch
- **Concerto UI** for chord grouping — planned as 02-concerto-ui
- **Backfill migration** for existing waves — intentionally deferred
- **Watch trigger filtering** by area — would skip irrelevant rebases, but the step handles this via early exit
