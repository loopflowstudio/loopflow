# 01: Block Queue View

**Finish line:** The primary Concerto screen is a queue of blocks — what's stuck, what needs a decision. Not a notification feed. A machine waiting for you.

## Context

Concerto currently centers on sessions and wave management. The redesign pivots it to conductor: the thing you look at to know what needs your attention across all waves.

The block queue is the UX realization of the signals architecture. Every block that can't self-heal ends up here. The human sees them, makes decisions, and the system continues.

Three kinds of human intervention arrive here:
- **Build: design review** (forward-looking) — is this the right thing to build?
- **Build: code review** (backward-looking) — is what we built good enough?
- **Tend: calibration** (meta) — are we making real progress? Are we drifting?

## What to build

1. **Block list view.** Queue sorted by urgency. Each block shows: which wave, what kind of block, when it occurred, what the system already tried. Compact enough to scan in seconds.

2. **Block detail view.** Tap a block to see full context: the wave's recent history, the chord's assessment, proposed actions. Enough to make a decision without leaving Concerto.

3. **Decision actions.** Per block type:
   - Design review: approve / request changes / redirect
   - Code review: ship / iterate / reject
   - Calibration: approve mutations / modify / override
   - Generic block: resolve / reassign / defer

4. **Block status.** Blocks move through: surfaced → viewed → decided → resolved. The queue only shows unresolved blocks. History is searchable.

5. **Empty state.** When the queue is empty: "Nothing needs you. Waves are running." This is the goal state — the system working autonomously.

## Done when

- Block queue is the default Concerto screen
- Blocks from tend flow (calibration) and build flow (review) both appear
- Human can make decisions that flow back to the system
- Resolved blocks leave the queue, enter history
- Empty state communicates health, not absence
