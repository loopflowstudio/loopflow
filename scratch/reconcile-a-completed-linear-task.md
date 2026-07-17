# Reconcile a completed Linear Task whose Session still needs directive acknowledgement

## Problem

A Task can merge, disposition itself `CompleteTask`, and go Linear-complete, yet
its Session never reaches a terminal status — because the body was interrupted
**after** the final directive was applied and **before** its acknowledgement was
recorded. The Session is left `Waiting` with `current_directive_version >
incorporated_directive_version`, and every supported actor is barred from closing
the loop:

- **Completion refuses.** `task_completion_gate` pushes a blocker while
  `has_pending_directive(session)` is true: *"directive v5 is not yet
  incorporated; acknowledge it or re-steer before completing."*
  (`ops/task.rs:4330`).
- **Acknowledgement refuses.** `task_acknowledge` requires `LF_TASK_SESSION_ID`
  to equal the owning Session id **and** a live body write lease
  (`ops/task.rs:5460`). A terminal-ish Session has no body, so nothing can run
  inside it.
- **A body cannot be started.** For a merged `CompleteTask` PR the supervisor's
  recovery has no correct branch: `relaunch_inactive_process` fails *"Task W2-308
  is waiting; terminal Task Sessions cannot start a process"* (`ops/task.rs:1834`),
  and the pending-directive rotation path (`ensure_working_pr_with_authority`,
  `ops/task.rs:3699`) would mint another serial PR and launch another provider —
  both forbidden by the human directive.

The orphaned terminal recovery command `cc_a172ed28…` stays `Persisted` with no
generation, indefinitely. This is the exact "no-stranded-Task" failure the
Developer Efficiency KR forbids: a durable command orphaned against a dead
generation with every supported owner barred from acting.

W2-308 (PR #1064, directive v5) is the live reproduction; W2-127 (PR #876,
directive v2) is a second deterministic one whose historical Technical
Architecture Project Session is intentionally abandoned.

## The demo

```
$ lf task reconcile W2-308 --directive 5 \
    --summary "Directive v5 shipped in the merged head; ack turn was interrupted."
W2-308 reconciled: directive v5 incorporated by operator attestation,
Task completed, recovery command cc_a172ed28 cleared.

$ lf task status W2-308
W2-308  completed   directive v5 incorporated   no pending commands
```

A Wave or a human — never the body, never a hands-off supervisor — attests the
final directive by name with a semantic summary, and the Task settles truthfully:
no new PR, no provider turn, no duplicate command.

## Approach

Add a supported **out-of-band reconciliation** verb, `lf task reconcile <issue>
--directive <version> --summary <text>`, plus a `Reconcile` legal action and a
supervisor branch that recognizes the shape without acting on it hands-off.

The load-bearing distinction, set by the revised direction:

> `applied_at` proves **delivery**; a merged `CompleteTask` PR proves **shipped
> work**. Neither proves **semantic incorporation**. Reconciliation must not
> synthesize an incorporation receipt from those predicates.

So the predicates are **guards**, not evidence. The *evidence* of incorporation
is the explicit Wave/Operator attestation: the caller names the exact current
directive version and writes a non-empty semantic summary, and **that summary
becomes the incorporation record**. This mirrors the wave-memory learning that a
generation or PR head never proves the current directive was adopted — a separate
incorporation receipt is required.

### `task_reconcile(issue, authority, version, summary)`

1. **Authority.** Resolve `CallerAuthority` at the CLI surface (as the other
   control ops now do since #1079). Accept only `Operator` or the owning `Wave`.
   Reject `Project` — the automatic actor must never *create* the attestation.
   Validate via `validate_caller_authority` (`ops/util.rs:144`); the resulting
   `ChildCommandSource` is the audited attester.
2. **Guards (all must hold, else refuse with a specific message).**
   - `version == session.current_directive_version` (name the *exact* current
     directive; a stale version is refused, like `task_acknowledge`).
   - `summary` non-empty after trim.
   - The current directive is **delivered**: its `ChildDirective.applied_at`
     is `Some` (a body ran under it) and `incorporated_at` is `None`.
   - The latest PR is **merged** with `after_merge == CompleteTask`.
   - **Linear complete** — the merge dispositioned the Task complete (the
     `CompleteTask` publication is the durable proof; reconcile also reconciles PR
     state first so a stale row can't pass).
   - **No active body**: `!status.is_process_active()` and no live tmux for the
     latest process.
3. **Record the attestation as incorporation.** Call the existing out-of-band
   `store.incorporate_child_directive(target, version, summary)`
   (`store/child_sessions.rs:1334`) — no lease, no `LF_TASK_SESSION_ID`. This sets
   `incorporated_at` + `incorporated_summary = <attestation>`. Append a new
   `TaskEventKind::DirectiveReconciled { directive_id, version, summary,
   attested_by }` so the audit trail shows an out-of-band Wave/Operator
   attestation, distinct from the body-authored `DirectiveIncorporated`. This is
   how we settle **without impersonating the Task**: the incorporation is real and
   attributed, not a forged body acknowledgement.
4. **Complete.** With `has_pending_directive` now false the completion gate
   clears; complete the Session out-of-band (lease `None`, the path
   `task_complete` already takes when not inside a body).
5. **Clear the orphan.** `reject_persisted_child_command(cmd_id, reason)`
   (`store/…:1915`) terminalizes the `Persisted` recovery command to `failed`
   with *"reconciled out of band; directive vN attested by <authority>"*. It
   touches **only** the existing row (`WHERE state = 'persisted'`); it never
   creates or supersedes-with a duplicate.

### `Reconcile` legal action

Add `TaskAction::Reconcile` to the model (`task/actions.rs`) and a
`directive_applied: bool` field to `TaskActionEvidence`. In `merged_pr_model`,
when the completion refusal is the pending-directive one **and** the directive is
already applied (`directive_applied`), recommend `Reconcile` and block `Resume`
and `StartNextPr`, naming the attestation requirement in their reasons. This is
the named fix for *"legal-action reporting stops recommending resume when the
current human directive forbids a provider turn."*

A pending directive that is **not** applied (`applied_at.is_none()` — a genuine
post-land steer that still needs work) keeps today's `StartNextPr`
recommendation. Only the applied-but-unincorporated shape reconciles.

### Supervisor: recommend or consume, never create

`reconcile_project_tasks` (`ops/task.rs:2163`) gets one guarded branch for the
applied-but-unincorporated + merged-`CompleteTask` shape:

- **Do not** relaunch a body, rotate a serial PR, or queue a CI-fix for it (these
  are the paths that error today).
- **Consume** an already-durable attestation: if the directive was already
  incorporated out-of-band, the pending-directive blocker is gone, so the
  existing `reconcile_task_completion` → `advance_completion_after_gate` completes
  the Task hands-off (and clears any lingering orphan). This is the *consume*
  half.
- Otherwise **leave the recommendation**: surface `Reconcile` (via the action
  model) and stop. The supervisor never fabricates the semantic summary.

Also gate the pending-directive rotation in `ensure_working_pr_with_authority`
(`ops/task.rs:3699`) on `applied_at.is_none()`: an applied-but-unincorporated
directive must not mint a successor PR. A not-yet-applied post-land directive
still rotates as before.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does an out-of-band, lease-free incorporation path already exist? | Yes — `store.incorporate_child_directive` (non-lease) sits beside `incorporate_child_directive_for_lease`; `mark_child_directive_applied` too. | Reconcile reuses it; no new store primitive for incorporation. |
| Can a `Persisted` command be terminalized with no lease/generation? | Yes — `reject_persisted_child_command` updates `WHERE state='persisted'` → `failed`, no lease. Its `changed==0` is a genuine "no longer persisted", so it never races a body that took ownership. | Reconcile clears `cc_a172ed28…` directly; no duplicate command. |
| Does `applied_at` prove the directive was *incorporated*? | **No.** It is set when a turn *starts* under the version (`task/runner.rs:1448`). It proves the body ran under vN, not that its meaning was integrated. A merged head is delivery, not adoption. | Predicates are guards only. The Wave/Operator `--summary` is the incorporation evidence; no predicate synthesizes it. |
| Can completion run out-of-band once the directive is incorporated? | Yes — `task_complete` resolves `lease = ambient_task_write_lease(session)`, which is `None` outside a body, and completes without one. | Reconcile calls the same settle path; no reshaping for tests. |
| Does W2-127's abandoned Project block reconciliation? | Yes for a `Project` caller — `validate_caller_authority` refuses a Project that is not the Task's live route, and W2-127 has no live route. | Reconcile is authorized by `Wave`/`Operator`, so a retired Project is never resurrected. |
| Could reconcile silently erase a genuinely un-applied post-land directive? | Only if the guards let an `applied_at.is_none()` directive through. | Guard requires `applied_at.is_some()`; the not-applied case keeps `StartNextPr`/rotation and a real body. |
| Residual: applied directive whose turn produced no merged output? | Narrow — requires a directive applied on a turn that shipped nothing while an *earlier* turn's head merged. The merged-`CompleteTask` + applied + explicit-summary combination still forces a human/Wave to look and attest. | Accepted: the human attestation is the backstop, not the predicate. Noted, not engineered around. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Auto-incorporate from `applied_at` + merged PR (my first draft) | Zero human touch; supervisor settles hands-off | **Rejected by direction:** conflates delivery with semantic incorporation; lets a lost ack turn silently become "adopted." |
| Let the supervisor synthesize a placeholder summary and incorporate | Also hands-off | Same conflation; an unaudited machine-authored summary is exactly the forged receipt the direction forbids. |
| Relax `task_acknowledge` to run out-of-band | Reuses one verb | Acknowledgement is the *body's* receipt; loosening its `LF_TASK_SESSION_ID`/lease guard would let any process forge a body acknowledgement. Reconcile is a distinct, attributed authority. |
| Weaken the completion gate to ignore pending directives on merged Tasks | One-line change | Silently erases direction accepted after `lf pr land` armed auto-merge — the precise regression the gate exists to prevent. |

## Key decisions

- **Attestation, not inference.** The incorporation evidence is a human/Wave
  summary naming the exact current version. Predicates (`applied_at`, merged
  `CompleteTask`, Linear complete, no active body) only decide whether
  reconciliation is *permitted*, never whether it *happened*.
- **Authority is `Wave`/`Operator` only.** A `Project` may recommend `Reconcile`
  or consume an already-durable attestation; it may never create one. This keeps
  semantic handoff a judgment, not an automatic side effect.
- **Distinct audit event.** `DirectiveReconciled { …, attested_by }` records who
  attested out-of-band, so the trail never reads as a body acknowledgement.
- **One existing row cleared, none created.** The orphan `Persisted` command is
  terminalized in place; reconciliation adds no command.
- **Rotation guard tightened.** The pending-directive successor rotation fires
  only for a *not-yet-applied* directive; an applied one awaits attestation.

## Scope

- **In scope:** `lf task reconcile` op + CLI; `TaskAction::Reconcile` +
  `directive_applied` evidence; `DirectiveReconciled` event; supervisor
  recommend/consume branch; rotation guard on `applied_at`; W2-308 and W2-127
  behavioral regressions; guard/refusal regressions.
- **Out of scope:** any change to `task_acknowledge`'s in-body authority; a
  generic multi-product deploy platform; modifying or abandoning W2-127
  operationally from this body; resurrecting the Technical Architecture Project.

## Done when

- `lf task reconcile W2-308 --directive 5 --summary "…"` records the attestation
  as incorporation, completes the Task, and clears `cc_a172ed28…` — creating no
  PR and no duplicate/superseding command.
- `lf task reconcile` refuses when: the directive version is not the current one;
  the summary is empty; the current directive is not applied; the latest PR is
  not a merged `CompleteTask`; a body is live; or the caller is a `Project`.
- Legal-action reporting for the applied-but-unincorporated merged shape
  recommends `Reconcile` and blocks `Resume`/`StartNextPr`.
- The supervisor no longer errors on the shape: it recommends `Reconcile` and,
  once an attestation is durable, completes hands-off — but never fabricates the
  attestation.
- A regression exercises the full path for **both** W2-308 (v5) and W2-127 (v2):
  final directive applied → PR merged (`CompleteTask`) → Linear complete → body
  interrupted before ack → Project recovery attempted (recommends `Reconcile`, no
  relaunch) → Wave/Operator attests → Session settles terminal, orphan cleared.
  W2-127's attestation summary is that the provider stayed stopped, no
  branch-built `lf` touched the live registry, and supervisor-owned
  implementation merged as PR #876.
- `cargo fmt`, `cargo clippy -- -D warnings`, and the targeted tests pass. Any
  local reconciliation probe runs against an isolated `LF_DB_PATH`, never the
  live registry.

## Measure

Not a quantitative change. The observable outcome is the Developer Efficiency KR:
zero Sessions stranded in a non-terminal state on a dead generation, and zero
durable commands left orphaned against one. W2-308 reaching `completed` with
`cc_a172ed28…` no longer `Persisted` is the concrete before/after.
