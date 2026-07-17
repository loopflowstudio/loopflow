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

This PR is its own demo, and main has already performed half of it. This PR
carries this design doc, so its head fails `scratch-clear` exactly like #1055 and
#1060 did — and on 2026-07-17 main duly **armed a third wake against head
`904185190`**, waking a body to repair the design doc under review. That body
refused and reported the structure instead. The "before" is measured, not argued.

The "after": with this change, `lf ci` and `lf task status` still report the head
failing `scratch-clear` — the observation stays honest — and `lf task status`
shows **no ci-fix command queued and no body woken**. The failure is visible; the
repair is not attempted.

## Who asks "is this failure actionable?"

**Five** consumers, and every one of them computes the answer as
`ci.state == Failing`. That expression conflates *red* with *a body can fix
this*, so on a design-carrying PR — every Task PR pre-land — all five are wrong
in the same direction, for the same reason.

| # | Asker | What it does on a scratch-clear-only head | Owned by |
|---|-------|-------------------------------------------|----------|
| 1 | `current_ci_incident` → `wake_legal` (`ops/task.rs:2686`) | Mints the wake identity, arming a body | **PR 1** |
| 2 | `ci_fix_restart_bar` → `wake_legal` (`task/mod.rs:761`) | Permits an automated restart of a submitted Task | **PR 1** |
| 3 | `open_pr_model` (`actions.rs:256`) | Recommends `Resume` ("fix failing required checks"), **blocks `Review`** | PR 2 |
| 4 | `next_move` (`waves.rs:1805,1817`) | Routes ownership to `Ci`; labels a live body "fixing CI" | PR 2 |
| 5 | `decide_open_pr_status` (`ops/task.rs:2663`) | `failing && !head_advanced` → **Blocked**, "the Task body did not repair the head. Needs a new directive or human review" | PR 1 defuses in practice |

Consumers 3 and 4 are live on **this PR right now**. `lf task status W2-309`:

```
action: resume  (required checks failed: scratch-clear)
  blocked: review  (required checks failed)
```

Consumer 5 is the sting in the tail, and it inverts the cost. A ci-fix body that
does the *right* thing — refuses to delete the design doc — leaves the head
unmoved, so `decide_open_pr_status` blocks the Task and demands "a new directive
or human review". The honest refusal is punished; the only unpunished move is the
destructive one. That is the avoidable-human-step KR violated by the machinery
that exists to avoid human steps.

It defuses in practice once PR 1 lands: 5 only bites when a turn ends without
pushing, and with no wake armed, no turn runs that has nothing to push. (A
kickoff/iterate body pushes its design, so `head_advanced` is true and the arm
never fires.) So PR 1 is load-bearing for 5 without touching it.

**How I got this wrong the first time.** This design originally asserted
`wake_legal` "has exactly the two callers that matter". False — I grepped the
*callers of a function* when the question was *who asks this question*. One
`grep -rn "CiState::Failing"` found three more askers, one of which blocks the
Task. Same trap wave memory records twice: grep for the **shape**, not the symbol
you're standing on.

## Approach

One predicate, shared; each consumer keeps its own policy.

The class belongs to the *check*, so it is named once on `CiCheck` and every
asker can reuse it. PR 1 routes consumers 1 and 2 through it via
`CiObservation::wake_legal()` (`task/mod.rs:326`) — documented as *the* legality
question and the shared entry point for both. Today it asks only
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

`land_time_precondition` lives on `CiCheck`, not inside `wake_legal`, precisely
so consumers 3–5 can adopt it in PR 2 without re-deriving the class. PR 1 does
not change them; it makes their fix a one-line question rather than a second
opinion.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does the existing merge-gate classifier already expose "not actionable"? | No. `MergeGateReading::from_checks` (`ops/pr.rs:551`) drops the **required aggregate** from the *seed* by a structural test (required-vs-full check sets). "Land greens it" is a different axis and is invisible to that test — `scratch-clear` is a genuine non-required leaf, indistinguishable from `rust-test`. | The types do not expose the distinction. Establish one source of truth (`LAND_TIME_PRECONDITION_CHECK`) rather than bending the aggregate rule to cover a class it cannot see. |
| What does the measured failure actually look like through our read path? | With required `[tests-result: fail]` and full `[tests-result: fail, scratch-clear: fail, …pass]`, `failing_leaves` = `["scratch-clear"]` (aggregates are dropped when a non-required leaf also failed). So the observation carries exactly one check named `scratch-clear`. | The predicate sees the real shape. The lifecycle harness (`set_checks`) reproduces it byte-for-byte, so the regression runs the production read path, not a hand-built struct. |
| Can the classifier live next to `clear_scratch` (the code that actually resolves the check)? | No, without inverting layering: `task/mod.rs` imports `child_session`, `engine`, `id` — never `crate::ops`; `ops` depends on `task`. | Const lives on the model, with doc comments binding both directions (`clear_scratch` gains a one-line pointer). The bind is doc-level, so it is pinned by a test instead (below). |
| "A name list rots" — what stops the rot? | The workflow is in-repo. A unit test can read `.github/workflows/ci.yml` and assert a job named by the const exists. Renaming the job then turns the test red at the const, instead of silently re-arming wakes in production. | Include the pin test. It is the only mechanism that makes the literal self-detecting; without it the const is exactly the drifting name list the seed warned about. |
| Does refusing the wake strand the incident? | `mark_ci_incidents_green(&pr.id, …)` (`ops/task.rs:3129`) settles by PR, not by wake, and land's scratch clear moves the head green — so a recorded scratch-clear incident closes on its own. Nothing retries arming from recorded incidents: `runner.rs:2396` re-derives through `current_ci_incident`, which respects `wake_legal`. | Safe to keep recording the incident. No sweeper, no `blocked_at` bookkeeping. |
| Does the second `wake_legal` caller change behavior correctly? | `ci_fix_restart_bar` permits an automated restart of a submitted Task only when `wake_legal`. A scratch-clear-only head now stays barred by `open_pr_bar` — the same bar the plain supervisor restart already applies. | Correct by construction: the one automated restart path is closed for exactly the heads that warrant no repair. |
| Are those two the only askers? | **No** — the original claim was false. `grep -rn "CiState::Failing"` finds three more: `open_pr_model`, `next_move`, `decide_open_pr_status`. Each re-derives "actionable" from `state == Failing` independently. | Named in "Who asks this question"; PR 2 owns 3 and 4. The predicate is placed on `CiCheck` so they adopt rather than re-derive. |
| Does leaving 3–5 unfixed in PR 1 leave the KR unmet? | Partly. 5 defuses in practice (no wake → no empty turn → no block). 3 and 4 stay wrong: they are **read** surfaces, so they burn nothing themselves — but they invite a supervisor to Resume, which does. | PR 1 as directed; PR 2 immediately after. The serial split is the directive's ("smallest honest classification before CI wake creation"), not a deferral of the finding. |
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

- **In scope (PR 1, this PR):** `CiCheck::land_time_precondition` + the const on
  `task/mod.rs`; the `wake_legal` clause; a doc pointer on
  `ops::land::clear_scratch`; unit regressions on `wake_legal` and
  `ci_fix_restart_bar`; a lifecycle regression through the real read path proving
  both directions; the workflow-name pin.
- **Next serial PR (PR 2, `ci-fix-actionable-consumers`):** consumers 3 and 4 —
  `open_pr_model` stops recommending `Resume` and stops blocking `Review` on a
  head whose only failures are land-time preconditions; `next_move` stops routing
  ownership to `Ci`. Both adopt `land_time_precondition`; neither re-derives it.
  The reviewable question PR 2 raises, and PR 1 deliberately does not: **should a
  design-carrying PR be reviewable while `scratch-clear` is red?** Wave doctrine
  says yes ("state the approval condition as *every check green except
  scratch-clear*"), and today the action model says no. That is a product call
  worth making explicitly rather than smuggling into a wake fix.
- **Out of scope:** `scratch-clear` itself and all `scratch/` handling; the
  `tests-result` roll-up's `needs: scratch-clear` structure (a design-carrying PR
  is red by construction pre-land — structural, not a defect); the publish path's
  mislabelled M<B diagnostic (ae4775c7); `blocked_at` bookkeeping; any change to
  `merge_gate_state`'s aggregate/leaf rule; `decide_open_pr_status` (consumer 5),
  which PR 1 defuses in practice — reopen it only if a scratch-clear Blocked is
  measured after PR 1 lands.

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

Baseline (2026-07-17): **3 measured armings** on structurally-red heads — ENG-4
/#1055/`4041c795e`, W2-297/#1060/`58c47028c`, and W2-309/#1062/`904185190` (this
PR, i.e. the fix's own design armed the defect it removes) — each burning one
agent body on an impossible repair, whose only "successful" action would have
been destructive.

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

## Verification status — read before approving the implementation

**The local test run is blocked by host `syspolicyd`, before the test binary
reaches `main`. No local pass is claimed here.** Every `cargo test` invocation in
this worktree died on signal (exit 143/144) during or just after compile, without
producing a `test result:` line. That is the host security policy refusing the
freshly built executable — not a compiler error and not a failing assertion.
Nothing about the code has been proven locally, and nothing about it has been
disproven either.

**GitHub CI is therefore the verifier for this PR.** `rust-test` and `rust-lint`
run the same targets on a host without that policy, and they are the authority
that decides whether this lands: the merge queue answers to required checks
alone, so an implementation that does not compile or whose tests fail cannot
reach main regardless of what this section says.

What a reviewer should hold me to, given that:

* Judge the implementation on **`rust-test` + `rust-lint` green at the published
  head**, not on any claim in this doc.
* The three tests named under "Done when" are written and committed
  (`c4fa88eee`); whether they *pass* is CI's answer, not mine.
* The sabotage proof (revert the `wake_legal` clause → the scratch-clear tests go
  red, the real-leaf test stays green) has **not** been executed. It is a claim
  about the tests' construction, argued from their fixtures, not a measurement.
  If CI is green, the sabotage run is still owed before anyone treats the guard
  as proven — and I have not run it.

A wry note for the Task this design serves: a local verifier that cannot be run
is exactly the class of infrastructure friction the Developer Efficiency KRs
measure. It is not W2-309's defect and I am not widening scope to chase it, but
it is worth someone's filing.

## Review log

A reviewer's observation head can lag the current one by a revision, so this
table says which sha resolved what. If a finding below is quoted back as
outstanding, the head being read is older than the sha named here.

| Finding | Resolved at |
|---------|-------------|
| Design claimed `wake_legal` "has exactly the two callers that matter" — false; three more askers exist (`open_pr_model`, `next_move`, `decide_open_pr_status`) | this head — see "Who asks this question" |
| Measure baseline said 2 armings | this head — 3, incl. this PR's own `904185190` |
| Consumers 3/4 (Resume recommended, Review blocked on a scratch-clear head) unowned | this head — PR 2 `ci-fix-actionable-consumers`, scoped in "Scope" |
| Implementation landed in the PR (design-only through `bdcf2a9b`) | `c4fa88eee` — `land_time_precondition`, the `wake_legal` clause, the `clear_scratch` doc pointer, and all four tests |
| Local test evidence | **none, and none claimed** — see "Verification status"; host `syspolicyd` kills the test binary before `main`, so CI is the verifier |
