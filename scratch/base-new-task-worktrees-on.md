# Base new Task worktrees on canonical main

## Problem

`lf task run` currently gives two different actors authority over a new root
Task. The canonical `main` checkout is the Wave/Project control plane, but Task
placement fetches `origin/main`, records that remote tip as the PR base, and
refuses when clean canonical `main` contains commits that origin does not. A
Wave can therefore commit durable memory successfully and immediately lose the
ability to delegate its next Task until somebody publishes or discards that
memory.

The refusal was introduced to prevent an inherited canonical-main commit from
appearing in the Task's GitHub PR. Removing only the refusal would reintroduce
that failure: current root-Task rebase and PR-copy paths derive authored history
from `origin/main`, so they would treat both the Wave commit and the Task commit
as Task work. The fix must move the authority, not disable the proof.

For every managed Task PR, `TaskPr.base_commit` must mean one thing: the exact
commit the Task-owned branch forked from. Before integration, Task work is
`base_commit..HEAD`. During integration, Loopflow replays exactly that range
onto one pinned target. After successful integration, that target becomes the
new recorded base. Root and stacked Tasks differ in target selection, not in
how their authored range is identified.

## The demo

Commit a Product Wave memory change on clean canonical `main` without pushing,
then run `lf task run LOO-256`. The sibling Task worktree appears at that exact
local `main` SHA, its active PR and initialization event record the same SHA,
and running the command again returns the same Task. After Task work is added,
`lf pr land` rebases only `base_commit..HEAD`; the GitHub range contains the
Task change and not the unpublished Wave commit.

## Approach

### Placement reads canonical main once

For a new root Task, keep `ensure_clean_main` as the entry boundary. Once PM,
lifecycle, and collision resolution have identified a genuinely new placement,
resolve the canonical checkout's `HEAD` to an immutable commit SHA and use that
same string for both:

- `PlacementPlan.base_ref`, so `git worktree add -b <branch> <sha>` creates the
  sibling from the captured commit rather than a movable branch or remote ref.
- `TaskPr.base_commit`, which is already persisted atomically with the Task,
  Run reservation, and `WorktreeInitializing` event and later projected as the
  active Task base.

Do not add a second base field to `Task`. The Task owns a serial PR chain; the
active `TaskPr.base_commit` is the Task's current base, and `PrStarted` plus
`WorktreeInitializing` are historical evidence of that same value.

Delete root placement's `resolve_upstream_base` call and
`refuse_if_canonical_ahead`. Root placement performs no Git fetch, parity test,
push, reset, or reconciliation. Stacked placement keeps its existing contract:
the published parent PR is the child's target and fork.

An immutable SHA closes the placement race. If another process moves canonical
`main` after the snapshot, both the branch and durable record still name the
same commit. If uncommitted files appear after the cleanliness check, a
worktree created from the commit object cannot inherit them.

### The recorded base owns every managed Task rebase

Generalize the existing stacked-rebase context into a managed-Task rebase
context resolved from the active `TaskPr`. It carries:

- the active PR and its immutable `base_commit`;
- whether the PR is rooted or has a parent PR;
- the live parent branch when a stacked parent has not merged.

Every managed Task rebase passes the recorded base as the replay fork. Plain
non-Task branches retain the existing merge-base heuristic.

| Boundary | Fork | Target | Durable result |
|---|---|---|---|
| Root Task placement | clean canonical `main` HEAD `C` | `C` | branch starts at `C`; base is `C` |
| Stacked Task placement | published parent tip `P` | `P` | branch starts at `P`; base is `P` |
| Root Task integration | active PR base `B` | pinned current default-branch tip `T` | replay `B..HEAD`; base becomes `T` |
| Stacked integration | active PR base `B` | live parent, or pinned default-branch tip after parent merge | replay `B..HEAD`; preserve or clear parent link; base becomes target |

`lf rebase`, final landing rebase, manual conflict continuation, and automated
conflict recovery must all use this context. Consolidate their postconditions
into one path:

1. Before rewriting, prove the active PR still owns the current branch, its
   recorded base equals the selected fork, and the fork is an ancestor of
   `HEAD`.
2. Pin the target SHA, then replay exactly `base_commit..HEAD` with the existing
   `--onto` machinery. This is already the squash-proof mechanism used by
   stacked children.
3. Verify the pinned target is an ancestor of the new head, no conflict or new
   tracked dirt remains, and expected authored work did not disappear.
4. When a push was requested, verify remote-head equality before changing the
   durable base.
5. Persist the pinned target as the active PR base. Clear a parent link only
   after a merged stacked child has collapsed onto the default branch.

Delete caller-specific “stacked versus ordinary” metadata recording where the
shared rebase completion path can own it. A failed rebase, failed push, or
failed remote-head proof leaves `base_commit` unchanged.

### Range checks reflect the lifecycle boundary

The current `M == B` parity proof conflates three different moments. Replace
the `StaleBaseAction` policy switch with explicit boundary semantics:

- **Authored source:** before integration, require only that recorded base `B`
  is an ancestor of Task `HEAD`. The Task-owned change is `B..HEAD`, regardless
  of whether `B` is ahead of, behind, or divergent from the current remote tip.
  This is the precondition for exact replay and for pre-rebase emptiness checks.
- **Publication:** `lf pr publish` still owns no integration. Require GitHub's
  fork against the target to equal `B`; otherwise refuse before push and direct
  the operator to `lf rebase`. A canonical base absent from origin is expected
  pre-integration state, not “foreign contamination,” but it still must not leak
  into a PR.
- **Integrated:** after exact replay, require the pinned target, recorded base,
  and GitHub fork to agree. `lf task changes`, generated PR copy, and GitHub then
  describe the same minimal range.

Managed Task PR-copy generation must inspect `TaskPr.base_commit..HEAD`, not
`origin/main..HEAD`, because landing generates copy before it performs its
owned rebase. Generic non-Task PRs continue to use their branch target. Update
the built-in PR workflow guidance so managed Tasks inspect their recorded range
instead of treating `origin/main` as a second authored-history authority.

### Idempotency and ownership stay unchanged

The existing-Task fast path remains first: a second `lf task run` validates the
pinned lifecycle and returns the durable Task without reading a new main base,
creating a branch, or changing PR history. A deterministic branch or worktree
that exists without a Task remains a collision, not something to adopt. Dirty
canonical main remains a launch refusal. Stack parent identity, Task mutation
leases, worktree writer ownership, and serial PR rotation retain their current
guards.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Can placement start from an exact local commit without publishing it? | `create_from_placement_plan` passes `PlacementPlan.base_ref` directly to `WorktreeBranch::New { start_point }`; Git accepts the locally present commit SHA. No remote operation is part of worktree creation. | Capture canonical `HEAD` once and pass the SHA, not `main` or `origin/main`. |
| Can uncommitted canonical files leak? | `ensure_clean_main` already refuses pre-existing dirt, and a worktree created from a commit object materializes only that committed tree. | Preserve the cleanliness gate; do not stash, copy, or relax it. |
| Is deleting the ahead-of-origin refusal sufficient? | No. Root land currently supplies `fork_base: None`, so the rebase heuristic sees `origin/main..HEAD` as authored history. PR-copy generation reads the same remote range before rebase. | The recorded base must drive integration and PR-copy inputs in the same change. |
| Does exact-fork replay already exist? | Yes. `rebase_with_recovery` accepts `fork_base` and uses `git rebase --onto`; `stacked_child_collapses_onto_main_dropping_squashed_parent` proves it removes inherited parent history while preserving child work. | Generalize the proven mechanism from stacked Tasks to all managed Tasks instead of adding a second rebase path. |
| What currently prevents the configured failure? | The focused `submit_refuses_a_contaminated_range_before_any_push` test passes and confirms the current verifier rejects `M < B` before any push. This protects PR minimality but also makes canonical-main commits illegal bases. | Replace the fixed-context precondition with boundary-specific proofs; retain the no-leak publication proof. |
| What if canonical main moves during Task launch? | A branch created from a symbolic `main` ref could observe a different commit from the one recorded. A captured SHA cannot move. | Store and place from the same immutable value. |
| What if origin advances, lags, or diverges after placement? | The Task-owned source remains `base_commit..HEAD`; exact replay onto a freshly pinned target is valid whenever the recorded base remains an ancestor of the Task head. | Do not classify the relation between base and origin during placement. Resolve it only at publication/integration. |
| Does this require a second Task base column? | No. Task snapshots and `lf task changes` already derive the active base from the active `TaskPr`; events preserve placement history. | Keep one durable authority and avoid a migration or cross-language DTO field. |
| Will existing Task reuse change base when main moves? | The existing Task branch returns before placement planning today. | Keep that ordering and add a regression assertion over path, PR id, and base SHA. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Push or reset canonical main before Task creation | Keeps origin as the universal base and preserves current verifier assumptions. | Mutates external state, makes Wave learning block delegation, and directly violates the requested no-push/no-reset contract. |
| Delete only `refuse_if_canonical_ahead` while still basing on `origin/main` | Small code change. | Does not satisfy the outcome: the worktree still omits canonical-main state. Basing on local `main` without changing integration would instead leak that state into the PR. |
| Copy canonical-main files into an origin-based Task worktree | Could reproduce the working tree without changing ancestry. | Converts committed control-plane state into uncommitted Task edits, destroys authorship, and silently includes data the Task does not own. |
| Add an allowlist for Wave memory commits | Preserves the old origin-first model for selected paths. | Extends the fixed-context precondition, duplicates path policy, and fails for any legitimate canonical-main code or documentation commit. |
| Make a hidden local integration branch and base Tasks there | Separates canonical main from origin while keeping a remote-shaped ref. | Creates another movable base authority and a new reconciliation lifecycle. The exact canonical commit already is the required authority. |

## Key decisions

### Canonical main is placement truth; origin is integration truth

The canonical checkout is where Wave and Project work commits durable context.
A new root Task must see that exact committed state. Origin matters later, when
Loopflow prepares a publishable PR. This separation removes the launch
precondition without weakening PR minimality.

### A recorded base is authority, not a diagnostic

`TaskPr.base_commit` must select the replay range, PR-copy range, Task diff, and
post-integration proof. Keeping it only as metadata while rebase recomputes a
merge-base is the failure mode this design removes.

### Publication does not integrate

`lf pr publish` may refuse an unpublished canonical base and tell the caller to
run `lf rebase`; it must not silently rebase, push canonical main, or publish a
nonminimal PR. `lf pr land` already owns integration and must accept this state,
perform the exact replay, then publish the verified head.

### Success preserves local continuity without transferring ownership

In the successful path, a Wave memory commit can immediately inform a Task,
yet that commit remains owned by canonical main and does not appear in the Task
PR. The surprising but useful property is that committed local context can be
consumed before it is remotely published.

### Failure would be a plausible launch followed by a lying PR

The dangerous near-miss is a green placement test while landing or PR-copy
generation still uses `origin/main`. Six months later this would present as
unrelated Wave commits in Task PRs, overwritten durable bases after failed
pushes, or review copy describing changes the final PR does not contain. The
end-to-end range proof is therefore part of this slice, not follow-up work.

## Scope

- In scope: new root Task placement from exact clean canonical-main `HEAD`;
  active PR/event base recording; root and stacked managed-Task replay from the
  durable fork; publication/integration range semantics; Task PR-copy range;
  focused temporary-repository behavior tests; user and built-in workflow docs.
- Out of scope: relaxing the clean canonical-main requirement; including
  uncommitted files; pushing or reconciling canonical main; changing stacked
  parent publication/ownership; changing serial PR rotation; changing raw
  `lf wt` defaults; inventing a Task-level duplicate base field; automatic
  dependency publication for Tasks built on unpublished code.
- Metric claim: none. The sponsored task-loop-trust score remains Unknown
  because lifecycle-scorecard instrumentation is absent; this work supplies a
  binary launch/range proof, not a replacement metric.

## Done when

Add focused behavioral coverage in temporary repositories and retain the
affected existing suites:

1. In a repository with clean canonical `main` one or more commits ahead of
   `origin/main`, invoke the real `task_run` path with seeded PM/provider state
   and bounded fake containment. Assert the sibling worktree `HEAD` equals the
   pre-launch canonical SHA; its tree includes the committed canonical change;
   `TaskPr.base_commit`, `WorktreeInitializing.base_commit`, and
   `PrStarted.base_commit` equal that SHA; canonical main and origin are
   unchanged; and the new worktree is clean.
2. Invoke `task_run` again for the same issue after canonical main moves. Assert
   the Task id, worktree, branch, active PR id, and base SHA are unchanged.
3. From the ahead-of-origin fixture, add Task-owned work and run the real
   integration path against a pinned `origin/main`. Assert only
   `old_base..old_head` is replayed, the unpublished canonical commit/file is
   absent from the final GitHub range, Task work is present, the durable base is
   the pinned target, and a rejected push leaves the old base recorded.
4. Assert `lf pr publish` before integration performs no push and directs the
   caller to `lf rebase`; after integration it publishes the minimal range.
5. Retain stacked-child collapse, stale-base healing, dirty-main refusal,
   unowned branch/worktree collision, empty-range refusal, and no-remote
   placement coverage.

Focused commands:

```bash
cargo test -p loopflow --test task_initialization_tests
cargo test -p loopflow --test task_pr_range_tests
cargo test -p loopflow --test rebase_tests
```

The configured Product proof is the demo: from clean canonical Product `main`
with its memory commit still unpublished, `lf task run LOO-256` creates and
starts the sibling Task without first pushing or resetting the Wave commit.

## Forbidden outcomes

- Root placement still fetches or compares `origin/main` before creating a
  Task.
- The worktree is created from movable `main` while a separately read SHA is
  recorded, allowing branch and base to disagree under concurrency.
- Ahead/divergent canonical commits are relabeled with an allowlist or special
  path rule instead of removing origin parity from placement.
- Dirty or untracked canonical files appear in the Task worktree.
- A second Task base field, migration default, compatibility fallback, or DTO
  mirror competes with `TaskPr.base_commit`.
- Root integration uses a runtime merge-base and replays canonical-main commits
  that precede the recorded Task fork.
- PR copy or built-in review guidance describes `origin/main..HEAD` when the
  managed Task's authored range is `base_commit..HEAD`.
- Publication silently integrates, mutates canonical main/origin, or pushes a
  range whose GitHub fork differs from the recorded base.
- Failed local verification, push, or remote-head equality advances the durable
  base.
- Existing Task reuse adopts an unowned worktree/branch or changes the Task's
  active PR/base.

## Internal slices

1. **Canonical placement authority.** Capture exact clean canonical `HEAD`, use
   it as the root placement start point and initial PR base, delete the fetched
   origin precondition, and prove creation plus idempotent reuse.
2. **Durable replay authority.** Generalize exact-fork rebase from stacked
   children to every managed Task, split publication/source/integrated range
   checks, and make successful rebase completion the sole base-update owner.
3. **Range consumers and proof.** Route PR-copy/review inputs through the
   recorded Task range, add the landing/no-push failure proofs, and update the
   user/built-in documentation.

These are ordered implementation cuts inside one PR. Slice 1 must not ship
without slices 2 and 3 because placement would work while PR authorship would
be wrong.

## This slice

Ship the complete three-part change in this PR: exact canonical placement,
recorded-base integration, and end-to-end behavioral proof. The focused proof
is the temporary-repository `task_run` plus landing path where canonical main is
ahead of origin and only Task-owned work survives into the publishable range.

## Slice ledger

- 2026-08-23 — Current root placement resolves/fetches `origin/<default>` and
  calls `refuse_if_canonical_ahead` before Task reservation. This exactly
  explains the 2026-08-21 Product failure.
- 2026-08-23 — `cargo test -p loopflow --test task_pr_range_tests
  submit_refuses_a_contaminated_range_before_any_push` passes, proving the old
  fixed-origin safety gate remains active and precedes remote mutation.
- 2026-08-23 — `cargo test -p loopflow --test rebase_tests
  stacked_child_collapses_onto_main_dropping_squashed_parent` passes, proving
  the existing exact-fork `--onto` mechanism preserves child work while
  dropping inherited history.
- 2026-08-23 — Design changed from “remove the ahead refusal” to “make the
  recorded fork authoritative through placement, PR copy, rebase, and landing”
  after tracing root landing's `fork_base: None` and pre-rebase
  `origin/main..HEAD` PR-copy input.
