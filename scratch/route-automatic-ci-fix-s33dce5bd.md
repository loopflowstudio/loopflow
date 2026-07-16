# Route automatic ci-fix wakes through the durable Task command ledger

ENG-20 · Developer Efficiency · wave `infrastructure`

## Problem

A human direction to a Task is durable, typed, deduplicated, and auditable: it
lands in `child_commands`, moves through `Persisted → Claimed → Delivering →
Accepted`, emits `CommandChanged` events, and survives a restart. An automatic
CI-fix wake is none of those things.

Today the Project runner reads a boolean off a mutable JSON blob and calls the
launcher directly:

```rust
// project_session/runner.rs:835-849
} else if !task.status.is_process_active() {
    let warranted = observed.as_ref().and_then(|pr| pr.fresh_ci())
        .is_some_and(crate::task::CiObservation::wake_warranted);
    if warranted {
        crate::ops::task::wake_task_ci_fix(store, task).await?;   // :845 — the bypass
    }
}
```

Three consequences, all real:

1. **Dedup lives in the wrong place and is committed too late.** The key is
   `CiObservation::woken_failure_set` (`task/mod.rs:307`) — a `serde(default)`
   field on the *current* observation row. It is re-derived on every reconcile,
   carried forward by a hand-written equality check (`ops/task.rs:2554-2564`),
   and only committed at body birth in `arm_ci_fix_wake`
   (`task/runner.rs:1955`). Between the runner's `warranted` read and the
   child's arming write there is an open window with nothing durable in it.
2. **The wake is invisible.** No command, no `CommandChanged` event, no
   receipt. `CiIncident.trigger_command_id` (`task/mod.rs:355`) — the field
   built to answer "which command woke this body?" — is hardcoded `None` on the
   only path that writes incidents (`ops/task.rs:2568-2605`). The evidence
   ledger cannot name its own trigger.
3. **The same question is asked three times and answered once.** The project
   runner checks `wake_warranted` (`runner.rs:840`), `ci_fix_restart_bar`
   checks it again (`task/mod.rs:759`), `arm_ci_fix_wake` checks it a third
   time (`task/runner.rs:1963`) — only the third persists. Legality and dedup
   are conflated in all three.

And the body re-derives its own seed from `pr.fresh_ci()` at boot
(`ci_fix_seed`, `task/runner.rs:1982`), so if the observation row moves between
the decision to wake and the body reading it, the body repairs a different
failure than the one that woke it. Nothing carries the failure set across.

**Who benefits:** the Developer Efficiency KR *"No Task strands on a dead body:
across one full week of real runs, zero Sessions sit in failed awaiting a manual
resume, and zero durable commands are left orphaned 'uncertain' against a dead
generation."* An automatic wake that isn't a durable command can't be counted by
that KR at all — it is invisible to the very ledger the KR measures. This work
brings the automatic path inside the boundary the KR is written against.

**Why now:** #1021 landed the `CiIncident` ledger with `trigger_command_id`
already in the schema, and #1001 landed the `Blocked` classification. The
evidence side is built and deliberately inert — the migration header says
*"without becoming another wake queue."* The command it was waiting for is this
one.

## The demo

Fail a required check on an open Task PR whose body is asleep. Then:

```
$ lf ci
IDENTITY                                  PR    HEAD     FAILURE SET     TRIGGER              RESPONDED
github:ci:loopflow:1042:a1b2c3d:9f0e…     1042  a1b2c3d  tests-result    cc_7f3a…             12s ago

$ lf task status ENG-NN --json | jq .events
… { "kind": "ci_fix_armed", "command_id": "cc_7f3a…", "pr_number": 1042,
    "head_sha": "a1b2c3d", "failing_checks": ["tests-result"] }
… { "kind": "command_changed", "command_id": "cc_7f3a…", "state": "accepted" }
```

Then re-run the observation twice and restart the Project runner mid-flight —
`lf ci` still shows exactly one trigger command and one woken body. The wake is
now something you can name, look up, and count.

## Approach

**One typed command variant on the existing ledger, keyed on the identity the
incident ledger already mints.**

### 1. The command

Add one variant to `ChildCommandKind` (`child_session.rs:363`). It rides
`kind_json` — **no migration**:

```rust
/// An automatic wake for a failed required-check head on this Task's open PR.
/// Unlike every other variant this is not input to a live body: it is a launch
/// intent whose payload becomes the born body's seed. `incident_identity` is the
/// dedup key, minted by and shared with `CiIncident` — one command per
/// (repo, PR, failed head, failure set), forever.
CiFix {
    incident_identity: String,
    repo: String,
    pr_number: u32,
    head_sha: String,
    failing_checks: Vec<CiCheck>,
},
```

`ChildCommand::new` gives it `effect: None` — its effect is a launch, not a
delivery into an existing turn.

It carries `CiCheck` (name + log URL), not bare names, because the seed needs the
URLs and re-reading them from the observation at boot is exactly the drift this
change removes.

### 2. Identity is the dedup key, and it already exists

`ci_incident()` (`ops/task.rs:2568`) already mints:

```
github:ci:{repo}:{number}:{head_sha}:{sha256(failure_set)}
```

That string is precisely "one failed head, one failure set, one repair attempt."
The command reuses it verbatim. Dedup becomes `ensure_child_ci_fix_command` —
the same shape as the existing `ensure_child_decision_command`
(`sqlite/child_sessions.rs:1045`), which scans the session's commands for a
matching `decision_id` and returns `(existing, false)` instead of inserting:

```rust
// sqlite/child_sessions.rs — scan by incident_identity inside the tx
pub fn ensure_child_ci_fix_command(&self, command: &ChildCommand)
    -> Result<(ChildCommand, bool)>
```

Per-session command counts are small and `idx_child_commands_pending` already
covers `(target_kind, session_id, …)`; a scan is the right cost here, and it is
the cost the decision path already pays.

### 3. Delete `woken_failure_set` entirely

This is the point of the change, not a side effect. The JSON marker, its
carry-forward block, `wake_warranted()`, and `mark_woken()` all go:

| Deleted | Replaced by |
|---|---|
| `CiObservation::woken_failure_set` (`task/mod.rs:300-307`) | the existence of a `CiFix` command with that identity |
| `CiObservation::wake_warranted()` (`:329`) | `state == CiState::Failing` (legality) + `ensure_` dedup (identity) |
| `CiObservation::mark_woken()` (`:336`) | `Persisted` row, written before any body starts |
| carry-forward equality block (`ops/task.rs:2554-2564`) | nothing — a DB row needs no carrying |
| `arm_task_pr_ci_fix_for_lease` marker write (`sqlite/child_sessions.rs:795`) | keeps only its `responded_at` stamp |

`fresh_ci()` **stays** — head-move staleness is genuine and orthogonal to dedup.

Keeping both would be the "second state model" the directive forbids. The
command ledger is strictly better than the marker on every axis: durable before
the body exists, survives observation churn, addressable by id, restart-safe by
construction.

### 4. Split legality from dedup

`ci_fix_restart_bar` (`task/mod.rs:750`) currently asks a dedup question. It
should ask only a legality question:

```rust
PrPhase::Open if !pr.fresh_ci().is_some_and(|o| o.state == CiState::Failing) => {
    return Some(self.open_pr_bar(pr));
}
```

**Bar = is this launch legal. Ledger = has this wake already fired.** Two
questions, two owners, each asked once.

### 5. Enqueue replaces launch

`project_session/runner.rs:835-849` becomes an enqueue. The `!is_process_active()`
guard **stays** (see Key decisions):

```rust
} else if !task.status.is_process_active() {
    if let Some(incident) = observed.as_ref().and_then(pending_ci_fix_wake) {
        crate::ops::task::queue_ci_fix_command(store, task, incident).await?;
    }
}
```

`wake_task_ci_fix` (`ops/child.rs:508`) and its `ops::task` re-export
(`ops/task.rs:2498`) are deleted. `LaunchIntent::CiFix` survives — it is now
reached from `queue_command`'s intent match rather than a bespoke entry point:

```rust
let launch_intent = match (&kind, &source) {
    (ChildCommandKind::Resume { .. }, ChildCommandSource::Human) => LaunchIntent::ExplicitResume,
    (ChildCommandKind::CiFix { .. }, _) => LaunchIntent::CiFix,
    _ => LaunchIntent::Supervisor,
};
```

Everything after that is the ledger's existing behaviour, unchanged:
`queue_command`'s tail (`ops/child.rs:740`) launches when inactive; the `!created`
branch (`:681`) relaunches a non-terminal duplicate and no-ops a terminal one —
which is exactly the restart semantics we want, for free.

Source is `ChildCommandSource::System`. Like `Linear`, it must not be `Human` —
it must never carry the operator-resume affordance.

`wait_for_resolution = false` for `CiFix`, joining `FollowUp`. The supervision
pass must not block 2s on a body's boot.

### 6. Arming reads the command, not the observation

`arm_ci_fix_wake` (`task/runner.rs:1955`) stops re-deriving warrant and instead
consumes what was claimed:

```rust
// task/runner.rs — before the harness exists
async fn arm_ci_fix_wake(...) -> Result<Option<CiFixWake>> {
    // claim_commands has already run; find the claimed CiFix, if any
    // begin_delivery (Claimed → Delivering)
    // build the seed from the command payload
    // accept_command (Delivering → Accepted)
    // stamp CiIncident.responded_at by incident_identity
    // emit TaskEventKind::CiFixArmed
}
```

Ordering note: `reconcile_stale_deliveries` runs at `task/runner.rs:115`, before
this. That ordering is load-bearing and stays — a `Delivering` CiFix from a dead
generation goes `Uncertain` before the new generation claims, and
`plan_body_recovery` filters on `claimed_by_generation == Some(generation)`
(`child_session.rs:760`), so a predecessor's uncertainty does not poison this
one.

`ci_fix_seed` (`task/runner.rs:1982`) takes the command payload instead of
`pr.fresh_ci()`. **The body now repairs the failure that woke it**, not whatever
the observation row says at boot. This is a correctness fix riding along, and
worth stating: the seed and the dedup key now describe the same failure.

The `Delivering` window is a few statements wide and contains no provider call
— narrower than any existing command's.

### 7. Live-body race → `Superseded`

Add a `CiFix` arm to `absorb_commands` (`child_control.rs:293`). If a live body
ever claims one (the enqueue guard raced), it records `Superseded` with no
harness delivery: a body already working this PR supersedes the wake for it.
Existing state, existing event, no new concept.

### 8. Evidence closes the loop

- `queue_ci_fix_command` stamps `CiIncident.trigger_command_id` after
  `ensure_` reports `created: true`. New store method
  `mark_ci_incident_triggered(identity, command_id)` beside the existing
  `mark_ci_incident_responded`. No FK on the column (`0.11.024:18`), so no
  ordering constraint — but we write it after creation regardless.
- `arm_ci_fix_wake` stamps `responded_at` — unchanged milestone, now emitted
  from the command path.
- One new event, `TaskEventKind::CiFixArmed { command_id, pr_number, head_sha,
  failing_checks }`, project-observable. This is what makes *"which failure set
  woke which body"* answerable from the event stream rather than by joining two
  tables in your head.
- `lf ci` (`lf/commands/ci.rs`) renders `trigger_command_id`.

**`CiFix` never mints a `ChildDirective`.** A directive would bump
`current_directive_version`, and `has_pending_directive` (`ops/task.rs:3180`)
would then block Task completion at `task_completion_gate` (`:3797`) until the
ci-fix body acknowledged a direction no human ever gave. A wake is a command, not
a direction. This is the sharpest trap in the change.

### 9. No migration

`kind_json` is JSON; `trigger_command_id` already exists in `0.11.024`. The
latest ordinal stays `0.11.024_ci_incidents.sql`. This matters beyond
convenience — wave memory records that *migration ordinals race until merge*: a
sibling can merge your ordinal while the PR sits, and auto-merged migration
tests lie. Landing this with zero migrations sidesteps that class entirely.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Does a new `ChildCommandKind` variant need a migration? | No. `child_commands.kind_json TEXT NOT NULL` (`0.10.001_initial.sql:294`) holds the serde-tagged enum; `state` has no CHECK either. | Zero migrations. Avoids the ordinal-race class in wave memory. |
| Is there already an idempotent-insert precedent, or must I invent one? | Yes — `ensure_child_decision_command` (`sqlite/child_sessions.rs:1045`) scans for a matching `decision_id` and returns `(existing, false)`. | `ensure_child_ci_fix_command` copies it verbatim, keyed on `incident_identity`. No new pattern. |
| Does `CiIncident` already have somewhere to record the trigger? | Yes — `trigger_command_id TEXT` (`0.11.024:18`), no FK, COALESCEd on the upsert (`sqlite/ci_incidents.rs:86`), and hardcoded `None` by its only writer (`ops/task.rs:2568-2605`). | The field was built for this and left inert. Just fill it. |
| Is the incident identity actually a sound dedup key for a wake? | `github:ci:{repo}:{number}:{head_sha}:{sha256(failure_set)}` — the same tuple `woken_failure_set` approximates, but minted once and stored under a PRIMARY KEY. | Reuse it. One identity, two rows (`ci_incidents` evidence, `child_commands` wake). |
| Can `queue_command` launch an inactive Session, or does CiFix need its own entry point? | It already does — `ops/child.rs:740` launches when `!is_process_active()`, and `:681` relaunches a non-terminal duplicate. `(Resume, Human) → ExplicitResume` proves the intent-per-kind pattern. | `wake_task_ci_fix` deletes entirely. Add one arm to the intent match. |
| Does the existing test suite already assert the behaviours the done-when names? | Yes — `ci_fix_lifecycle_tests.rs:580` covers duplicate arm (`:628`) and restart-between-observation-and-settlement (`:633-639`); `:683` covers a changed failing set. | Preserve every assertion; re-point them at the ledger. Test names survive the refactor — that's the proof the contract didn't move. |
| Is the project-runner → wake path covered end-to-end today? | **No.** `project_session/runner.rs:844` has no test. Explore confirmed zero coverage in `tests/`. | The seam being replaced is untested. New coverage is required, not optional. |
| Would a `CiFix` command block Task completion? | It would if it minted a directive — `has_pending_directive` (`ops/task.rs:3180`) gates `task_completion_gate` (`:3797`) on `current > incorporated`. | `CiFix` mints no directive. Called out explicitly; asserted by test. |
| What is ENG-19, and does it collide? | `8ffcd0c6` — *"Settle one bounded ci-fix turn without entering the generic Task gate."* It owns the turn's **exit**; this owns the **entry**. | Complementary. Seam named below. |
| Can the `Delivering → Uncertain` path strand a ci-fix wake? | `reconcile_stale_deliveries` runs at `task/runner.rs:115` *before* claim; `plan_body_recovery` filters on the current generation (`child_session.rs:760`). The window here holds no provider call. | Keep the existing ordering; the window is narrower than any live-input command's. No special casing. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Keep `woken_failure_set`; add the command on top | Smallest diff; no behaviour moves | Two dedup models for one question. The directive forbids a second state model, and it's right: the marker and the ledger would drift and the marker would win the race it's already winning. |
| A dedicated `ci_fix_wakes` table with its own states | Purpose-built schema, clean queries | Literally the parallel queue the directive forbids and `0.11.024`'s header warns against (*"without becoming another wake queue"*). Rebuilds claim/dedup/restart that `child_control.rs` already got right. |
| Reuse `FollowUp { text }` with a rendered seed | Zero new variants | Loses the typed payload the done-when requires (canonical PR number, head SHA, failed checks). Dedup would key on prose. `FollowUp` also means *deliver to a live body*, which is the opposite of a wake. |
| Drive the wake off `ci_incidents` rows (open incident with no `responded_at`) | No new command variant | Turns the evidence ledger into the wake queue — the one thing its schema comment forbids. Also gives up acceptance, `Uncertain`, receipts, and events. |
| Make `CiFix` mint a `ChildDirective` for symmetry with steer | Uniform "direction" model, incorporation for free | Bumps `current_directive_version` → `has_pending_directive` blocks completion on a direction no human gave. Symmetry that breaks the gate. |
| Enqueue unconditionally, let a live body supersede | Ledger becomes the complete record of every failed head observed | If the live body dies without fixing it, that failure set is permanently `Superseded` and never wakes again — a stranding hole, against the KR. Keep the `!is_process_active()` guard; a dead body's next reconcile finds no command and enqueues cleanly. |

## Key decisions

**The incident identity is the wake identity.** One string, minted once by
`ci_incident()`, keying both the evidence row and the command. Two ledgers, one
key, joinable by a human at 2 a.m. with `lf ci` and one grep.

**`woken_failure_set` dies rather than coexists.** Net deletion: a field, two
methods, a carry-forward block, and one of three redundant warrant checks. The
system gets more true — which is the test the review ritual asks for.

**Bar answers legality, ledger answers dedup.** Today `ci_fix_restart_bar` asks
both and `wake_warranted` is consulted three times at three layers. After: the
bar asks "is an open PR failing on its current head" and the ledger asks "did we
already wake for this identity." Each question has one owner.

**`CiFix` is a launch intent, not an input.** Every other variant delivers into a
live body via `harness.send_input`. `CiFix` targets a sleeping Task and its
payload becomes the born body's seed. That's why it is armed before the harness
exists (`task/runner.rs:122`, ahead of `:141`) and why a live body supersedes it.

**The seed comes from the command.** `ci_fix_seed` stops reading
`pr.fresh_ci()`. The body repairs the failure that woke it. Today those can
differ; that's a latent bug this closes.

**`ChildCommandSource::System`, never `Human`.** Same reasoning that made
`Linear` distinct (`child_session.rs:1092`): an automatic wake must not inherit
the operator-resume affordance.

**Keep the `!is_process_active()` enqueue guard.** Enqueueing against a live body
would permanently supersede that failure set even if the body then dies. The
guard keeps the ledger honest about wakes rather than about observations.

**Lifecycle is untouched.** No `CiFix` phase, no `ci_fix` lifecycle-plan fields,
no origin-phase persistence. `Kickoff/Iterate/Gate` stand. The command launches
one `ci-fix` flow (`QueuedInvocation::load(&session.worktree, "ci-fix")`,
`task/runner.rs:124`) and the Task parks until new PR evidence arrives.

## The ENG-19 seam

ENG-19 (*"Settle one bounded ci-fix turn without entering the generic Task
gate"*) owns the **exit**; ENG-20 owns the **entry**. The seam is the command's
terminal state plus `CiIncident.responded_at`:

- ENG-20 guarantees: when a ci-fix body is born, exactly one `CiFix` command is
  `Accepted` against its generation, its payload named the failure, and
  `responded_at` is stamped on the matching incident.
- ENG-19 consumes: at turn end, `arm_ci_fix_wake` returning `Some(CiFixWake)` is
  the durable, restart-surviving signal that *this generation exists to service
  one bounded ci-fix turn* — which is exactly the predicate ENG-19 needs to park
  instead of entering the generic Gate. Today that signal is a `bool` local that
  dies with the process.

ENG-19 stays a narrow follow-up because this PR makes its predicate durable and
queryable without touching lifecycle. **This PR must not change settlement
behaviour** — the turn ends exactly as it does today. Landing settlement here
would merge two bets.

## Scope

**In scope**

- `ChildCommandKind::CiFix` + serde round-trip.
- `ensure_child_ci_fix_command` (store + sqlite), modelled on
  `ensure_child_decision_command`.
- `mark_ci_incident_triggered`; `ci_incident()` and `queue_ci_fix_command`
  populate `trigger_command_id`.
- `queue_command` intent arm; `wait_for_resolution = false` for `CiFix`.
- `absorb_commands` `CiFix` → `Superseded` race arm.
- Delete `wake_task_ci_fix` (both `ops/child.rs:508` and the `ops/task.rs:2498`
  re-export). Rewrite `project_session/runner.rs:835-849` to enqueue.
- Delete `woken_failure_set`, `wake_warranted`, `mark_woken`, the carry-forward
  block, and the marker write inside `arm_task_pr_ci_fix_for_lease`.
- `ci_fix_restart_bar` → legality only.
- `arm_ci_fix_wake` reads the claimed command; `ci_fix_seed` takes the payload.
- `TaskEventKind::CiFixArmed`, project-observable.
- `lf ci` renders the trigger command id.
- Tests (below).

**Out of scope**

- Settlement / parking behaviour at turn end — **ENG-19**.
- Any lifecycle phase, `ci_fix` plan field, or origin-phase persistence.
- Webhook delivery (`webhook_received_at` / `provider_completed_at` stay
  `None` on the poll path — a separate bet).
- A pending-command list on `TaskSessionSnapshot`.
- Any migration.

## Done when

`cargo test -p loopflow` green, with these tests:

**Preserved, re-pointed at the ledger** (`task/runner/ci_fix_lifecycle_tests.rs`
— names survive; a green suite under the same names is the proof the contract
didn't move):

- `a_failed_head_wakes_exactly_one_ci_fix_body_and_rearms_until_green` (`:580`)
  — now asserts one `CiFix` command per identity, `Accepted` after arm, and
  `trigger_command_id` linking incident → command.
- `a_changed_failing_set_on_the_same_head_rearms` (`:683`) — new identity, new
  command.
- `a_gh_outage_degrades_the_read_without_inventing_a_reading` (`:713`) — no
  incident, no command.
- `fresh_ci_ignores_a_reading_for_a_past_head` (`task/mod.rs:1486`).
- `ci_fix_restart_bar_permits_only_a_warranted_open_pr_wake`
  (`task/mod.rs:1561`) — retitled to the legality-only contract.
- `ci_fix_wake_refuses_an_open_pr_without_a_warranted_failure`
  (`ops/child.rs:1289`).

**New:**

- `duplicate_observation_enqueues_one_ci_fix_command` — reconcile ×3 on one
  failed head ⇒ exactly one command row, one incident, one `trigger_command_id`.
- `a_restart_before_the_body_boots_delivers_the_same_command` — enqueue, drop
  the process, reconcile again, boot ⇒ the *same* `command_id` is claimed and
  accepted; no second row, no second body.
- `a_delayed_ci_fix_command_survives_until_a_body_claims_it` — command sits
  `Persisted` across N supervision passes with no body, then boots and accepts.
- `a_ci_fix_command_seeds_the_body_from_its_own_payload` — mutate
  `pr.ci_observation` between enqueue and boot; the seed still names the
  command's `head_sha` and `failing_checks`.
- `a_live_body_supersedes_a_raced_ci_fix_command` — no second body, no harness
  delivery.
- `a_ci_fix_command_mints_no_directive` — `current_directive_version` unchanged;
  `task_completion_gate` not blocked.
- `a_ci_fix_command_round_trips_its_payload` — serde on `kind_json`.
- **Project-runner coverage** (`project_session/`): the `:835-849` seam
  enqueues rather than launches — the path Explore confirmed has zero tests
  today.

**Observable outcome** — the demo above: `lf ci` names the trigger command, and
`lf task status --json` shows `ci_fix_armed` + `command_changed{accepted}` for
the same `command_id`.

## Measure

`CiIncident` is already the instrument; #1021 built it for the KR
*"across one full week of real runs … zero durable commands are left orphaned
'uncertain' against a dead generation."*

Baseline, before:

```bash
lf ci --json | jq '[.[] | select(.trigger_command_id == null)] | length'   # expect: all of them
```

Every incident today has a null trigger — the wake is unattributable, so the KR
cannot see the automatic path at all.

After: every incident with `responded_at != null` carries a
`trigger_command_id`, and that command is `Accepted` against the generation that
serviced it. Weekly check:

```bash
# wakes with no attributable command — target 0
lf ci --json | jq '[.[] | select(.responded_at != null and .trigger_command_id == null)] | length'
# incidents whose command went Uncertain against a dead generation — target 0
lf ci --json | jq '[.[] | select(.trigger_command_id != null)] | length'   # cross-ref child_commands.state
```

"Better" = the automatic wake path becomes countable by the same query that
already counts the human path, and the orphaned-`uncertain` count stays 0.
