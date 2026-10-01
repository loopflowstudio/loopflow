# Primary Sessions (LOO-364)

Status: draft, revised 2026-10-01. Restated from Unit 2 of
`loopflow.growth-thoughts/scratch/growth-thoughts.md`, which was written
against Runs and a `PrimarySession { scope, run_id, replacement_run_id }`
pointer record. Jack Heart moved the unit out of LOO-353 on 2026-10-01; Desktop
presentation stays there.

## Outcome

Jack talks to one ongoing conversation per repository and per Wave. The
conversations find themselves and reconcile on entry with the same check the
schedule runs. They are conversations with Jack, not owners of repair.

## Decisions (Jack Heart, 2026-10-01)

- **No TaskSession owns CI repair, and nothing wakes a conversation.** "i think
  theres probably no need to have the task session do it then, you can just
  start a simple ci-fix skill or whatever." The clock starts one plain `ci-fix`
  run as the Task. This replaces his earlier same-day TaskSession-as-owner
  call. The TaskSession, Flow switching through it, outbox delivery into a live
  conversation and scheduled wakes are removed from this Task.
- **The clock is replaceable.** LOO-332's minute cron (#1382) is the clock now;
  the LOO-365 watcher replaces it later. The repair entry point is independent
  of its caller.

## The model

A primary Session is an ordinary interactive `conversation` AgentSession.
Primary is a nullable attribute of the row (`primary_scope`); the scope's
identity is in columns the row already has (`repo`, `wave_id`). The current
primary of a scope is its one uncompleted row, stated by a partial unique index
per scope. Completed predecessors remain ordinary history. Membership,
attribution, title, rename, connect and complete are the existing Session
operations and grant no Flow, review or process authority.

- **Repository Session** — `primary_scope='repository'`, keyed by the canonical
  repository (linked worktrees collapse to the main checkout). Works with zero
  Waves, zero Tasks and unavailable planning. Skill `repo/session`.
- **Wave Session** — `primary_scope='wave'`, `wave_id`. Launched with the Wave
  selector, so goal and memory arrive as ordinary Wave context (Jack,
  2026-09-30). Skill `wave/session`.

`lf session ensure [-w WAVE]` finds or admits the scope's primary and starts its
durable terminal once; a failed start keeps the Session for the next ensure.
`lf session replace ID` stops the predecessor's provider, then completes it and
admits the successor in one transaction; a repeat returns the same successor.

## CI repair claim: already present, unchanged

The direct repair #1382 shipped already has the claimed entry point Jack asked
for, so this Task adds none.

- **One claim per PR, head and failure.** `ci_incidents.identity` is
  `github:ci:<repo>:<pr>:<head>:<digest of failing check names and URLs>`.
  `admit_ci_fix` takes the cross-process checkout admission lock, reads the
  reservation, and returns without launching when its Exec is live or
  unresolved. `reserve_repair` writes the claim in one immediate transaction,
  fenced on the landing generation. A second clock seeing the same failure
  finds the reservation.
- **Durable attempt record.** `repair_exec_id`, `repair_session_id`,
  `repair_retries`, `repair_finished_at`, `repair_error`, `repair_conclusion`.
- **Retry once, then surface.** A repair that died or errored without a
  conclusion is retried while `repair_retries < automation.retries` (default
  1), reusing the reserved Session; after that the landing blocks with "repair
  startup exhausted automatic retries". A concluded repair (`published` or
  `blocked`) is never rerun on unchanged evidence: the landing blocks with
  "waiting for changed evidence".
- **Caller-independent.** The claim lives in `pr_landing`, reached by
  `lf pr reconcile` / `lf task reconcile`; the cron only calls them.
- Coverage: `lf_pr_land_returns_before_later_checks_repair_and_observe_merge`
  exercises an overlapping second check during a live repair and a failed
  start retried on the reserved Session.

Observed, not changed: the identity includes failing check URLs, so a provider
rerun of the same head that fails again is new evidence and may get its own
repair. That matches "unchanged failure" literally; tighten to head-only only
if reruns are seen producing repeat repairs.

## Remaining

1. Scope checkout for design writing (`ensure_agent_worktree` with a local
   base) so primary conversations stop sharing the main checkout.
2. `primary_scope` on the Session DTO, with its first Desktop consumer
   (LOO-353).

Not in this Task: a per-Task conversation with Jack (the original LOO-353
meaning of TaskSession) has no accepted design here; Desktop discovery calling
ensure is LOO-353.

### Delete — do not maintain

Nothing. The consumerless `pending_observations` /
`mark_observation_delivered` stay as #1360 left them; this Task no longer gives
them a consumer.

## Checks

`cargo test -p loopflow --lib primary::tests` — 5 passed (2026-10-01); clippy
and fmt clean. Gate owns affected suites; a real provider launch belongs to
demo.
