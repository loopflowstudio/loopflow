# Make Task command caller authority explicit

## Problem

A Task/Project control command's authority is decided by *mutable ambient
environment*, deep in the ops layer, at the moment the command runs:

- `command_source()` (`ops/task.rs:384`): `LF_PROJECT_SESSION_ID` present →
  `Project`; else `resolve_child_command_source()`.
- `resolve_child_command_source()` (`ops/util.rs:54`): reads `LF_WAVE_ID`;
  owning wave → `Wave`; foreign/stale → loud error; **`NoContext` (absent) →
  `Human`.**

That last arm is the hole. During ENG-19 recovery the *same* `lf task resume`
spelling was refused as a Wave supervisor command with `LF_WAVE_ID` set, then
accepted as a Human/operator command when only that one variable was removed
(`env -u LF_WAVE_ID lf task resume`). An environment edit crossed the open-PR
control boundary: supervisor authority (barred from restarting a submitted
Task — W2-129) silently became operator authority (allowed to restart it to
answer review). Memory records this as a "workflow"; it is a defect.

The live regression is W2-319. A Project review command
`cc_03a5949e9a2649a8b6b99d4a7db08eaa` sat `persisted`, `effect=next_turn`,
`generation=null`. A single Wave-scoped Resume returned the self-referential
open-PR bar **and** persisted a second inert command
`cc_e322b7b22f234d9c87ea82e3371f1654` with `effect=null`, `generation=null`,
while Task generation 2 stayed `finished`. Two failures compound:

- **D1 — authority is ambient.** Removing `LF_WAVE_ID` converts a Wave command
  into a Human command, which flips `launch_intent`
  (`Supervisor`→`ExplicitResume`, `ops/child.rs:665`) and so flips the restart
  bar (`supervisor_restart_bar`, which bars an open PR, → `abandon_intent_reason`,
  which does not).
- **D2 — a barred supervisor resume strands an inert command and self-loops.**
  `queue_command` (`ops/child.rs:642`) *creates and persists* the `ChildCommand`
  (line 734) and appends its `Persisted` event (line 795) **before** `launch()`
  (line 839) evaluates the restart bar. When a supervisor resume hits
  `open_pr_bar`, launch returns `Err`, but the row is already durable — a
  `Persisted` command with no generation, never claimed (the body is finished),
  stranded forever. The bar's own text — "resume it explicitly with
  `lf task resume W2-319`" — is written for a human operator, so a Wave/Project
  reading it re-runs the exact command and strands another inert row.

## The demo

From inside a wave body:

```bash
# Was: escalates to operator and re-opens a submitted Task (W2-129 regression).
# Now: refuses loudly — env stripping cannot mint operator authority.
env -u LF_WAVE_ID lf task resume W2-319
#> refused: this command runs inside a managed session (LF_CHANNEL set) but
#>          carries no resolvable Wave/Project authority. Re-run with the
#>          session identity intact, or from a clean operator shell.

# A Wave supervisor resuming an open-PR Task refuses CLEANLY and names the owner —
# no stranded cc_… behind it.
lf task resume W2-319          # (LF_WAVE_ID set to the owning wave)
#> refused: Task W2-319 submitted pull request #1077 and is in review. A
#>          supervisor cannot restart a submitted Task; this is the reviewer's
#>          to advance. Nothing was queued.
```

Observable proof: the two refusals above, plus the command ledger is unchanged —

```bash
sqlite3 ~/.lf/loopflow.db \
  "SELECT count(*) FROM child_commands WHERE target_id='<ts>' AND state='persisted'"
# identical before and after the barred supervisor resume
```

## Approach

Three moves. One typed authority model, resolved once and failing closed; a
barred supervisor resume that refuses before it writes; and a bar message
written for the audience that actually sees it.

### Move 1 — `CallerAuthority`: one typed input, resolved once, fail-closed

Introduce `CallerAuthority`, the typed answer to "who is issuing this control
command," resolved **once** at the ops boundary (`task_resume`,
`project_resume`, `queue_command` callers) and threaded down as a parameter —
never re-derived from env deeper in the stack.

```rust
pub enum CallerAuthority {
    Operator,                    // a human at the CLI, or any non-managed process
    Wave(WaveId),
    Project(ProjectSessionId),
}
```

It maps 1:1 onto the stored `ChildCommandSource` (`Operator`→`Human`,
`Wave`→`Wave`, `Project`→`Project`). `Attachment`/`System`/`Linear` are
ingestion sources, not caller authorities; they never flow through the resume /
human-control CLI and keep their existing construction.

The two scattered classifiers (`command_source` in `ops/task.rs`,
`project_command_source`→`resolve_child_command_source` in `ops/util.rs`)
collapse into one funnel used by Wave, Project, and operator alike — the single
source of the classification matrix (Done-when #2):

```
CallerAuthority::from_ambient(store, target) -> OpsResult<CallerAuthority>
```

with these rules (evaluated top-down):

1. `LF_PROJECT_SESSION_ID` present → `Project(id)` iff it matches the target's
   Project Session; else "cannot control" error. (today's `ops/task.rs:388`)
2. else `LF_WAVE_ID` present → resolve through the store; owning → `Wave`;
   foreign → "cannot control"; stale/registry → loud error. (today's
   `resolve_child_command_source`)
3. else **any managed marker still present** → **refuse.** Managed markers =
   `{LF_WAVE_ID, LF_PROJECT_SESSION_ID, LF_TASK_SESSION_ID, LF_CHANNEL}`. This
   is the arm that kills D1: a wave body that stripped `LF_WAVE_ID` still
   carries `LF_CHANNEL`; a task body still carries `LF_TASK_SESSION_ID`. The
   command is inside a managed session but its identity env is inconsistent —
   refuse and name the stray marker, never downgrade to Operator.
4. else (no managed markers at all) → `Operator`.

The old "`NoContext` → `Human`" arm becomes "no markers → Operator, but a stray
marker → refuse." That is the entire fix for Done-when #1 and #4: authority is
a typed value produced by an explicit rule, and **removing one env var from a
managed body fails closed instead of escalating.**

`--wave <name>` at the CLI (`bin/lf.rs:1362`) stays the operator's *explicit*
way to assume Wave authority — it resolves to a registered wave row, so it is a
deliberate typed assertion, not ambient inheritance. That is the intended
"explicit typed input from the invocation surface" for a human acting as a wave.

### Move 2 — refuse the barred supervisor resume *before* persisting

In `queue_command`, for a **Resume** whose body is not active, evaluate the
launch bar *before* creating the durable command. Today the order is
create → persist-event → `launch()` (which checks the bar and errors). Reorder
to: check the restart bar for the resolved launch intent; if it refuses, return
the bar error with **no** `create_child_command` and **no** `Persisted` event.

- Operator resume (`ExplicitResume`) is unaffected: its bar is only
  `abandon_intent_reason`, so an open PR still launches (that is how review is
  answered).
- Supervisor / Wave / Project resume (`Supervisor`) of an open-PR Task refuses
  before any write — no `cc_…` stranded, nothing for a re-run to duplicate
  (Done-when #3, "repeating the same command cannot self-loop").

Scope: **Resume only.** `CiFix` deliberately persists-then-bars for incident
attribution (`ensure_child_ci_fix_command` dedups by incident identity, and
`relaunch_on_duplicate` keys on `Persisted`); leave that path untouched. Steer,
follow-up, and interrupt against a *live* body still persist a directive for the
body to consume — the pre-persist bar only guards the *restart* of a non-live
body.

### Move 3 — the open-PR bar speaks to the supervisor who sees it

Because the operator path takes `ExplicitResume` (whose bar is abandon-only),
`open_pr_bar` fires **only** for a supervisor (`Supervisor`/`CiFix`). So its
text can be rewritten wholesale for that audience: name the real next owner (the
reviewer/operator) and stop recommending the self-referential `lf task resume`.

```
Task <id> submitted pull request #<n> and is in review. A supervisor cannot
restart a submitted Task (an open PR is not an invitation to start over).
This is the reviewer's to advance: an operator answering review resumes from a
clean operator shell; if review is blocked, escalate to the owner. Nothing was
queued.
```

The "explicit typed recovery path" the Done-when allows is already present and
needs no new verb: an **operator** resume is legitimately allowed past the
open-PR bar and delivers any pending review directive (e.g. W2-319's persisted
`cc_03a…`) to the next generation via the existing `resume_task_async` directive
delivery. The supervisor's sanctioned outcome is refuse-clean + accurate
ownership; delivery is the operator's job, and forcing it from a supervisor
would reintroduce the W2-129 regression. So Move 3 is: refuse cleanly, name the
owner, and never emit self-loop text.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Do managed bodies carry a marker beyond the primary id, so stripping one var still fails closed? | Yes. Wave body env = `LF_WAVE_ID` + `LF_CHANNEL` (`wave/mod.rs:270`). Task body = `LF_WAVE_ID` + `LF_TASK_SESSION_ID` (+lease/gen) (`ops/task.rs:1961`). Project body = `LF_WAVE_ID` + `LF_PROJECT_SESSION_ID` (`ops/project.rs:491`). | Marker set `{LF_WAVE_ID, LF_PROJECT_SESSION_ID, LF_TASK_SESSION_ID, LF_CHANNEL}` catches every body type after a single-var strip. Move 1 arm 3 is sound. |
| Is the escape truly closable by env sniffing? | No — a local caller can scrub *all* identity, and this machine's trust model lets a local operator be an operator. The floor is inherent. | Scope is ending **ambient/accidental** crossing and the documented *one-variable* escape (Done-when #4's exact words), not defeating a full scrub. State this floor explicitly rather than pretending to close it. |
| Does the operator path already launch past an open PR (so Move 1's reclassification is what closes the escalation)? | Yes. `(Resume, Human)` → `ExplicitResume` (`ops/child.rs:665`), whose bar is `abandon_intent_reason` only — no open-PR bar. | The escalation was purely the Wave→Human misclassification. Fix classification (Move 1) and the operator path keeps working unchanged; supervisor path is handled by Moves 2–3. |
| Does reordering the bar in `queue_command` break live-body steering? | The pre-persist bar guards only `Resume` where `!is_process_active()`. A live body's steer/follow-up/interrupt persist a directive regardless and are not restart paths. | Move 2 changes only the barred-restart case; no regression to live-body control. |
| Could a genuine operator carry `LF_CHANNEL` and get false-refused? | Possible but rare and self-inflicted (`export LF_CHANNEL=foo` with no wave binding). | Accepted, named, recoverable: the refusal names `LF_CHANNEL` and the fix (`unset LF_CHANNEL` or pass `--wave`). Fail-closed beats silent escalation. |
| Does the persisted review directive (`cc_03a…`) need a supervisor to deliver it headlessly? | No supported supervisor delivery exists that respects W2-129; delivery past an open PR is operator-only. The directive's own words permit "refuse before persisting a duplicate inert command" as the supervisor outcome. | No new headless-delivery verb in scope. Supervisor refuses clean; operator resume delivers. A fully headless review-handoff (no operator present) is a separate boundary, noted in `questions.md`. |
| Existing test `command_source_classifies_every_ambient_context` (`ops/util.rs:134`) — does it move? | It is the classification matrix and already covers owning/foreign/stale/absent. | Extend it into the `CallerAuthority` funnel test: add "stray marker, no wave → refuse" and "all markers absent → Operator"; keep the six existing cases. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Add a `--caller wave\|operator` flag to the control verbs | Explicit at the surface | A flag is as forgeable as env and adds a surface a human must remember; the runtime would still have to set it, moving the ambient problem one layer up without closing arm 3's strip case. `--wave` already gives the operator an explicit typed assertion. |
| Separate verbs: `lf task supervise-resume` vs `lf task resume` | Clear intent split | Two verbs for one action; the runtime would still classify by env when choosing which to spawn. Doesn't remove the env-decides-identity property, just renames it. |
| Keep `NoContext → Human`, only fix the bar (Moves 2–3) | Smaller diff | Leaves D1 fully open: `env -u LF_WAVE_ID` still escalates a supervisor to operator and restarts a submitted Task. Fails Done-when #1 and #4. |
| Make managed authority unforgeable (signed token in env) | Actually closes the scrub | Over-engineered for a local, single-operator trust model; a scrubbed token is absent, so it degrades to the same marker-presence question. Defer until a multi-tenant threat model exists. |

## Key decisions

- **Fail closed, not open.** The single behavioral inversion — "managed marker
  present but authority unresolvable ⇒ refuse" replacing "no wave id ⇒ Human" —
  is the whole of D1's fix. Everything else is threading a typed value and
  moving a check earlier.
- **One funnel, one matrix.** Wave, Project, and operator classification share
  `CallerAuthority::from_ambient`, so they cannot drift (Done-when #2). The
  existing `resolve_child_command_source` already unified Task and Project on the
  wave arm; this extends that unification to include the operator arm and the
  fail-closed rule.
- **Refuse before persist, for Resume only.** Surgical reorder in
  `queue_command`; ci-fix's persist-first incident attribution is deliberately
  preserved.
- **The bar addresses its real reader.** `open_pr_bar` only ever reaches a
  supervisor, so it names the reviewer/operator and drops the self-loop text.
- **No new verb.** The operator resume is the sanctioned typed recovery; a
  headless-with-no-operator review handoff is out of scope and filed as a
  question.

## Scope

- In scope: `CallerAuthority` type + `from_ambient` funnel (fail-closed);
  rewire `command_source`/`project_command_source` onto it; pre-persist Resume
  bar in `queue_command`; audience-correct `open_pr_bar` text; regressions for
  Wave/Project/operator resume against an open-PR Task, the `env -u LF_WAVE_ID`
  strip case, and the ENG-19 stranded-body shape.
- Out of scope: a new headless review-delivery verb; changing ci-fix's
  persist-then-bar ordering; the open-PR bar policy itself (W2-129 stands);
  W2-319 mutation, scratch deletion, global-binary promotion, reteam apply, or
  landing PR #1077.

## Done when

- `CallerAuthority` is an explicit typed input resolved once at the ops
  boundary, not inferred by reading `LF_WAVE_ID`/`LF_PROJECT_SESSION_ID` deep in
  the stack.
- Wave, Project, and operator commands derive legal actions and refusals from
  the one `from_ambient` funnel.
- A supervisor barred by the open-PR bar is told the reviewer/operator is the
  next owner; the message recommends no self-resume, and the refusal persists no
  command (verified: `child_commands` count unchanged after a barred Wave
  resume).
- `env -u LF_WAVE_ID lf task resume <open-pr-task>` from inside a managed body
  refuses loudly and is **not** reclassified Operator.
- Tests cover Wave, Project, and operator resume against an open-PR Task,
  including the ENG-19 stranded-body shape, each sabotaging the production
  branch it names (remove the pre-persist bar → a stranded `Persisted` command
  appears; revert the fail-closed arm → the strip case classifies Operator and
  launches).
- `cargo test -p loopflow` green; `cargo clippy --all-targets -- -D warnings`
  clean.

## Measure

Reproduce the W2-319 shape in a regression harness: a submitted Task (open PR,
finished body) with a pending review directive.

- Before: a Wave resume persists an inert `ChildCommand` (state `persisted`,
  `generation=null`) and returns the self-referential bar; an
  `env -u LF_WAVE_ID` resume launches past the open-PR bar.
- After: the Wave resume returns a clean refusal naming the reviewer, leaving
  zero new `child_commands` rows; the `env -u LF_WAVE_ID` resume refuses; the
  operator resume launches and delivers the pending directive to generation N+1.

Directly serves Developer Efficiency's KR "No Task strands on a dead body …
zero durable commands left orphaned 'uncertain' against a dead generation": the
inert `Persisted` supervisor command is exactly such an orphan, and Move 2
prevents it at the source.
