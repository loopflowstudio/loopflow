# Terminal-per-wave dashboard

## The idea

The default Concerto view is N terminals for N waves. No panes, no roadmap panels — just the terminal session for each wave, always visible.

lfd runs all `lf` operations inside tmux sessions. Every automated run is watchable. Concerto attaches.

## Wave terminal lifecycle

1. **Wave exists → worktree exists.** Kept on main (or rebased onto main) until a run claims it. A free-form tmux session is attached to the worktree.

2. **Run starts → new tmux session `lf-<waveId>-<runId>`.** `lf <flow>` executes inside it. Dashboard switches to show this session.

3. **Run finishes, PR in merge queue.** Session is idle but persists (output is inspectable). Worktree stays on the run's branch HEAD.

4. **Next run starts → new session, stacks.** Branches off old HEAD (`--stack`). Dashboard shows the new session. Old session still attachable.

5. **PR merges → rebase.** Stacked work rebases onto main automatically. If CI fails, the stack needs a rebase after fix.

6. **No runs in flight, last PR merged → worktree resets to main.** Free-form session, clean shell, ready.

## Key decisions

**One worktree per wave, not per run.** Runs stack on the same worktree. No worktree proliferation, no cleanup. The worktree is the wave's home.

**One tmux session per run.** Named `lf-<waveId>-<runId>`. Each run gets its own session. Old sessions persist until cleanup (PR merged or failed). The dashboard shows the most recent session — the 0th. Older sessions are still attachable if you want to inspect them.

**Free-form session when idle.** When no run is active, the wave has a free-form session in the worktree. When a new run starts, it gets its own session. The free-form session stays available.

**Stacking, not waiting.** When a PR is in merge queue and the next run starts, it branches off the current HEAD. No idle time waiting for CI. `--stack` is the mechanism.

**Dirty state blocks, not stashes.** If the worktree has uncommitted changes when a run wants to start, that's a block. The run doesn't start. No auto-stash complexity. The user committed or the previous run committed. If something's dirty, something went wrong.

## lfd changes (existing wave items)

This connects to:
- **lfd/README** "one execution path" — lfd spawns `lf <flow>` in tmux instead of raw `tokio::process::Command`
- **lfd/03** daemon-hosted shells — this IS the shell model, just arrived via a different path
- **chord-model/02a** worker pools — `workers: 1` means one worktree, one tmux session, runs stack

### What lfd needs to do

1. **Create free-form tmux session when wave is created.** `tmux new-session -d -s lf-<waveId> -c <worktreePath>`.

2. **Create run session per run.** `tmux new-session -d -s lf-<waveId>-<runId> -c <worktreePath>`. Run `lf <flow>` inside it.

3. **Detect run completion.** Structured events from `lf` (the shared-store model from lfd/README). `lf` already writes lifecycle events.

4. **Stack on PR-in-flight.** When starting a new run and the current branch has a PR in merge queue: `lf ops wt create --stack` (or the equivalent git operations) before running the next flow.

5. **Reset on merge.** When a PR merges and no runs are in flight: reset the worktree to main.

6. **Clean up old sessions.** When a PR merges or a run is abandoned, kill the tmux session.

## Concerto changes (this branch)

### Dashboard view

The empty state (when no attention items) becomes N wave terminal cards:

- Each card shows the wave name, vision tagline, and diff indicator (done in this branch)
- Each card embeds a Ghostty terminal attached to `lf-<waveId>` tmux session
- Grid layout, not list — you see all waves at once

### Wave sidebar

Already cleaned up in this branch:
- Vision tagline instead of area
- Diff indicator (green/red) instead of flow badge
- No default flow display, no area display

## What's in this branch vs what's next

**This branch (shipped):**
- Wave README taglines
- Wave overview in empty state (clickable cards with name + vision + diff)
- WaveRow: vision tagline, diff indicator, no flow/area

**Next (lfd wave, separate branch):**
- lfd creates tmux sessions for waves
- lfd runs `lf` inside tmux
- Concerto dashboard embeds Ghostty attached to wave tmux sessions
- Stacking logic for PR-in-flight
- Worktree reset on merge
