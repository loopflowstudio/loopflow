# Work Isolation and Integration

Work starts in the right place and stays there. Waves, loops, projects, and
tasks have explicit execution and delivery boundaries, so parallelism never
trades away ownership and related work can still become one coherent PR.

## KRs

- A new wave can ship its first simple PR from one human request with zero
  explicit branch, worktree, placement, or PM commands; the runtime creates at
  most one task worktree, creates its Linear record before execution, passes
  that id through the formal task lifecycle, and starts one Wave-supervised
  Task Session with no task-specific server.
- For one month of normal operation, no live wave, loop, or worker changes its
  worktree path or checked-out branch because another run starts, lands,
  combines, or is deleted.
- A Wave stays directly steerable in its permanent home while every concrete
  change runs in a durable child Task Session; Wave-to-Task steering,
  interruption, completion, and recovery succeed N/N times across Codex,
  Claude, and OpenCode with honest live-versus-queued behavior.
- Every task execution is attached to an existing Linear issue before work
  begins and reports its execution worktree, lineage, and delivery target; a
  month of runs requires zero manual worktree creation, roadmap reconciliation,
  or branch repair to correct placement.
- Every project execution is attached to an existing Linear project, advances
  its KRs by selecting or creating Linear tasks before edits begin, and creates
  no permanent project branch or worktree.
- A Task Session survives provider/process restart and PR review in the same
  worktree and becomes complete only when merged or explicitly abandoned; a
  month of runs loses zero review context or unintegrated work.
- Independent work targets `main` unless a different integration boundary is
  explicitly selected; a month of normal shipping introduces zero accidental
  wave-level merge gates or repeated conflict resolutions.
- CLI, Swift, automation, and workers use `lf` as the single command surface
  for starting and delivering work; one month of runs has zero dependency on
  an `lfd` exec proxy or alternate `lfq` binary.
- Parallel contributors to one change each write in an isolated worktree and
  produce one reviewable PR through the declared integration target, with zero
  intermediate PRs, concurrent writers, or manual cherry-picks across ten
  representative batches.
- Independent and dependent tasks reliably become, respectively, independent
  PRs or one explicit dependency edge, and completed worker cleanup never
  removes an active owner or loses unintegrated work across a month of landings.
