# Reconciliation assumptions — October 4, 2026

No unresolved product judgment blocks implementation. Jack Heart approved the
workflow design; its [implementation defaults](focus-on-your-own-work.md) remain
reversible: 120-second Waiting fallback, most-recent interactive Task primary,
optional occurrence names, backward-target loops and source-only workflow YAML.
These are implementation choices, not Jack's exact spelling/timing preferences.

The saved-capture bridge preserves the existing invocation; live migration and
configured acceptance remain separate from implementation.

Superseded questions and proposals: `5090f672e:scratch/questions.md`.

Implementation interpretation: the existing per-Task automation on/off field also
holds CI repair. Preserve that setting and its delivery UI while deleting Flow
scheduling and its counters. This preserves unrelated CI policy; it provides no
Flow restart authority.

## Worker removal — implementation interpretations (October 4)

Reversible choices made while deleting Task-worker authority. None is attributed
to Jack Heart; each is the simplest reading of his complete-removal direction.

- **`flow start` always launches fresh.** A second start while a Flow is live
  launches a second Flow in the same checkout. The caller checks `task status`
  first. Open: whether Desktop's Start should say a Flow is already running.
- **Task status describes the most recently launched Flow.** One Flow's graph is
  shown until every associated Flow gets the shared detail view. It selects
  nothing and grants no control.
- **Builtin `feature` ends at a published PR.** It is `task-design` then `pursue`
  with no design-review pause; `ship` lands after the demo in the conversation.
  A design review first means launching `task-design`, then `pursue`. `code` now
  equals `pursue`; `ship-demo` is deleted. Open: whether `feature` should stop
  after kickoff, and whether `code` should remain.
- **Only live or unresolved execution holds completion.** A Flow whose driver
  died can no longer be resumed or ended, so it no longer blocks Task completion,
  checkout cleanup, abandon or landing cleanup. This adapts an evidence reader;
  admission/completion policy stays LOO-367.
- **A legacy review position neither waits nor blocks.** It reads as stopped
  history in status and no longer holds CI repair.
- **Any FlowSession counts as started work at a chapter boundary.** It replaces
  the worker-claim evidence. The terminal-plan/live-claim conflict is no longer
  reported as unresolved.
- **Remote Session placement was only the managed review.** That branch is gone
  with it; owning-Home remote association remains retained LOO-353 work.
- **Stacking no longer checks for a claim.** A ready Task can select a parent PR
  while a Flow runs.
- **A Flow that stops at a watched landing stays unfinished.** `pr land` inside
  a Flow returns while GitHub is still checking; the Flow exits "waiting on
  delivery" and its row stays `current` after the merge, because nothing
  resumes it. Landing cleanup proceeds as for a standalone landing. Open:
  whether reconciliation should mark such a Flow finished.
- **Main's `lf flow end` is dropped.** #1435 (LOO-326) added it to retire a
  stopped Flow that blocked a Task. Here a stopped Flow blocks nothing, so the
  merge kept boot-time exit evidence and omitted the command. Open: whether
  Infrastructure still wants an explicit "replaced" mark on stopped history.
