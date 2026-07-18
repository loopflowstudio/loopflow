# Recommend and route only CI failures a body can repair (ENG-33)

## Problem

Five consumers ask "is this failing head actionable by a Task body?" and every
one computes it as `ci.state == Failing`, conflating "red" with "a body can fix
this". On a design-carrying PR — every Task PR pre-land, where `scratch-clear`
is red by construction — all five are wrong the same way. #1062 fixed the two
wake-arming askers. This task owns two more:

- **open_pr_model** (`task/actions.rs`) recommends Resume ("fix failing required
  checks") and blocks Review on any failing head.
- **next_move_for_task** (`lf/commands/waves.rs`) routes `owner=Ci` and labels a
  live body "fixing CI" on any failing head.

Live evidence: `lf task status W2-309` on a head whose only red was
`scratch-clear` printed `action: resume` and `blocked: review`, and the Waves
supervision surface read `owner=Ci` when the true owner was the reviewer.

## Approach

Adopt the predicate already on `main`, don't re-derive it. `CiCheck::land_time_precondition()`
(task/mod.rs) classifies the one check `lf pr land` greens itself. Add one
reader on `CiObservation`:

```rust
pub fn only_land_time_preconditions(&self) -> bool  // Failing && !empty && all preconditions
```

the exact dual of `wake_legal()` within the failing state. Route both consumers
through it:

- open_pr_model: a head red only on preconditions returns the *reviewable* model
  (recommend Review, block Resume) — the same model a passing head returns,
  factored into `open_pr_reviewable(reason)`.
- next_move_for_task: such a head returns `owner=Review`, before the `fixing`
  check, so a live body on it is the reviewer's turn, not "fixing CI".

Product call made in the open: **a design-carrying PR is reviewable while
`scratch-clear` is red.** Wave doctrine (approval condition = every check green
except scratch-clear) says yes; the tests-result roll-up is red by construction
pre-land and only greens at land.

## Scope

- In: the two consumers + shared predicate + sabotage-sensitive tests.
- Out: `decide_open_pr_status` (#5, defused by #1062), scratch/scratch-clear/
  tests-result handling, CI topology.

## Done when

- Head red only on `scratch-clear`: open_pr_model recommends Review (not Resume,
  not blocked); next_move routes `owner=Review` (not Ci, not "fixing CI").
- Head with a real leaf (rust-test): still Resume, still `owner=Ci` / "fixing CI".
- Sabotage: reverting each consumer change reddens its first test; the real-leaf
  test passes with the bug fully present.
