# How lf breaks for friends

Draft implementation for Jack Heart's review — 2026-10-11.

## What to build

Make lf honor unfamiliar repositories' existing practices while preserving a
single meaning for publication, review handoff, merge, and release completion.

Jack's framing: “Imagine I give lf to my friends to use. How might it break?”
Jack asked for “coherence over customizability” and a best-effort implementation
to read and give feedback on. The mechanisms below are implemented proposals,
not evidence of Jack's acceptance of the resulting behavior.

## Placement

Wave unresolved; none supplied. Companion `levels-of-lf` owns instruction tiers:
launcher-only use remains useful without adopting Tasks or a planning service.
This branch owns compatibility of the native operations those instructions use.

## Scope and decisions

Additive series. This reviewable keystone removes incidental destructive defaults
and supplies explicit GitHub teammate handoffs. No generic command overrides or
second review-host framework. Existing Flows/Workflows own procedure.

Jack selected [failure-map](failure-map.md) concerns 01, 03, 06, 10, 12–16 and
deprioritized 02, 04, 05, 07–09, 11. Remaining prioritization was delegated.
Worktrees are provisionally acceptable; their placement should follow existing
conventions without requiring lf configuration.

Three boundaries keep generalization coherent:

| Owner | Choices |
| --- | --- |
| lf | Stable lifecycle operations, preserved work, revision-bound requests and observed completion |
| Repository | Reviewer choice, procedure, checks, permitted merge methods, release build/publishing commands |
| Host | PR identity, actual reviews, protection, queues and completion evidence |

An arbitrary replacement for `pr open` cannot satisfy GitHub-specific identity
and reconciliation. Adapt whole host lifecycles only when a second concrete host
is selected. A successful script is not proof that a PR merged or an artifact
was delivered.

## The demo

1. In a real Git fixture with two Conductor-style city checkouts or Herdr,
   Worktrunk or Claude-style directories, plan/create another worktree without
   config. Observe the established parent; reuse existing branches in place.
2. Fail a checkout hook. Observe an error with its output and the retained path.
   Reject a normal push. Observe unchanged remote history and preserved local work.
3. Preserve existing tracked and untracked scratch by default; opt in to cleanup
   only in a repository that intentionally treats that directory as disposable.
4. Publish a PR, then explicitly request `alice` and `acme/platform` with
   `lf pr request-review -r alice -r acme/platform`. `lf pr reviews` reads the
   requests, aggregate decision and review commits. `submit` leaves assignments
   alone. A teammate reviews on GitHub without lf.
5. In provider fixtures, disallow squash: auto-merge chooses another allowed
   method. Configure rebase: it uses rebase. Enable a queue: the queue owns the
   method. Required pending checks return 8, not a failure diagnosis.

Provider fixtures are headless demos, not live GitHub interoperability proof.
No live reviewer requests or merge mutations were performed for this review.

## Data structures and key functions

- Existing `WorktreeName`, `PlacementPlan` and Git registrations remain the
  placement model. `main_repo_root` chooses a working checkout, not bare metadata.
  `inferred_worktree_dir(repo, component)` infers a common parent from at least two
  live branch-bearing non-primary worktrees. Only genuine repository-prefixed
  siblings retain `<repo>.`; other inferred layouts use the flat lf name.
- `Config.clear_scratch_on_land: bool` defaults false. `clear_scratch` reads only
  repository config; persistent workspace preservation remains. This repository
  opts in to its existing scratch-clear convention.
- `Config.merge_method: Option<MergeMethod>` is a repository-only preference.
  `MergeMethods::select` honors it within GitHub's allowed methods, otherwise
  chooses squash, merge, then rebase. A queue supplies its own strategy.
- `PrCommand::RequestReview { pr, reviewer }` and `Reviews { pr, json }` operate on
  the existing PR. Review JSON passes through GitHub CLI fields; no local review
  database. Each submitted review retains its commit. Inline threads stay on GitHub.
- `pr_checks(watch, logs, required)` uses provider check selection and preserves
  provider exit codes. Pending does not launch failure-log retrieval.
- Automatic scratch traversal and repository MEMORY inclusion skip symlinks.
  This prevents incidental traversal into linked private files and cycles; it
  is not a secret scanner or a general exclusion policy.

No schema migration.

## Current system and Delete — do not maintain

Remove `push_with_fallback`, which escalated an ordinary push rejection into a
force push. Remove `assign_to_me` and tests naming assignment as requested review.
Replace fixed squash selection, branch-leaf filename inference, metadata-parent
root inference, and the hook-failure-as-success path in the same diff.

Retain observed PR/head identity, host protection, existing checkout reuse,
collision handling, explicit sync recovery, and release candidate verification.
Release already exposes `verify`, `prepare`, `workflow`, `publisher`, and
`completion`; this cut documents their ownership instead of adding a second
release customization system.

## Constraints and forbidden outcomes

No mandatory onboarding to discover a layout. No moving existing work to match
an inference. Directory compatibility does not register a workspace with another
app or run its setup. Explicit sync reset operations retain their existing lease
semantics; removing fallback does not claim all history rewriting is gone.

No assignment treated as review; no stale review treated as current approval;
no provider read failure treated as passing; no protection bypass; no successful
handoff reported as merge. Personal configuration cannot grant scratch deletion
or override repository merge policy.

A separate Git directory has no Git backlink to its original checkout. From that
checkout lf uses its actual root; from a linked checkout it uses an available
registered working checkout. This does not promise one canonical primary identity
for every separate-directory layout.

## Internal slices and remaining increments

**This slice:** native preservation and placement repairs, explicit teammate review
requests/reads, repository merge method selection, pending/required CI semantics,
and matching documentation. Focused verification is complete; the draft remains
open for Jack's feedback.

Later increments, not filed or launched:

- **Remote roles (10):** separate integration remote/base from publication remote
  across refresh, sync, push, PR targeting and reconciliation. Origin assumptions
  still span these operations; changing one call is not fork support.
- **Checkout completeness (14/17):** prove sparse patterns, initialized submodules,
  LFS filters and repo setup through a useful build. Hook failure truthfulness
  does not supply dependencies or initialize missing content.
- **CI preparation policy:** remove the global special treatment of a check named
  `scratch-clear` in `CiCheck::land_time_precondition`. It still leaks this repo's
  convention into repair selection. Preserve intentional preparation boundaries
  through repository-owned policy, without duplicating required-check authority.
- **Review feedback:** inline unresolved threads and revision-aware remediation.
  This cut reads submissions, not every discussion or a computed “safe to merge.”
- **Host reach:** select a real second host, then adapt identity/publication/read/
  completion together. Current review and release completion remain GitHub-based.

Existing dirty/index/untracked restoration tests cover part of concern 16; they
are evidence to retain, not a claim that every interrupted Git state is handled.

## Done when

Headless acceptance: `uv run python scripts/test_network.py cargo test --offline
-p loopflow --test worktree_tests --test commit_tests --test land_tests --test
pr_review_tests --test wt_ci_logs_tests --test task_pr_authority_tests` with stdin
closed and inherited LF variables cleared by the wrapper. Focused library checks
cover merge policy, ambient context and default scratch preservation; existing
checkout-restoration tests cover index/untracked recovery. Required formatting,
Clippy and documented-command checks pass. No display, network credentials, or
live review mutation required. CI retains the complete platform/release matrix.

Review finding: keeping review state at the provider avoids a second approval
model. Removing self-assignment fixes the misleading handoff; leaving queue
strategy unset preserves host authority. The remaining hardcoded CI check and
remote-role assumptions are explicit follow-ups rather than compatibility claims.

Check: isolated focused Cargo suites and selected library/recovery/documentation tests — 134 distinct tests passed (corrected merge fixture rerun); `cargo fmt --check`, `cargo clippy --offline --all-targets -- -D warnings`, and `git diff --check` passed; complete platform/release matrix remains with CI.
