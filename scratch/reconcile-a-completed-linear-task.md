# Reconcile a completed Linear Task whose Session still needs directive acknowledgement

## Problem

A Task can merge a `CompleteTask` PR and go Linear-complete, yet its Session
never reaches a terminal status — because the body was interrupted **after** the
final directive was applied and **before** its acknowledgement was recorded. The
Session is left non-terminal with `current_directive_version >
incorporated_directive_version`, and every supported actor is barred from closing
the loop:

- **Completion refuses.** `task_completion_gate` blocks while
  `has_pending_directive(session)` holds: *"directive vN is not yet incorporated;
  acknowledge it or re-steer before completing."*
- **Acknowledgement refuses.** `task_acknowledge` requires `LF_TASK_SESSION_ID`
  to equal the owning Session id **and** a live body write lease. A terminal-ish
  Session has no body, so nothing can run inside it.
- **A body cannot be started.** For a merged `CompleteTask` PR the supervisor's
  recovery has no correct branch — `relaunch_inactive_process` fails *"terminal
  Task Sessions cannot start a process"*, and rotating a serial successor to carry
  the directive would mint another PR and launch another provider, both forbidden.

The orphaned recovery command stays `Persisted` with no generation, indefinitely.
This is the exact "no-stranded-Task" failure the Developer Efficiency KR forbids:
a durable command orphaned against a dead generation with every supported owner
barred from acting.

**Live reproduction: ENG-29** (PR #1083, directive v12 fully implemented and
verified, acknowledge barred outside the body, resume barred after merge). The
regression seeds two deterministic reproductions: **W2-308** (directive v5) and
**W2-127** (directive v2, whose historical Project Session is intentionally
abandoned — proving a Wave/Operator can reconcile without resurrecting it).

## The demo

```
$ lf task reconcile ENG-29 --directive 12 \
    --summary "Directive v12 shipped and verified in PR #1083; the ack turn was interrupted."
ENG-29 reconciled: directive v12 incorporated by out-of-band attestation,
Task completed, 1 orphaned command cleared.

$ lf task status ENG-29
ENG-29  completed   directive v12 incorporated   no pending commands
```

A Wave or a human — never the body, never a hands-off supervisor — attests the
final directive by name with a semantic summary, and the Task settles truthfully:
no new PR, no provider turn, no duplicate command.

## Approach

A supported **out-of-band reconciliation** verb, `lf task reconcile <issue>
--directive <version> --summary <text>`, plus a `Reconcile` legal action and a
supervisor branch that recognizes the shape without acting on it hands-off.

Two load-bearing distinctions:

> **Delivery is not incorporation.** `applied_at` proves a body *ran* under the
> directive; it never proves the direction was *adopted*. The incorporation
> evidence is an explicit Wave/Operator **attestation** — the caller names the
> exact current directive version and a non-empty semantic summary, and that
> summary becomes the incorporation record. Predicates only decide whether
> reconciliation is *permitted*.

> **A `CompleteTask` publication is intent, not proof of Linear completion.** The
> publication write can fail (ENG-29/ENG-75), so reconciliation force-refreshes
> the owning team's *actual* Linear issue and requires `completed == true`. The
> PM/GOAL root is the Wave's canonical repo (`wave.repo()`), never
> `session.worktree` — this command targets dead Tasks whose worker worktree may
> already be pruned, and PM ownership is Wave-durable.

### `task_reconcile(issue, authority, version, summary)`

1. **Authority.** Only `Operator` or the owning `Wave` may attest. A `Project` is
   the automatic actor — it may recommend `Reconcile` or consume a durable
   attestation, but never *create* one.
2. **Guards** (each refusal names the exact failing fact): `version` equals the
   current directive; non-empty summary; the current directive is applied
   (`applied_at` set) but unincorporated; no active body (status + live tmux); no
   active PR; the **newest PR by sequence** is a merged `CompleteTask`; and the
   **Linear issue is actually complete** (live force-refresh from `wave.repo()`).
3. **Record the attestation as incorporation.** Out-of-band
   `store.incorporate_child_directive` (no lease, no `LF_TASK_SESSION_ID`); the
   summary becomes the incorporated summary. A distinct `DirectiveReconciled`
   event names the attester, so the trail never reads as a forged body ack.
4. **Complete** through the shared `settle_completed_session` — the clean-tree
   check, completion gate, PM writeback, and terminal transaction are all
   preserved unchanged.
5. **Clear the orphan** `Persisted` recovery command in place
   (`reject_persisted_child_command`); no new or superseding command.

### The newest-PR-by-sequence invariant, in *every* consumer

`latest_pr_completes_task(&[TaskPr])` is the one definition of "this Task settled
on a completing merge": the PR with the highest `sequence` must be a merged
`CompleteTask`. An older `CompleteTask` behind a later `Review`/next-PR merge is a
serial successor, not a Task settling, and must never authorize reconciliation.
This one predicate governs:

- the **action model** recommendation (`latest_pr_after_merge == CompleteTask`);
- the **command guard** in `task_reconcile`;
- the **supervisor suppression** in `reconcile_project_tasks`;
- **automatic completion** — `merged_completing_pr` selects the newest PR and
  returns it only when the predicate holds (so `advance_completion_after_gate`
  cannot complete from an older disposition).

The duplicate `task_prs(..).any(merged CompleteTask)` logic is deleted from all
of them.

### Supervisor: recommend or consume, never create; cheap checks first

`reconcile_project_tasks` gates on the cheap session-local predicates —
`has_pending_directive(task)` then `current_directive_applied` — **before**
loading PR history, so the common non-terminal loop pays no extra `task_prs`
query. Only that rare shape reads PRs and consults `latest_pr_completes_task`. A
durable attestation is consumed by `reconcile_task_completion` (the gate clears
and the Task completes); otherwise the supervisor leaves the `Reconcile`
recommendation standing and never fabricates the summary.

## Key decisions

- **Attestation, not inference.** The incorporation evidence is a human/Wave
  summary naming the exact current version; predicates only gate permission.
- **Authority is `Wave`/`Operator` only.** A retired Project is never resurrected.
- **Linear completion is proven live from the Wave repo**, not inferred from the
  PR publication and not read through a possibly-pruned worker worktree.
- **Newest-PR-by-sequence is one shared predicate**, used by four consumers with
  no duplicated disposition logic.
- **One existing row cleared, none created.**
- **Gate proofs preserved.** Reconciliation does not weaken the clean-tree or
  completion gate and invents no missing-worktree completion policy.

## Done when

- `lf task reconcile <issue> --directive N --summary "…"` records the attestation,
  completes the Task, and clears the orphaned command — no PR, no provider turn,
  no duplicate command.
- Reconcile refuses when: the version is not current; the summary is empty; the
  directive was never applied; a body is live; the caller is a `Project`; the
  newest PR is not a merged `CompleteTask` (older-CompleteTask-behind-later-Review
  included); or the Linear issue is not actually complete.
- Legal-action reporting recommends `Reconcile` only for a merged `CompleteTask`
  latest with an applied, unincorporated directive; a merged `Review`/next-PR
  latest stays `StartNextPr`.
- The supervisor never errors on the shape, never fabricates an attestation, and
  pays no PR-history query outside the rare reconcilable case.
- Regressions cover: the W2-308 and W2-127 happy paths; the guard refusals; the
  Linear-incomplete refusal; the PM root resolving to `wave.repo()` not a dead
  worktree; the multi-PR sabotage (direct command + supervisor +
  `advance_completion_after_gate`); and the `latest_pr_completes_task` predicate.
- `cargo fmt`, `cargo clippy -- -D warnings`, and the targeted tests pass. Local
  reconciliation probes use an isolated `LF_DB_PATH`.

## Measure

The Developer Efficiency KR: zero Sessions stranded in a non-terminal state on a
dead generation, and zero durable commands orphaned against one. A merged,
Linear-complete Task reaching `completed` with its recovery command cleared is
the concrete before/after.
