# Mark Phase 1 Complete

## Problem

The Phase 1 ordered set in `roadmap/concerto/README.md` references items that don't exist:
- `20260131-02-history-and-recency.md`
- `20260131-03-waiting-state-actionable.md`
- `20260131-04-running-state-progress-and-connect.md`
- `20260131-05-empty-state-creates-and-teaches.md`
- `20260131-06-quick-experiment-path.md`

These files were design docs in `scratch/` that got cleared after implementation—exactly as the loopflow style guide intends. But the README still references them as if Phase 1 is in progress.

Reality: **All Phase 1 features shipped.** Evidence from git log and code:

| Item | Feature | Evidence |
|------|---------|----------|
| 01 | Attention summary and grouping | README notes complete |
| 02 | History and recency | `WaveSidebar.swift`: Recent Activity section, 1-hour window, top 5 |
| 03 | Waiting state actionable | `WaveDetailPanel.swift`: Connect button, PR badges |
| 04 | Running state progress | `FlowProgressPills.swift`: step indicators, elapsed time |
| 05 | Empty state teaches | `QuickExperimentView.swift`: step buttons with descriptions |
| 06 | Quick experiment path | `QuickExperimentSidebarView/DetailView`: run steps without waves |

## Approach

Update `roadmap/concerto/README.md` to:
1. Mark Phase 1 status as **Complete**
2. Remove the orphaned Phase 1 ordered set (references nonexistent files)
3. Add a summary of what Phase 1 delivered
4. Clarify Phase 2 is now the active focus

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Create missing Phase 1 docs retroactively | Would document history | Style guide says code speaks for itself post-merge. Busywork. |
| Find new Phase 1 items to extend it | Could surface polish gaps | If nothing urgent, delays Phase 2 unnecessarily. Ship what's ready. |
| Leave README as-is | No work | Misleading. Phase 1 looks incomplete when it's done. |

## Key decisions

**Mark Phase 1 complete rather than extend it.** The conductor/improviser/listener personas all have their core flows working:
- Conductor: glanceable status, quick connect, land PRs ✓
- Improviser: quick wave creation, easy step running, low commitment ✓
- Listener: catch up quickly, see what happened, decide what to do ✓

**Don't recreate vanished design docs.** Per CLAUDE.md: "lf ops pr land removes scratch/* contents—by then, the code and its README should speak for themselves."

**Focus README on what's next, not what's done.** The roadmap is for planning future work. Shipped items don't need entries.

## Scope

- In scope: Update `roadmap/concerto/README.md`
- Out of scope: Creating new backlog items, changing Phase 2/3 items

## Done when

```bash
# README shows Phase 1 as Complete
grep -q "Phase 1.*Complete" roadmap/concerto/README.md

# No orphan Phase 1 file references
! grep -q "20260131-0[2-6]" roadmap/concerto/README.md
```
