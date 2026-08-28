# 5 Whys: Managed Task submit regained a second shipping gate

## The Problem

Main again allowed `lf pr submit` in a managed Task worktree after #1233 had
removed that path, so one Task could carry both its Loopflow `finally` review
and a separate human GitHub merge decision.

## Observed History

- #1233 (`2a242ddd9`) added `reject_managed_task_submit` and tests and docs that
  directed managed Tasks to `lf pr land`.
- The architecture branch was based on that exact change:
  `git merge-base c1f78f397 2a242ddd9` resolves to `2a242ddd9`.
- Source commit `c1f78f397`, later merged as #1236 (`fa0186c4d`), deliberately
  removed the guard. Its review says this made the controller's `finally` flow
  an authorization gate over a delivery command. Its demo ends controller-free
  Task work with `lf pr submit`.
- The same commit reversed both behavioral tests: managed submit changed from
  expected refusal to expected success, and the docs and prompt goldens were
  changed with them. The branch's test evidence therefore confirmed the new
  behavior; it did not expose an untested regression.
- The original #1233 guard ran after `prepare_land`. Its tests began from clean,
  committed work, so they proved no push or GitHub mutation but did not prove
  that a dirty worktree stayed untouched.
- The current restoration resolves only managed Task Work, reads no controller
  state, and refuses before scratch cleanup, commit, rebase, durable request,
  push, or GitHub mutation. This demonstrates that the policy does not violate
  the Work/controller dependency boundary.

The earlier recovery hypothesis that stale integration accidentally overwrote
#1233 is false. The change was an intentional descendant design decision.

## Chain

Managed Task submit regained a second shipping gate → #1236 intentionally
removed the refusal → the architecture classified the refusal as controller
authorization → it conflated dependency separation with delivery policy → all
local oracles were rewritten to confirm that interpretation → Task shipping
authority had no independent domain invariant.

**Problem**: `lf pr submit` marked a managed Task PR ready, recorded a
`PrMergeMode::User` request, assigned a human, and asked for a merge click even
though managed Tasks settle through their reviewed `finally` outcome.

↳ *Could we have caught this earlier?*  
Yes. A stable Task delivery contract would have made the assertion inversion
visible as a product behavior change rather than incidental architecture work.

**Why 1**: #1236 explicitly deleted `reject_managed_task_submit`, changed its
tests to require managed submit to succeed, and documented `submit` as the end
of a controller-free Task path.

↳ *What process allowed this?*  
The branch reviewed itself against an architecture Done When that required
delivery commands to work without controller authorization. The review listed
removing the refusal as a positive result.

**Why 2**: The design treated the refusal's explanation—“the Task's
`finally` review already owns the shipping decision”—as proof that delivery
depended on controller state.

↳ *What assumption was wrong?*  
It assumed every rule exercised by a controller belongs to the controller.
Shipping disposition is instead a policy of managed Task delivery. The
controller may decide when to request it, but Work can reject an illegal second
decision without loading a playhead, provider session, gate proposal, or any
other controller fact.

**Why 3**: The refactor conflated two separate questions:

1. Can Task Work and delivery exist without a controller? Yes.
2. May every delivery transition be used for every kind of Work? No.

Controller independence removed an ownership edge. It did not require managed
Task settlement to expose the non-Task human-submit transition.

↳ *Why did tests not preserve the distinction?*  
The relevant tests were framed as implementation-local expectations. The
refactor reversed them along with the code, so the suite had no independent
contract against which to reject the new expectation.

**Why 4**: The architecture design, review, demo, docs, and goldens formed one
self-consistent evidence set, but none enumerated preserved user-visible
delivery behavior or named a superseding product decision for changing it.
Passing tests proved consistency inside the branch, not continuity of the
shared API contract.

↳ *Why was that possible?*  
The policy existed as one guard plus prose centered on `finally`. The durable
Task PR model still represented `PrMergeMode::User`, and the shared merge-write
API accepted it. Deleting the guard made the formerly illegal state writable
again without any type or store invariant failing.

**Why 5 (Root)**: Managed Task shipping authority is not represented as an
independent Work-domain invariant. It was encoded only at one command edge and
described in controller vocabulary, so a valid effort to separate controllers
from Work could reinterpret the policy as unwanted coupling and deliberately
rewrite every executable oracle.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 2 | Which other Work rules are described in controller vocabulary and therefore vulnerable to the same reinterpretation? | High |
| Why 4 | What is the smallest review-time signal for an intentional assertion inversion without making architecture work dependent on frozen tests? | Medium |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Refuse managed Task submit before every local, durable, and remote mutation; keep ordinary non-Task submit unchanged. | This reported second-gate path and the untested dirty-worktree mutation window in #1233. |
| Structural | Make the managed Task settlement matrix explicit at the Task delivery write boundary: managed Tasks can request exact-head automatic settlement, never a new user-merge request. Keep legacy `User` values readable only if stored history requires it. | Another command or refactor bypassing the CLI-edge guard and writing the same illegal Task state. |
| Process | When a branch reverses a user-visible behavior assertion, require its design and review to name the superseding product contract and the surface owner whose decision it replaces. | A self-consistent refactor silently redefining a shared API while all of its edited tests pass. |
| Systemic | Document architecture boundaries as two matrices: dependency ownership and legal domain transitions. Controller independence must not imply that every Work operation is legal for every Work kind. | Future architecture simplifications erasing policy because its current caller lives in a higher layer. |

## Changes to Implement

- [x] Restore the managed refusal ahead of scratch cleanup, commit, rebase,
  durable request, push, and GitHub mutation.
- [x] Prove a dirty managed worktree keeps its HEAD, status, remote, durable
  request, and GitHub state unchanged; prove ordinary non-Task submit still
  assigns for review.
- [x] Restore prompts and user docs to teach publish during work, then managed
  Task review and `land`.
- [x] Move the invariant from the `submit` command edge to the managed Task
  merge-request write boundary: Task delivery exposes only an exact-head Auto
  request, while generic persistence still reads and reconciles historical
  `PrMergeMode::User` rows.
- [x] Pin the Task delivery invariant with a controller-free Work test: `land`
  records and replays only an exact-head automatic settlement request.
- [ ] Add the shipping disposition matrix to the delivery architecture.
- [ ] Add an assertion-inversion check to architecture review guidance: changed
  user behavior requires an explicit superseding contract, not only updated
  tests and goldens.
