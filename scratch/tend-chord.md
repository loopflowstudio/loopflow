# Chord — 2026-03-18

## Context

The tend cycle's draft proposed five tactical mutations to untangle a deadlock between chord-model and agent-embedding. On review, the deadlock was already resolved — agent-embedding's unlanded branch had bootstrapped an lfd wave, drawn clean boundaries, and done the structural work. The tend scan just couldn't see it because it only looked at main and open PRs.

## Moves

### 1. Scan step: add unlanded branch visibility

The scan-waves step now checks remote branches and local worktrees for work ahead of main. This was the root cause of the misdiagnosis — the tend cycle was blind to 37 commits of real progress.

### 2. Review-chord step: rewrite for coherence

The mutation-by-mutation approve/defer/reject walkthrough replaced with a conversation about what's working and what isn't. Moves emerge from shared understanding, not triage.

### 3. lfd wave joins the chord

agent-embedding already bootstrapped `wave/lfd/` with a README, config, and 4 items. Three member waves now: chord-model (engine/flow/signals), lfd (daemon runtime/process/PTY), agent-embedding (Concerto UI). Lands when agent-embedding's branch passes CI.

## What the draft got wrong

The five original mutations (expand chord-model area, add terminal session item, narrow PR #567, prune worktrees, silence agent-embedding) were treating symptoms of a visibility gap. The waves had already self-organized — the chord just couldn't see it.

## Session

The tend scan needs to see all work in flight, not just what's landed. When it can't, it invents problems. The fix was mechanical (scan remote branches), not structural (reorganize waves). Trust what the waves produce; make sure you can see it.

Tend is a daily habit of small shifts, not a crisis response framework. Find the one thing that needs adjusting, make the adjustment, roll forward. Most cycles should feel like "cool, found it, moving on" — not five-mutation intervention plans. When the chord draft is heavier than the fix, the process is working too hard.
