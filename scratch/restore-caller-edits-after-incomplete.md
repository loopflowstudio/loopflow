# Restore Caller State by Removing Release from the Caller Worktree

## Problem

`lf release run` mutates a pre-existing checkout before it decides whether to
start a release or resume an existing tag. `release_run` resolves the canonical
checkout, calls `sync_main`, and only then inspects the latest tag and hosted
build. When `origin/main` rewrote a path with local edits, `sync_main` correctly
refused to pop its auto-stash over the new tree: doing so could resurrect stale
content. The release continued with the caller's tracked and untracked edits in
`sync_main: auto-stash`.

That safe generic sync behavior becomes data loss at the release boundary. A
later artifact timeout or publisher validation failure returns without an owner
for the stash. During the v0.12.14 resume on 2026-08-22, the visible result was
an Infrastructure Wave memory file at its pre-edit content even though the
release target process itself exited successfully and publication remained
incomplete.

The release controller should never borrow a caller checkout. Scheduled release
runs and active Wave writers benefit: a release may fetch, prepare, tag, resume,
validate, and publish without changing the branch, HEAD, index, tracked bytes,
untracked bytes, or stash list of any checkout that existed when it began.

## The demo

Run the release-shaped fixture with staged, unstaged, and untracked caller work,
an existing release tag, and an injected artifact-download timeout. The command
reports the resumable failure; byte-for-byte caller-state snapshots before and
after match. Flip the fixture to success and retry: the same tag publishes once,
with the caller snapshot still unchanged.

## Approach

Give the release controller a leased, detached sibling worktree rooted at the
fetched default-branch commit. The controller worktree is the only checkout the
release may synchronize or use for mutable orchestration. Existing user, Wave,
Task, and canonical-default worktrees are read-only to the release path.

Introduce a release control-worktree owner around `release_run`:

1. Resolve the canonical repository only as the Git common-state and durable-log
   home. Do not reset its checked-out branch.
2. Acquire the existing narrow worktree lease for the deterministic
   `<repo>.release-control` path. The process lease proves no live release owns
   the path; it does not by itself authorize deleting what is there.
3. Fetch the default branch into `origin/<default>` without moving
   `refs/heads/<default>` or touching a checkout.
4. Materialize `<repo>.release-control` with `git worktree add --detach --lock
   --reason <release-owner-marker>` at the fetched commit. The Git worktree lock
   is the durable ownership marker left across process death. A detached
   controller creates no second release branch or resumable identity.
5. Load release configuration from this committed, synchronized checkout and
   execute release selection, verification, PR recovery, tagging, workflow
   observation, and publisher control from it.
6. Remove the detached worktree under the same lease on every returned success
   or error. Removal first verifies the exact registered path, detached state,
   and release lock reason. A cleanup error names and preserves the exact
   release-owned path and marker; the next invocation recovers it after
   reacquiring the lease. Best-effort `Drop` cleanup covers unwinding, while the
   explicit finish path retains both the workflow error and any cleanup error.

At startup, recover a stale controller only when the registered worktree carries
the same detached-state and lock-reason evidence. An unregistered directory, a
branch worktree, or a different/missing lock reason is a collision: fail closed
with the exact path and observed ownership evidence, leaving it untouched. This
keeps crash recovery from turning the deterministic name into deletion
authority.

Keep two paths explicit through the publisher boundary:

- `control_repo`: the detached, current-main checkout that supplies config,
  controller scripts, hooks, and GitHub command context.
- `release_home`: the canonical repository root used only for shared Git
  metadata, worktree placement and leases, and durable `.lf/logs` receipts.

The publisher's tagged source remains the already-leased
`LF_RELEASE_SOURCE_REPO`. `LF_RELEASE_MAIN_REPO` continues to name the durable
release home. This preserves the shipped current-controller/exact-tag-source
contract while preventing a temporary controller checkout from owning receipts
that disappear during cleanup.

Remove every transitive default-checkout sync from the full release path. The
initial `sync_main` call goes away. Release preparation starts from the
controller's fetched `HEAD` (or `origin/<default>`) with
`create_named_worktree(..., false)`. Dirty release-PR reintegration explicitly
fetches `origin/<default>` and also creates its worktree without the helper's
`sync_default_base` behavior. `release.rs` should no longer import or call
`sync_main`, directly or through a release-owned `create_named_worktree(...,
true)` invocation.

Remote version tags remain authoritative and shared by every linked worktree.
An artifact timeout or candidate-validation failure therefore keeps exactly one
resumable tag. Retrying creates no replacement tag or control branch and invokes
the successful publication stage once.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Does the reported failure reproduce on the current v0.12.14-shaped path? | Yes. A local existing-tag fixture with an overlapping `wave/infrastructure/MEMORY.md` edit reproduced both a mocked `gh run download` timeout and a publisher candidate-validation failure. Each returned with upstream memory in the checkout, the caller's unrelated untracked file absent, and both edits retained only in `sync_main: auto-stash`. | Treat this as release ownership failure, not artifact transport or candidate validation failure. Cover both terminal paths with behavioral tests. |
| Is `sync_main` itself behaving incorrectly? | No. Its overlap rule was introduced to prevent a stash pop from silently reverting freshly merged work. The focused `sync_main_does_not_revert_rewritten_paths` test passes and demonstrates that invariant. | Keep generic synchronization behavior intact. Stop calling it from release orchestration. |
| Is the initial `sync_main` the only mutating route? | No. New release preparation and dirty-PR reintegration call `create_named_worktree(..., true)`, which calls `sync_main` and can reset whichever worktree has the default branch checked out. | Replace all three routes; fixing only the resume path leaves new releases and PR recovery unsafe. |
| Can a stash guard restore exact caller state? | Not robustly. The current pop does not request index restoration, overlap may conflict, untracked paths may become tracked upstream, stash ordering is shared repository state, and `Drop` cannot return an actionable restoration error. Exact restoration would become a second recovery protocol. | Reject caller mutation and restoration. Isolation makes restoration conflicts impossible and leaves any pre-existing stash untouched. |
| Can Git provide a synchronized checkout without a local branch? | Yes. The installed Git supports `git worktree add --detach <path> <commit-ish>`. Fetching updates `origin/<default>`; linked worktrees share objects, tags, and remote-tracking refs without requiring the local default branch to move. | Use a detached controller at the fetched commit. The release tag remains the sole durable identity. |
| Does isolation break current-main publisher control over exact tagged source? | No. Publisher command expansion and preflight can use the controller checkout, while the existing tagged publisher worktree remains `LF_RELEASE_SOURCE_REPO`. Durable receipts can continue to use the canonical release home. | Thread control and home paths separately instead of replacing every `main_repo` argument mechanically. |
| What happens after an abrupt process death? | The process lease releases, while Git preserves the controller's registered detached worktree and lock reason. A new lease holder can verify that durable ownership marker and remove the exact stale checkout. Tags, hosted workflow state, and draft/published release state live outside the checkout. | Create the worktree with an atomic Git lock reason, require that marker for stale recovery, and never infer release progress from the worktree's survival. |
| Could the deterministic path collide with human work? | Yes. A directory or worktree may already occupy `<repo>.release-control`; path equality alone is not ownership. Git exposes a durable worktree lock reason in porcelain output. | Remove only a registered detached checkout with Loopflow's exact release marker while holding the process lease. Report every other occupant and leave it untouched. |
| Does this alter hosted release semantics or require a new external API? | No. GitHub workflow lookup, artifact download, publisher validation, tag idempotence, and publication stay unchanged. Only the local execution checkout changes. | Keep LOO-259 publisher ownership and LOO-261 candidate validation out of this slice. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Scoped caller-state guard around `sync_main` | Smaller call-site diff and can reset to the original commit before applying a private stash with `--index`. | It deliberately mutates caller state, must preserve shared stash ordering and partially staged files, can conflict with paths created during release hooks, and needs durable recovery for every failed restore. The recovery mechanism is as hard as the original task. |
| Operate only on remote refs with no controller checkout | Avoids worktree lifecycle and caller mutation for Git-only selection and tagging. | Release config discovery, verification hooks, current-main publisher scripts, and release preparation require one coherent filesystem tree. Re-implementing all of them as ref-aware readers would spread the ownership boundary across the release module. |
| Require a clean canonical default checkout | Makes `sync_main` predictable and avoids auto-stashing. | Refuses a valid context, blocks scheduled releases on unrelated work, and preserves the shared-checkout coupling that caused the incident. |
| Use a separate clone | Strong isolation, including separate mutable refs. | Duplicates repository/auth setup, introduces a second tag/ref reconciliation surface, and weakens the exact-one-release-identity story. A linked detached worktree gives the required filesystem isolation while sharing authoritative refs. |

## Key decisions

- Isolation is the state-preservation boundary. The release does not snapshot,
  stash, reset, pop, or otherwise restore a caller checkout because it never
  changes one. Therefore the requested restoration-conflict case is removed by
  construction: no release-created caller stash exists to conflict or be
  silently dropped. The analogous isolated-worktree collision retains Git's
  lock reason and returns the exact recovery path.
- The detached controller is ephemeral execution state, not release identity.
  The remote tag and hosted release state remain the only resumable facts.
- Release configuration comes from fetched committed main, not from dirty bytes
  in the canonical checkout. A scheduled release should execute reviewed
  release policy.
- The controller path is deterministic and lease-protected. This serializes
  local release controllers. Its Git lock reason supplies durable deletion
  authority, making crash recovery exact without treating an arbitrary path as
  release-owned or creating timestamped debris.
- Cleanup cannot hide the primary failure. If both the workflow and cleanup
  fail, the returned evidence includes both errors and the exact retained path.
  No stash reference is created or dropped.
- Keep `sync_main`'s overlap protection unchanged for callers that really do
  synchronize a checkout.

Wild success is boring: developers stop checking `git stash list` after a red
nightly because release automation is physically incapable of moving their
checkout. The strongest unexpected benefit is that dirty local release config
can no longer influence scheduled publication.

Wild failure would be a partial isolation refactor: the first sync moves, but a
`create_named_worktree(..., true)` later resets canonical main, or publisher
receipts are written inside the ephemeral checkout and disappear. Source-level
audits and end-to-end state snapshots must cover these near-misses explicitly.

## Scope

- In scope: the full `release_run` start/resume path; detached controller
  creation, lease, stale recovery, and cleanup; new-release preparation;
  dirty-release-PR reintegration; publisher control/home path separation; exact
  caller-state fixtures for artifact timeout, candidate-validation failure,
  retry, and first-attempt success.
- Out of scope: retrying or publishing v0.12.14; changing artifact download
  retry policy; changing candidate validation; redesigning the tagged publisher
  worktree lease from LOO-259; changing generic `sync_main`; changing decomposed
  read/manual release commands that do not synchronize a checkout; building a
  generic multi-product release platform.

## Done when

- A release-shaped fixture begins on stale default main with an existing remote
  tag, a pre-existing stash entry, a staged tracked edit, an overlapping
  unstaged Wave-memory edit, and an untracked file. Its caller snapshot includes
  branch name, `HEAD`, NUL-delimited porcelain status, cached and uncached binary
  diffs, relevant file bytes, and the stash list.
- Injected artifact-download timeout and candidate-validation failure each
  return the expected resumable error with the caller snapshot byte-for-byte
  unchanged, the original tag at the original commit, no extra release identity,
  and no surviving release-control worktree.
- A retry after each failure returns `ReleaseRunOutcome::Resumed`, publishes the
  same tag exactly once, and still leaves the caller snapshot unchanged.
- A first-attempt successful resume has the same caller-state guarantee.
- A stale detached controller checkout is recovered only after acquiring its
  exact-path lease and matching its durable Git lock reason. A live lease,
  unregistered directory, branch checkout, or mismatched marker fails closed
  without touching the occupant.
- `release.rs` has no `sync_main` import/call and no release-owned
  `create_named_worktree(..., true)` call. The fake publisher proves its control
  script/config came from fetched main while its source checkout is the existing
  tag.
- Focused proof passes with
  `cargo test -p loopflow --test release_tests release_run_preserves_caller_state`
  plus the control-worktree unit tests. The affected release suite,
  `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings` pass before
  shipping.
- No release is retried or published from this Task. The next scheduled release
  is the hosted post-merge proof toward the Stability & Security weekly-release
  KR.
