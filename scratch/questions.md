# Choices made at kickoff, concept review, and build, for review

- **Explicit parent completion runs immediately instead of pending on `tasks.completion_request`** (concept review, built):
  A refused `--children abandon` or `--children rehome` writes nothing to
  SQLite and answers `conflict`, so background reconciliation can never
  silently abandon or rehome children later, and there is zero version skew
  with released `lf`. Passed over: storing `ChildPolicy` on
  `completion_request`.
- **Live-child guard ignores idle held interactive conversations** (concept review, built):
  `--children abandon` blocks on running Graphs, headless runs, and active
  turns (`terminal_retention` present), but allows abandoning a child sitting
  idle at an interactive step (e.g. `ship-review`) whose held conversation
  `close_conversations` cleanly closes. Passed over: raw
  `task_execution_blockers`.
- **Bare refusal lists the open descendant tree, indented, with direct-child count** (concept review + implement, built):
  "LOO-12 has 3 open children" counts direct children while the indented tree
  shows every descendant (including grandchildren under a merged child and
  `running` state for active children). Passed over: direct children only, or
  counting every descendant in the header.
- **Rehome targets the nearest open ancestor along visible parents, stopping at deleted ones** (concept review pass 2):
  Skips closed (`done` / `abandoned`) grandparents so open children never land
  under a dead parent, and stops at a deleted ancestor (`planning_deleted_at IS NOT NULL`)
  so children become top-level rather than jumping under a grandparent nobody
  sees the parent attached to — matching `task_hierarchy` and Desktop's
  `WorkModel.rehomeDestination`. Passed over: walking through deleted rows
  (diverged from Desktop and `lf wave show`) or adding a new DTO field on
  `RoadmapTask`.
- **Desktop default button follows safety** (kickoff + concept review, built):
  When any open descendant would be abandoned, default is non-destructive
  `Complete & Move Children to …` and `Complete & Abandon Children` is
  `.destructive`. When every open descendant's PR is already merged, default is
  non-destructive `Complete & Finish Children` with `Move Children` second.
  Passed over: always defaulting to Move or abandon-first.
- **iOS learns about children via `ViewContent.open_children: Option<u32>`** (kickoff + concept review, built):
  Matches `ViewContent`'s other Task-only optional fields (`None` for Waves,
  `Some(n)` for Tasks) so iOS swipe asks before writing. Its dialog says
  "Move Children Up" without naming a destination.
- **Live-child guard refuses the whole completion** rather than abandoning idle
  children and skipping the live one (kickoff, built).
- **Leaf with an unmerged PR keeps the pending request** (implement, built):
  bare `lf task complete` on a childless Task with an unmerged PR still writes
  `CompletionRequested`; any `--children` or open child refuses with `conflict`
  and writes nothing.
- **Reopening an abandoned Task archives its closed PR and clears a deleted checkout** (concept review pass 2, proposed):
  When `lf task reopen` reopens an abandoned Task, `reopen_task` marks its
  abandoned PR (`abandoned_at IS NOT NULL`) as `historical = 1`, and
  `task_reopen` clears the stale `task_checkouts` row when abandoning already
  deleted both the worktree and local branch on disk. Status immediately
  recommends `run` (not "PR closed without merge"), Desktop's Run / Start /
  Auto-Ship buttons enable immediately (no missing-worktree blocker), `lf task
  complete` works if no new PR is needed, and `lf task run` places a fresh
  checkout from current `origin/main` while preserving any surviving checkout
  or local branch if cleanup had failed. Passed over: keeping the dead PR as
  `historical = 0` and patching `restore_task_checkout` around it (left Desktop
  Run disabled, status claiming "PR closed without merge", and `lf task
  complete` refused on the dead PR).
- **Completion refusals answer `conflict`** (qa + concept review pass 2):
  Nothing is written, so `outcome_unknown` ("It may have happened") is wrong.
  Both pre-flight refusals and `complete_task_now` transaction refusals return
  `OpsError::Refused(Error::conflict(...))`.
