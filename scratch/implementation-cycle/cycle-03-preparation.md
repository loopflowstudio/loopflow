# Parent preparation for the next bounded presentation slice

Read-only findings while cycle-2 compress/review runs; this is not a replacement
for the governing design or permission to implement the unreviewed two-loop
composition. No source edits are claimed here.

Current native frame still renders all repository roots and places search at
the top (`WorkspaceNavigator`). `WorkSurfaceView` still renders Task directive
labels, raw Description text, snapshot copy and the chapter section on Task.
The accepted website remains the composition reference; its simulated execution
and comments must not become product evidence.

Shared data seams that need reconciliation before the accepted working-set and
Task situations can be truthful:

- RoadmapTask carries planning, reference, runtime, condition/actions and PR.
  Runtime contains Work status/reason, not execution history or a pinned Flow.
  `ops/task_execution.rs::TaskExecutionSnapshot` is the existing current-worker
  projection (idle/starting/running/human/blocked/unknown). Its current step is
  insufficient by itself to declare never-started or to draw all occurrences.
- Chapter classification already uses `TaskStartEvidence`, the durable Started
  event and independent Run evidence. Reuse its evidence owners rather than
  equating preparation/worktree existence or a live process with “started.”
  Its cold per-Task Run scan must not be copied into a frequent desktop read.
  Prepared human Runs are identity without provider execution; preserve that
  distinction. Inspect generic Task-bound first launches as well as managed
  worker claims before choosing a shared started-work projection/writer.
- The existing Flow loader and saved `ExecutionCursor`/pinned invocation own
  topology and occurrence identity, including nested branches and both decision
  edges. Extend their read contract; no Swift-owned Flow parser or status matrix.
  User-approved final Advance composes queue then land in Feature; current
  stored invocations keep their pinned definitions.
- LinearClient::observe_issue already pages actual comments, including explicit
  missing-page errors, and returns IssueObservation/IssueComment. These are
  provider-internal ingestion facts, not yet a desktop JSON/read contract. Reuse
  that reader with truthful counts/missingness and existing PM configuration;
  do not invent a second comments store or convert Description paragraphs.
- Existing Task operations own prepare/run/restart/resume. New session must use
  the normal scoped conversation destination without starting the managed Flow.
  The Task-page action belongs beside its title; no provider is launched by a
  page read or selection alone.

Suggested next scope after cycle-2 review: one coherent frame/Wave/Task
presentation increment plus the minimum shared evidence it actually consumes.
Keep the complete remaining Flow/comments/control obligations explicit. Preserve
native Session breadcrumbs/rename and all retained surfaces. No simulated data,
blank fake count, placeholder-success control or a second navigation owner.
