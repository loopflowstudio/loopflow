---
requires: rebased branch
produces: updated code and plans
---
Integrate upstream changes from main into this wave's work.

## Goal

Main advanced. The rebase already landed the new code. Now figure out if it matters to this wave. Most of the time it doesn't — a quick scan, no changes needed. When it does matter, adapt the code and plans to account for what landed.

## Workflow

### 1. See what landed

```bash
git log --oneline HEAD@{1}..origin/main
```

Identify the PRs/commits that just arrived. Focus on what changed, not the full history.

### 2. Check relevance

Does any of it touch this wave's area? Read the wave's area, direction, and current scratch/ design doc to understand what this wave is working on.

If the upstream changes are outside this wave's scope — different area, unrelated feature — stop here. No action needed.

### 3. Adapt if needed

If upstream changes are relevant:

- **Code conflicts already resolved by rebase**: verify the resolutions make sense. The rebase step handles mechanics; this step handles semantics.
- **API or interface changes**: update this wave's code to use new APIs, follow new patterns, or account for removed functionality.
- **Design implications**: if upstream work changes assumptions in `scratch/`, update the design doc. Note what changed and why.
- **New opportunities**: if upstream work makes something easier or obsolete, simplify. Delete code that's no longer needed.

### 4. Update wave plan if needed

If the wave has items in `wave/` that are affected by upstream changes — completed by someone else, made easier, made harder, or made irrelevant — note the impact in `scratch/questions.md`.

## What doesn't matter

- Upstream changes outside this wave's area — skip them
- Cosmetic or style changes — the rebase handled the merge
- Commit message quality of upstream — not your concern

## Guardrails

- Don't refactor code that upstream just shipped. Respect the merge.
- Don't expand scope because upstream added something interesting. Stay on task.
- If upstream broke something in this wave's area, fix the breakage but don't fix upstream's bug — that's a different wave.
