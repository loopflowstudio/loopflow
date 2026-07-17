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

### Move 1 — `CallerAuthority`: one typed input, resolved at the invocation boundary, fail-closed

Introduce `CallerAuthority`, the typed answer to "who is issuing this control
command," resolved **once at the invocation boundary** (CLI arg parse →
`task_resume`/`project_resume` entry) and threaded down as a parameter — never
re-derived from env deeper in the stack.

**Environment still participates — as transport and consistency evidence, not
as an authority oracle.** The change is that authority is no longer *inferred by
absence*: a missing variable never mints a *different* authority. Precise roles:

| Env / input | Role | Who sets it |
|-------------|------|-------------|
| `--wave <name>` (CLI arg) | **Explicit operator assertion** of Wave authority; resolved against the registry, takes precedence | a human at the CLI |
| `LF_WAVE_ID` (inherited) | Transport of a Wave/Task body's stamped Wave identity + consistency evidence | the runtime, when it spawns the body |
| `LF_PROJECT_SESSION_ID` (inherited) | Transport of a Project body's session identity | the runtime |
| `LF_TASK_SESSION_ID`, `LF_CHANNEL` (inherited) | Consistency evidence only — "this process is inside a managed body" | the runtime |

Explicit `--wave` is distinct from inherited context: it is a deliberate typed
choice made *at the invocation surface*, so the CLI resolves it to
`CallerAuthority::Wave` directly and passes it in, rather than round-tripping it
through `LF_WAVE_ID` for the authority decision. (The env var may still be set
for prompt/journal context assembly; authority does not read it back.) Absent an
explicit flag, the ops funnel classifies from *inherited* managed context under
the fail-closed rule below.

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

with these rules (evaluated top-down; explicit `--wave` short-circuits to
`Wave` before any inherited-context arm):

1. `LF_PROJECT_SESSION_ID` present → `Project(id)` iff it matches the target's
   **live routing target**, `resolve_task_project_route(store, task).current` —
   **not** `TaskSession.project_session_id`. That field is historical
   provenance; W2-243 deliberately routes supervision to a *live successor*
   Project Session when the historical one is terminal. Comparing against the
   historical id would both (a) reject a legitimate successor's command
   (`current != historical` on the healthy successor path) and (b) accept a
   *terminal* predecessor's command. So: resolve the route, then require the
   incoming `LF_PROJECT_SESSION_ID == route.current`; else "cannot control"
   naming `route.current` as the live owner. (For a Project *self*-command via
   `project_resume`, the same rule holds with the Project's own route; the
   healthy live case is `current == historical`, `succeeded == false`.)
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

### Move 2 — a barred supervisor resume leaves no live command (structurally, not by timing)

The invariant: **a supervisor `Resume` that ends up barred leaves zero
`Persisted`/`Claimed`/`Uncertain` command rows — even if the PR phase changes
between the legality check and the launch.**

A naive "precheck the bar, then persist, then launch" is a TOCTOU and does not
hold the invariant. `queue_command` today creates the row (`ops/child.rs:734`),
appends `Persisted` (`795`), then `launch()` (`839`) re-reads
`active_task_pr` inside `supervisor_restart_bar`. A precheck that reads
`Working` can be followed by a launch that re-reads `Open` (the body just
finished publishing) and refuses — recreating the very inert command. Moving the
check earlier only *narrows* the window; it does not close it.

Close it structurally: **for a launch-driving `Resume`, no durable command is
committed unless the generation reservation succeeds.** Persistence becomes
*contingent on* the authoritative launch, not prior to it:

1. If the body is already active, behaviour is unchanged (the command persists
   for the live body to consume; no restart, no bar).
2. If the body is not active, the launch (`relaunch_inactive_process`, which
   evaluates the restart bar against a *single authoritative read* and reserves
   the generation) runs **first**. Only on a successful reservation is the
   `Resume` command created and linked to that live generation. A barred or lost
   reservation returns the bar error having written nothing.

Because persistence follows the one authoritative bar read, there is no window
in which a command exists without a live generation to own it — the raced
`Working→Open` case and the steady-state already-`Open` case exercise the *same*
path (launch bars ⇒ nothing persisted). The optional pre-`Open` fast refusal
(check the phase up front and return early) is a courtesy that also writes
nothing; correctness does not depend on it, so no wall-clock race needs
injecting.

A resumed generation that boots and finds its command not yet linked is not an
orphan: `Resume { message: None }` is "run the next turn," and a `message` lands
on the body's command poll immediately after reservation. The receipt still
resolves — the command id is minted with the reservation, in the same critical
section.

**Deterministic regression (no timing):** stand up an open-PR Task with a
finished body; issue a Wave (then Project) `Resume`; assert the refusal *and*
`SELECT count(*) FROM child_commands WHERE target_id=<ts>` is unchanged.
Sabotage proof that the contingency is load-bearing: restore the old
persist-then-launch order and the same test goes red with a stranded `Persisted`
row. This one regression covers the race because the fix collapses the raced and
steady-state paths into the same authoritative gate.

Scope: **Resume only.** `CiFix` deliberately persists-then-bars for incident
attribution (`ensure_child_ci_fix_command` dedups by incident identity, and
`relaunch_on_duplicate` keys on `Persisted`); leave that path untouched. Steer,
follow-up, and interrupt against a *live* body still persist a directive for the
body to consume — this contingency only guards the *restart* of a non-live body.
Operator resume (`ExplicitResume`) is unaffected: its bar is abandon-only, so an
open PR still reserves a generation and answers review.

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
| Does `LF_PROJECT_SESSION_ID` validate against the historical or the live Project Session? | `resolve_task_project_route` (`ops/project.rs:1254`) returns `historical` (provenance) and `current` (live routing target). A terminal historical routes to the latest live successor for the same Linear project (W2-243); the healthy live case is `current == historical`, `succeeded == false`. | Arm 1 validates the incoming `LF_PROJECT_SESSION_ID` against `route.current`, not `session.project_session_id`. Add a regression: (a) historical live → its command controls, successor id would be rejected as foreign; (b) historical terminal + live successor → the successor's command controls and the terminal predecessor's is rejected; (c) terminal + no successor → `resolve_task_project_route` already fails actionably. |
| Is the pre-persist bar a TOCTOU? | Yes — `queue_command` persists (`ops/child.rs:734/795`) before `launch()` re-reads the PR phase (`839`), so a `Working` precheck can precede an `Open` launch. | Make persistence **contingent on** a successful reservation (persist-after-launch) rather than prior to it. The raced and steady-state cases then share one authoritative gate; a single `count(*)`-unchanged regression + persist-then-launch sabotage covers both. See Move 2. |

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
- **Persistence contingent on reservation, for Resume only.** Not a reorder but
  a dependency: no durable `Resume` command exists unless a generation was
  reserved, so a barred/raced launch leaves zero orphan regardless of a PR
  phase change between check and launch. ci-fix's persist-first incident
  attribution is deliberately preserved.
- **Project authority follows the live route.** Validate against
  `resolve_task_project_route(...).current`, honouring W2-243's terminal→
  successor routing; the historical `project_session_id` is provenance only.
- **Environment is transport, not oracle.** Authority is resolved at the
  invocation boundary; env carries a body's stamped identity and consistency
  evidence, but no missing var can mint a different authority. Explicit `--wave`
  is a deliberate operator assertion, kept distinct from inherited context.
- **The bar addresses its real reader.** `open_pr_bar` only ever reaches a
  supervisor, so it names the reviewer/operator and drops the self-loop text.
- **No new verb.** The operator resume is the sanctioned typed recovery; a
  headless-with-no-operator review handoff is out of scope and filed as a
  question.

## Scope

- In scope: `CallerAuthority` type resolved at the invocation boundary
  (explicit `--wave` vs inherited context) + `from_ambient` funnel
  (fail-closed, Project validated against `route.current`); rewire
  `command_source`/`project_command_source` onto it; reservation-contingent
  `Resume` persistence in `queue_command` (TOCTOU-free); audience-correct
  `open_pr_bar` text; regressions for Wave/Project/operator resume against an
  open-PR Task, the `env -u LF_WAVE_ID` strip case, the ENG-19 stranded-body
  shape, and the terminal-historical / live-successor Project routing case.
- Out of scope: a new headless review-delivery verb; changing ci-fix's
  persist-then-bar ordering; the open-PR bar policy itself (W2-129 stands);
  W2-319 mutation, scratch deletion, global-binary promotion, reteam apply, or
  landing PR #1077.

## Done when

- `CallerAuthority` is resolved to a typed value **at the invocation boundary**
  (explicit `--wave` distinguished from inherited managed context) and passed
  into ops; the authority decision does not read `LF_WAVE_ID`/
  `LF_PROJECT_SESSION_ID` back deeper in the stack. Environment participates as
  transport and consistency evidence — never as an authority oracle that a
  missing var can flip.
- Wave, Project, and operator commands derive legal actions and refusals from
  the one `from_ambient` funnel. `Project` authority validates against
  `resolve_task_project_route(...).current`, not the historical
  `project_session_id`.
- A supervisor barred by the open-PR bar is told the reviewer/operator is the
  next owner; the message recommends no self-resume, and the refusal persists no
  command (verified: `child_commands` count unchanged after a barred Wave
  resume, **even under a PR phase change between legality check and launch** —
  persistence is contingent on a successful reservation).
- `env -u LF_WAVE_ID lf task resume <open-pr-task>` from inside a managed body
  refuses loudly and is **not** reclassified Operator.
- Tests cover Wave, Project, and operator resume against an open-PR Task,
  including the ENG-19 stranded-body shape and the terminal-historical /
  live-successor Project routing case, each sabotaging the production branch it
  names (restore persist-then-launch → a stranded `Persisted` command appears;
  revert the fail-closed arm → the strip case classifies Operator and launches;
  compare against `project_session_id` → the successor's command is wrongly
  rejected).
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
