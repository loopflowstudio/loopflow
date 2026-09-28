# Open design questions

2026-09-28 — Scheduled Task, Wave and repository operation, discussed with Jack Heart.

Launch-plan decision: Product owns the core, per Jack. Core defaults and proof
are in `task-automation.md`; the independent VSM outcome is in `recursive-vsm.md`.
Prepare both directly stacked on LOO-298. The attempted direct rebase was
aborted because it replayed unrelated newer-main commits into LOO-298's model.
Use one minute repository ticks, existing selected-work eligibility, and one
automatic timeout-only rerun per incident as implementation defaults, not as
newly attributed decisions by Jack. The workers may refine these with evidence
while preserving the accepted behavior and human review boundaries.

Settled: one repository-wide `vsm-operate`, alongside per-Task task-operate and
per-Wave wave-operate. Projects are part of Waves and have no operator.

VSM is recursive; Jack explicitly places S1–S5 within each Wave. Identity is
determined through the whole graph; repository vsm-operate is not its exclusive
owner. task-operate selects and runs the right Flow and carries work through to
landing while preserving intended steps and review gates.

- How do Task findings, Wave judgments and repository synthesis change shared
  identity over time? How do the five functions translate into concrete actions
  at each scope, and how do proposals and decisions travel between scopes?

- What decisions can the repository VSM pass execute directly, and which should
  become proposals for Jack or work for a Wave? Exact Flow steps remain open.

- Proposed Task trigger: frequent cron with a mechanical filter; skip active
  Task Sessions/Flow executions and wait for unresolved human Sessions. An
  admitted task-operate launches or recovers a Flow and hands execution over.
  Which final LOO-298 claim covers admission, and which evidence proves active
  execution versus an interrupted or parked invocation?
- Landing should return and scheduled checks should admit CI repair. How does
  the existing Flow record distinguish successful handoff from actual merge,
  including any later steps that require merge? Consolidate `land`/`arm` without
  retaining a second watcher. What happens for a PR without a Task?
- Scheduled Task closure is in scope. Verify existing settlement can resume
  partial local/Linear completion, and keep pending closure discoverable after
  all Flow/landing processes exit; reuse it rather than adding another owner.
- CI timeout proposed at 30 minutes from published-head CI wait, including queued
  or absent expected checks. Specify the bounded retry policy for repeated
  timeout/rerun attempts; unchanged ticks must not create a repair loop.
- Which Tasks receive ticks: all unfinished Tasks, or only Tasks selected to run?
  What starts work, and what remains intentionally queued or awaiting review?
- What cadence and operating budget should Task, Wave and repository passes have?
- Desktop installs/manages Task scheduling. Should the installed job perform
  one finite repository Task sweep (proposed), or should each Task have its own
  OS job? Which UI enables it, and does it also manage Wave/VSM schedules?
- Settled chat topology: channel per Wave/subwave, thread per Task. Specify
  provisioning, archive/reopen and cross-Wave Task transfer behavior.
- Discord thread messages and Linear comments are automatically injected into
  every non-interactive Session registered to the Task, including independent
  Sessions. Define source ingestion and per-recipient delivery progress, restart
  and late-binding semantics, and reply ownership. Interactive Sessions are
  excluded. One recipient must not consume another's input.
- Product owns both selected outcomes. Discord delivery and Wave/VSM schedule
  integration are named follow-ups only; file after the core contract settles.
