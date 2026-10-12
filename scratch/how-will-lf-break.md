# How lf breaks for friends

Delivery authorized by Jack Heart — 2026-10-12.

## What to build

Make lf honor unfamiliar repositories' existing practices while preserving a
single meaning for publication, review handoff, merge, and release completion.

Jack's framing: “Imagine I give lf to my friends to use. How might it break?”
Jack asked for “coherence over customizability” and a best-effort implementation
to read and give feedback on. The mechanisms below are implemented proposals,
not evidence of Jack's acceptance of the resulting behavior.

## Placement

Wave: [product](../.lf/wave/product/GOAL.md), confirmed by Jack Heart on 2026-10-12.
This fits Product's ownership of external usefulness and the shared user contract.
Companion `levels-of-lf` owns instruction tiers:
launcher-only use remains useful without adopting Tasks or a planning service.
This branch owns compatibility of the native operations those instructions use.

## Scratch ownership — updated direction, 2026-10-12

Jack Heart moved scratch policy to the companion `levels-of-lf` work:
“we will clear whatever files we added in scratch as part of our flows by default.
if your repo has scratch/FOO, pre-existing, no worries.” This supersedes this
branch's repository-opt-in cleanup proposal as the intended end state; the
opt-in code below describes the current implementation, not accepted policy.

Gate imported the companion's committed scratch implementation from
`levels-of-lf` at `5c6ceac565`, without editing or launching that checkout.
`git::scratch::{base, added}` and `ops::scratch::clear` are the single cleanup
path, exposed through `lf scratch clear` and delivery. Existing files, edits,
renames and stacked parent files remain ordinary repository content. Only
committed additions relative to the Task/base are disposable; this is a Git-diff
proxy for Flow-added notes, not an authorship registry. The fallback base follows
Git tracking. The obsolete opt-in and global CI job-name exemption are removed.
This repository's CI owns its stricter scratch-free landing candidate policy.

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
3. Clear committed scratch additions during delivery while preserving existing
   files, edits, renames and uncommitted files during explicit cleanup.
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
handoff reported as merge. Repository merge policy remains separate from personal configuration.

A separate Git directory has no Git backlink to its original checkout. From that
checkout lf uses its actual root; from a linked checkout it uses an available
registered working checkout. This does not promise one canonical primary identity
for every separate-directory layout.

## Internal slices and remaining increments

**Completed keystone:** native preservation and placement repairs, explicit teammate review
requests/reads, repository merge method selection, pending/required CI semantics,
and matching documentation. Focused verification is complete; the draft remains
open for Jack's feedback.

Later increments, not filed or launched:

- **Remote roles (10):** finish publication, stacked-Task branches, PR targeting
  and reconciliation against the actual push/host destination. The adaptive Git
  increment below handles default-base integration, not full fork support.
- **Checkout completeness (14/17):** prove sparse patterns, initialized submodules,
  LFS filters and repo setup through a useful build. Hook failure truthfulness
  does not supply dependencies or initialize missing content.
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
strategy unset preserves host authority. The global hardcoded CI check exemption is removed; remote-role assumptions
remain explicit limits.

Check: canonical Wave migration — isolated Rust Wave/store/context/discovery/sync/DTO suites: 135 passed; Python path consumers: 63 passed; standalone Swift parser fixture passed; fmt, Clippy all-targets and diff checks passed; full Desktop tests deferred to CI (ViewInspector/Ghostty dependencies unavailable).

## Adaptive Git increment — 2026-10-12

Jack Heart authorized further adaptation without new subsystems. The narrower
variants of dismissed concerns 09 and 11 belong with existing Git compatibility:
use the actual base and require remote access only for an operation that needs it.
The [autonomous worktree demo](worktree-demo.md) passed all four host-shaped layouts;
Jack does not need to repeat these mechanical fixture steps.

**This slice:** replace unconditional `origin/<default>` base selection with the
default branch's configured Git upstream. A repository without tracking uses its
available remote default or local branch. Advertised remote HEADs outrank local
fallback conventions; existing `master` and custom primary branches no longer
silently become `main`. Refresh, sync, landing preparation, agent placement and
new Task base selection share this rule. Explicit local sync targets skip unrelated
main refresh; existing worktree reuse skips refresh entirely. No settings, provider
framework, ownership registry or background service is added.

Delete the fixed-origin selection in these paths and the unconditional refresh
before reuse. Retain explicit network errors when an operation does need to fetch,
Git's existing tracking configuration, branch identity and preserved edits. Native
publication, stacked-Task remotes and GitHub identity still contain origin assumptions;
this increment does not claim complete fork publication support.

Headless acceptance: local-only `master` creation, named `team/trunk` refresh and
creation, fork integration from `upstream/main` with unchanged origin, reuse while
origin is unavailable, and explicit local sync while origin is unavailable. Existing
worktree/sync tests and affected Task-base checks must remain green. The same CLI
layout demo passed again against the completed binary. Review finding: preserving
Git tracking as the owner avoids a second remote policy; local targets and existing
checkouts need no remote refresh. Remaining origin assumptions are explicitly
limited above rather than represented as full fork compatibility.

## Canonical Wave directory — accepted 2026-10-12

Jack Heart approved `.lf/wave/` as the single authored Wave location and migration
of this repository alongside the code. This is one indivisible path cutover:
Rust readers/writers, context, metrics, sync classification, Desktop discovery
and authoring, documentation, fixtures and the repository's Wave files move
together. Wave identities, slugs, hierarchy, Tasks and planning storage survive;
no database migration or alternate-path configuration is needed.

**Delete — do not maintain:** top-level `wave/` path construction and repository
definitions. No dual lookup, alias directory, or automatic takeover of another
repository's arbitrary `wave/` directory. Preserve historical revision links and
command/skill namespaces such as `wave/session`. Existing users move authored
Wave definitions explicitly; this change migrates this checkout's definitions.

**Implemented:** live consumers, repository files, documentation and fixtures now
use `.lf/wave/`. The read/write fixture ignores and preserves a top-level
lookalike directory. Review found and corrected metrics root resolution for
the deeper path, and retained historical revision paths while updating live links.
Headless verification: focused Rust Wave/config/context/metrics tests, Desktop
parser tests where available, Python path-consumer tests, formatting and Clippy.

## Gate review — 2026-10-12

Main integrated through `6935bfbadb` before final checks. Conflict resolution
retains current CLI/provider APIs and the canonical `.lf/wave` paths. Review
caught a newly shared Task base resolver: explicit stacked parent branches must
remain explicit while the default integration base follows Git tracking.
Scratch cleanup uses the companion implementation; no second ownership system.
The incoming three-space edit is excluded and preserved in
`/tmp/lf-how-will-lf-break-pre-ship` alongside all original scratch artifacts.

Review also caught a nested checkout lock on explicit integration sync. The CLI
now refreshes the default upstream once before entering preservation; explicit
local targets still avoid unrelated network refresh. Python checkout fixtures
exercise the unchanged single-snapshot contract.
Gate found that explicit Wave document paths still met the old blanket `.lf`
exclusion. Authored `.lf/wave` documents now participate in explicit context and
source deduplication; other `.lf` state remains excluded. Existing context-delivery
tests exercise distinct memory scopes and one copy of the Wave goal.
Wave ownership fixtures now track `.lf/wave` instead of ignoring the whole `.lf`
directory. The broad gate also exposed an existing wall-clock assertion: a
planning refresh may advance `updated_at` across a second boundary. The fixture
now checks monotonic time and preserves its identity/state comparisons; no
production clock behavior changed and no retry was added.

Check: `uv run python scripts/test_network.py cargo test --offline -p loopflow --lib` — 1,947 passed, 8 findings repaired (local GitHub merge fields, explicit Wave context, Wave fixture ignores, refresh timestamp assertion), all 8 passed in focused reruns (`stored_task_publishes_and_completes_after_verified_merge`, `context_delivery`, `wave_repository_ownership`, `cold_detail_refreshes_confirmed_work_and_retains_identity`), 10 opt-in tests ignored; affected integration targets `worktree_tests commit_tests land_tests scratch_tests sync_tests pr_review_tests wt_ci_logs_tests task_pr_authority_tests task_initialization_tests context_launch_tests context_tests context_block_tests cli_discovery dto_fixtures documented_commands` — 236 passed; golden/remote fixtures — 3 passed; post-context repair context/golden integration — 25 passed; Python suite — 410 passed/3 findings, architecture+checkout rerun 40 passed (all 3 repaired), 1 skip; website `uv run python dev.py test` — 75 passed; `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, architecture, Swift boundaries, 9-file Swift syntax, four-layout CLI demo and diff checks passed; Desktop build/tests deferred to CI (ViewInspector fetch and missing GhosttyKit). Logs: `/tmp/lf-break-*.log`. No broad rerun needed without new code or failures.

Gate is complete for this local change. Jack Heart authorized the authored ship
flow and all local commits; its remaining delivery command is
`land --wait-and-fix`. The cached PR title/body cover repository adaptation,
scratch preservation and canonical Wave migration. Further scope remains the
limits already recorded above, not a blocker to this authorized delivery.
