# A ci-fix wake arms on scratch-clear, a check no Task action can ever green

## Problem

`scratch-clear` asserts a **land-time precondition**: `scratch/` must hold
nothing but `.gitkeep`. That is false on every Task PR carrying its own design
doc — which is every Task PR during kickoff and iterate, by construction. The
check is correct, and `lf pr land` greens it as its first act (`clear_scratch`,
`ops/land.rs:451`, commits `lf land: clear scratch/`).

Loopflow nonetheless mints a `CiIncident` for that failure and arms a `CiFix`
wake against it. Measured twice on 2026-07-17, both on PRs whose only red was
structural:

* ENG-4 / PR #1055, head `4041c795e`
* W2-297 / PR #1060, head `58c47028c` — while W2-297 was landing 7e91c1b7's fix

Each arming spends a full agent body on a bounded repair turn that cannot
succeed. Worse, the body's only route to "green" is deleting the reviewed design
doc — destroying the artifact the reviewer reads, to green a check land greens
anyway. This is the avoidable-repair-step KR's exact subject: an agent run doing
work no human would ask for.

Measured shape on #1060 at `58c47028c`: `rust-test`, `rust-lint`, `swift-test`,
`e2e-smoke`, `python-test`, `website-test`, `loopflow-ui-test`,
`migration-check` all pass; `scratch-clear` fails; `tests-result` fails *solely*
as its roll-up (`ci.yml:172` gives it `needs: scratch-clear`).

## The demo

This PR is its own demo. It carries this design doc, so its head fails
`scratch-clear` exactly like #1055 and #1060 did. On main that head arms a
ci-fix wake; with this change, `lf ci` and `lf task status` still report the head
failing `scratch-clear` — the observation stays honest — and `lf task status`
shows **no ci-fix command queued and no body woken**. The failure is visible; the
repair is not attempted.

## Approach

One predicate, at the one place that already answers the legality question.

`CiObservation::wake_legal()` (`task/mod.rs:326`) is documented as *the* question
"does this reading make a ci-fix wake legal", and it has exactly the two callers
that matter: `current_ci_incident` (the single mint point for a wake's identity,
`ops/task.rs:2685`) and `ci_fix_restart_bar` (`task/mod.rs:761`, the one
automated path allowed to restart a submitted Task). Today it asks only
`state == Failing`. It gains one clause:

```rust
pub fn wake_legal(&self) -> bool {
    if self.state != CiState::Failing {
        return false;
    }
    // An unnamed failure is one we cannot classify, not one proven harmless:
    // keep waking. Only a set that is *entirely* land-time preconditions is
    // provably not a repair.
    self.failing_checks.is_empty()
        || self
            .failing_checks
            .iter()
            .any(|check| !check.land_time_precondition())
}
```

The class is named on the check, beside the model it classifies:

```rust
/// The required check `lf pr land` greens itself, by clearing `scratch/`.
///
/// Land-time preconditions are the one class of required check no repair turn
/// can act on: `scratch-clear` fails on every PR carrying its own design doc,
/// and `ops::land::clear_scratch` — not a code change — is what greens it. A
/// body woken to repair it could only delete the artifact the reviewer reads.
///
/// A name belongs here only when an `lf pr land` step is what resolves it. This
/// is not a catalogue of CI jobs and not a mute button for checks that are
/// merely hard to fix.
const LAND_TIME_PRECONDITION_CHECK: &str = "scratch-clear";
```

Placement follows the existing seam rather than adding one: the observation stays
honest (it still records `scratch-clear` as failing, so status and `lf ci` still
show the red head), and only the *wake* is refused. Filtering earlier — inside
`merge_gate_state` — would have deleted the failure from the observation and left
status reporting `Failing` with nothing named.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does the existing merge-gate classifier already expose "not actionable"? | No. `MergeGateReading::from_checks` (`ops/pr.rs:551`) drops the **required aggregate** from the *seed* by a structural test (required-vs-full check sets). "Land greens it" is a different axis and is invisible to that test — `scratch-clear` is a genuine non-required leaf, indistinguishable from `rust-test`. | The types do not expose the distinction. Establish one source of truth (`LAND_TIME_PRECONDITION_CHECK`) rather than bending the aggregate rule to cover a class it cannot see. |
| What does the measured failure actually look like through our read path? | With required `[tests-result: fail]` and full `[tests-result: fail, scratch-clear: fail, …pass]`, `failing_leaves` = `["scratch-clear"]` (aggregates are dropped when a non-required leaf also failed). So the observation carries exactly one check named `scratch-clear`. | The predicate sees the real shape. The lifecycle harness (`set_checks`) reproduces it byte-for-byte, so the regression runs the production read path, not a hand-built struct. |
| Can the classifier live next to `clear_scratch` (the code that actually resolves the check)? | No, without inverting layering: `task/mod.rs` imports `child_session`, `engine`, `id` — never `crate::ops`; `ops` depends on `task`. | Const lives on the model, with doc comments binding both directions (`clear_scratch` gains a one-line pointer). The bind is doc-level, so it is pinned by a test instead (below). |
| "A name list rots" — what stops the rot? | The workflow is in-repo. A unit test can read `.github/workflows/ci.yml` and assert a job named by the const exists. Renaming the job then turns the test red at the const, instead of silently re-arming wakes in production. | Include the pin test. It is the only mechanism that makes the literal self-detecting; without it the const is exactly the drifting name list the seed warned about. |
| Does refusing the wake strand the incident? | `mark_ci_incidents_green(&pr.id, …)` (`ops/task.rs:3129`) settles by PR, not by wake, and land's scratch clear moves the head green — so a recorded scratch-clear incident closes on its own. Nothing retries arming from recorded incidents: `runner.rs:2396` re-derives through `current_ci_incident`, which respects `wake_legal`. | Safe to keep recording the incident. No sweeper, no `blocked_at` bookkeeping. |
| Does the second `wake_legal` caller change behavior correctly? | `ci_fix_restart_bar` permits an automated restart of a submitted Task only when `wake_legal`. A scratch-clear-only head now stays barred by `open_pr_bar` — the same bar the plain supervisor restart already applies. | Correct by construction: the one automated restart path is closed for exactly the heads that warrant no repair. |
| Does a failing-but-unnamed reading exist, and what should it do? | `merge_gate_state` falls back to `gate.failing_checks` when the full read is empty, so `failing_checks` is normally non-empty on a real gate failure; an empty set means we could not name the failure. | Fail toward waking. `failing_checks.is_empty()` stays legal — conservatism has a direction, and a filter that suppresses unknown failures is a mute button. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Filter land-time preconditions out of `failing_leaves` in `MergeGateReading::from_checks` | Reuses the existing "non-actionable" category the seed suggested; one place | Deletes the failure from the **observation**, not just the wake. Status and `lf ci` would report the head `Failing` with zero named checks — trading a wasted body for an unexplained red. The failure is real; only the repair is not. |
| Refuse at `queue_ci_fix_command` (the enqueue call site) | Smallest possible diff | `wake_legal` is documented as the single legality question and has two callers. Answering it in one caller leaves `ci_fix_restart_bar` believing a scratch-clear head warrants an automated restart — a second, divergent legality answer. |
| Record the incident as `blocked_at`/`blocked_reason = "land-time precondition"` | Explicit evidence trail | That machinery means "a wake fired and infra stopped it". Nothing fired. It would add state to say nothing happened, against the directive's minimal wire/state shape. |
| Make `scratch-clear` not required, or green it earlier | Removes the arming as a side effect | Explicitly out of bounds. The check is correct; arming a repair against it is what is wrong. |
| Detect the class dynamically (parse the job's script; ask GitHub what land would change) | No literal at all | Nothing in a check's GitHub projection says "our own land step resolves this". The fact lives in loopflow's land path, so naming it there *is* reading the authority. |

## Key decisions

**The observation stays honest; only legality moves.** A ci-fix wake is refused,
the red head is still reported. Anyone reading `lf ci` sees `scratch-clear`
failing on that head. This is the whole difference between a classification and a
mute button.

**One name, not a list, and a test that catches its rot.** There is exactly one
land step that greens a check. A `const` with a stated admission rule ("only when
an `lf pr land` step resolves it") plus a workflow-pin test is stronger than a
`&[&str]` catalogue nobody prunes.

**Unknown failures still wake.** The suppression triggers only when the failure
set is non-empty and *every* member is a land-time precondition. `[scratch-clear,
rust-test]` arms; `[]` arms.

**The incident record survives.** Refusing the wake is not the same as denying the
failure happened. `ci_incidents` keeps its row (recorded through `ci_incident`,
which does not consult `wake_legal`); it simply never gains a
`trigger_command_id`, which is the truth.

## Scope

- **In scope:** `CiCheck::land_time_precondition` + the const on `task/mod.rs`;
  the `wake_legal` clause; a doc pointer on `ops::land::clear_scratch`; unit
  regressions on `wake_legal` and `ci_fix_restart_bar`; a lifecycle regression
  through the real read path proving both directions; the workflow-name pin.
- **Out of scope:** `scratch-clear` itself and all `scratch/` handling; the
  `tests-result` roll-up's `needs: scratch-clear` structure (a design-carrying PR
  is red by construction pre-land — structural, not a defect); the publish path's
  mislabelled M<B diagnostic (ae4775c7); `blocked_at` bookkeeping; any change to
  `merge_gate_state`'s aggregate/leaf rule.

## Done when

```bash
cargo test -p loopflow --lib task::                       # wake_legal + restart bar
cargo test -p loopflow --lib ci_fix                       # lifecycle, both directions
cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check
```

Specifically:

1. `a_scratch_clear_only_head_arms_no_ci_fix_wake` — the lifecycle harness sets
   required `[tests-result: fail]`, full `[tests-result: fail, scratch-clear:
   fail, rust-test: pass]`; `observe()` returns `None`, `ci_fix_commands()` is
   empty, and `observation()` still names `scratch-clear` as failing (the read
   stays honest, and `scratch/` is untouched).
2. `a_failing_leaf_still_arms_exactly_one_wake` — the existing `checks_failing()`
   shape still mints exactly one attributable command. Both, or the fix is a mute
   button.
3. `wake_legal`: `["scratch-clear"]` → false; `["scratch-clear", "rust-test"]` →
   true; `["rust-test"]` → true; `[]` → true.
4. `ci_fix_restart_bar` bars an open PR whose only failure is `scratch-clear`.
5. The workflow pin: `.github/workflows/ci.yml` declares a job named by
   `LAND_TIME_PRECONDITION_CHECK`.

**Sabotage proof:** revert the `wake_legal` clause and (1), (3), (4) go red while
(2) stays green. That (2) passes with the bug fully present is exactly why this
survived two live occurrences — a test using a head with a real failure proves
nothing here.

## Measure

Baseline (2026-07-17): **2 measured armings** on structurally-red heads — ENG-4
/#1055/`4041c795e` and W2-297/#1060/`58c47028c` — each burning one agent body on
an impossible repair, whose only "successful" action would have been destructive.

After: zero `ChildCommandKind::CiFix` commands whose incident `failure_set` is
exactly `["scratch-clear"]`.

```sql
-- expect 0 rows for heads after this lands
SELECT identity, failure_set, trigger_command_id
FROM ci_incidents
WHERE failure_set = '["scratch-clear"]' AND trigger_command_id IS NOT NULL;
```

Incidents with `failure_set = ["scratch-clear"]` and a NULL `trigger_command_id`
are the healthy shape, not a gap: the head was red, and no repair was warranted.
