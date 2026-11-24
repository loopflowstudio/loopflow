# Choices made at kickoff and concept review, for review

- **Explicit parent completion runs immediately instead of pending on `tasks.completion_request`** (concept review):
  Replaces the kickoff plan's new `rehoming_completion_requested` event kind.
  A refused `--children abandon` or `--children rehome` writes nothing to
  SQLite, so background reconciliation can never silently abandon or rehome
  children later, and there is zero version skew with released `lf`. Passed
  over: storing `ChildPolicy` on `completion_request` (created a delayed
  abandon bug on refused runs and required a new event kind).
- **Live-child guard ignores idle held interactive conversations** (concept review):
  `--children abandon` blocks on running Graphs, headless runs, and active
  turns (`terminal_retention` present), but allows abandoning a child sitting
  idle at an interactive step (e.g. `ship-review`) whose held conversation
  `close_conversations` cleanly closes. Passed over: raw
  `task_execution_blockers` (would block every started child with an open
  conversation, with no `lf task interrupt` way to clear it).
- **Bare refusal lists the open descendant tree, indented** (concept review):
  Grandchildren under a merged child are visible before choosing
  `--children abandon`. Passed over: direct children only (hides unmerged
  grandchildren that `--children abandon` would abandon).
- **Rehome targets the nearest open ancestor, else top-level** (concept review):
  Skips closed, abandoned, or deleted grandparents so open children are never
  rehomed under a dead parent. Passed over: blindly copying
  `parent.parent_task_id`.
- **Desktop default button follows safety** (kickoff + concept review):
  When any open descendant would be abandoned, default is non-destructive
  `Complete & Move Children to …` and `Complete & Abandon Children` is
  `.destructive`. When every open descendant's PR is already merged, default is
  non-destructive `Complete & Finish Children` with `Move Children` second.
  Passed over: always defaulting to Move even when all children are merged, or
  abandon-first as today.
- **iOS learns about children via `ViewContent.open_children: Option<u32>`** (kickoff + concept review):
  Matches `ViewContent`'s other Task-only optional fields (`None` for Waves,
  `Some(n)` for Tasks) so iOS swipe asks before writing. Passed over:
  non-optional `u32` (misleading `0` on Waves) or always sending `children: null`
  and offering destructive choice from a Notice button.
- **Live-child guard refuses the whole completion** rather than abandoning idle
  children and skipping the live one (kickoff).
- **Refusal counts direct children** (implement): "LOO-12 has 3 open children"
  counts direct children, as the plan's example does, while the list shows the
  whole tree. Passed over: counting every descendant.
- **Leaf with an unmerged PR keeps the pending request** (implement): bare
  `lf task complete` on a childless Task with an unmerged PR still writes
  `CompletionRequested`; any `--children` or open child refuses with the
  unmerged-PR error and writes nothing.
