# Giving lf to friends: failure map

Draft, 2026-10-11. Jack Heart requested a broader search for ways lf could go
wrong before settling on more individual fixes. This is a discovery artifact,
not an implementation allocation or a claim that every scenario fails today.

**Evidence:** C = code path confirmed by source inspection; D = documented
external behavior compared with lf source; H = hypothesis requiring a probe.
C does not mean a reproduced user incident. **Response:** adapt = incidental
assumption to remove; choice = preserve explicit team/user policy; boundary =
state the unsupported capability without blocking unrelated use. Priorities are
proposed: P0 threatens work or authority; P1 blocks ordinary adoption or recovery;
P2 limits reach or polish. Classify the failure per capability, not per person.

## External worktree check — initial heuristic

The first implementation is narrower than real tools. Our inference requires
at least two live branch-bearing worktrees, one parent, and one constant prefix
before the branch leaf. Existing branch checkout reuse is separate and already
uses Git's recorded path.

| Tool / documented convention | Gap in the first prototype |
| --- | --- |
| Conductor: `~/conductor/workspaces/<repo>/<city>`; city path survives branch renames | City and branch can be unrelated: inference falls back even with a clear common worktree parent. |
| Herdr: `<configured-root>/<repo>/<slug-of-full-branch>` | `feature/foo` and `fix/bar` produce different apparent prefixes under the leaf rule. Two `feature/*` examples can falsely teach lf to prepend `feature-` to an unrelated new branch. |
| Worktrunk: default sibling `<repo>.<sanitized-full-branch>`; configurable templates | Same full-branch versus leaf problem. Hashes, codenames, nested branch paths and bare layouts exceed our model. |
| Claude Code CLI: `.claude/worktrees/<name>` on `worktree-<name>` | The filename is shorter than the branch leaf; initial suffix matching rejects it. |

Sources checked 2026-10-11 (official documentation/source; applications were not
launched): [Conductor paths](https://www.conductor.build/docs/concepts/git-worktrees),
[stable city names](https://www.conductor.build/docs/reference/cities),
[branch changes](https://www.conductor.build/docs/concepts/workspaces-and-branches),
[Herdr paths](https://herdr.dev/docs/cli-reference/#worktrees),
[Herdr slug implementation](https://github.com/herdrdev/herdr/blob/master/src/worktree.rs),
[Worktrunk templates](https://worktrunk.dev/config/),
[Claude worktrees](https://code.claude.com/docs/en/worktrees).

**Design correction proposed:** infer the containing directory separately from
filename generation. Evidence of one does not establish the other. Reusing a
host's directory also does not register a workspace in that host, execute its
setup, or grant lf cleanup ownership. Do not claim host integration from path
compatibility. Do not patch this by accumulating product-name special cases.
The current draft implements parent-directory inference; host lifecycle integration
remains outside its claim.

## P0 — preserve work and intent

| ID / evidence | Friend's situation and possible failure | Response / smallest experiment |
| --- | --- | --- |
| 01 C | Repo already uses `scratch/` for valuable files. `ops/land.rs::clear_scratch` deletes everything but `.gitkeep`, then stages all changes, unless the worktree is persistent. | adapt: stage a delivery fixture with pre-existing tracked and untracked scratch; prove only lf-owned disposable artifacts are removed. |
| 02 C | Multiple contributors or tools edit one checkout. `commit_workflow(add=true)` stages the whole tree; PR preparation invokes it for non-persistent workspaces. Unrelated work may be published. | choice: mixed staged/unstaged and unrelated files through PR preparation; observe exact committed contents. Explicit path commits already exist and must survive. |
| 03 C | Team forbids rewriting shared branches. `commit.rs::push_with_fallback` retries any failed normal push with `--force-with-lease`. | choice: divergent remote plus hook/auth rejection; record attempted operations and resulting remote history. Lease safety does not itself establish policy consent. |
| 04 H | Host or colleague owns a worktree that looks stale or merged. Pruning may remove a workspace still needed for a session, local service or review. | adapt: active foreign process, clean checkout and merged branch; prove no destructive ownership inference from location alone. Existing dirty-work protections are positive evidence, not full coverage. |
| 05 H | Repo commands need trust or the provider is sandboxed. Initialization or a repair might execute setup hooks or broaden permissions unexpectedly. | choice: trusted versus untrusted repo and denied provider action; show the exact permission boundary and preserve denial. |
| 06 H | Private code, ignored local files or credentials enter captured prompts, logs, reports or PR copy unexpectedly. | choice: synthetic secret markers only; trace inclusion, storage and publication boundaries without using real credentials. |
| 07 H | Two lf/native agents act on the same branch or one resumes after handoff. An old writer may publish stale results or cancel unrelated work. | adapt: interleave edits, attachment change, publication and cancellation; verify identity and preserved work. |
| 08 H | Process dies after a remote effect but before recording it. Retry can duplicate a PR, comment, merge request or Task. | adapt: cut execution at each effect/receipt boundary; retry and reconcile the same external object. |

## P1 — Git and repository reality

| ID / evidence | Friend's situation and possible failure | Response / smallest experiment |
| --- | --- | --- |
| 09 C | Repo uses `master`/`develop` with no `origin/HEAD`. `git::get_default_branch` returns `main`. | adapt: local-only master repo and clone without remote HEAD; plan/create from the actual intended base. |
| 10 C | Fork uses upstream for integration and origin for publication, or remote is named differently. Checkout refresh explicitly targets origin. | adapt: fork fixture with different upstream/origin commits; prove correct base and push destination separately. |
| 11 C | Offline/local-only work. `wt_create` calls `refresh_main`, which fetches origin, before creating. | adapt: no remote and unreachable remote; local work should remain possible when a valid local base exists. |
| 12 D | Conductor, Herdr, Worktrunk or Claude has already chosen a layout. Current detection misses or mislearns it. | adapt: source-derived fixtures for the four conventions above; test mixed tools and branch renames too. |
| 13 H | Bare repo, separate Git directory, linked checkout launched from a subdirectory, or symlinked checkout. Main-root assumptions may point at metadata or the wrong directory. | adapt: topology matrix through launch, placement and resume, not just path parsing. |
| 14 H | Sparse checkout, submodules or Git LFS. Checkout exists but needed content is absent or unexpectedly expanded. | adapt/boundary: run the repo's smallest real build from a newly created worktree; preserve its checkout strategy. |
| 15 C | A post-checkout hook fails. `worktree_add_inheriting` treats an existing `.git` path as successful creation despite Git's nonzero exit. | adapt: failing setup hook; distinguish checkout-created from environment-ready and retain actionable stderr. |
| 16 H | User has dirty main, unfinished rebase/merge, stash entries or a renamed branch. Automatic refresh can change the work context or stall without a usable recovery. | adapt: interrupted Git-state fixtures; preserve commits, index and every untracked file. Existing stash recovery and dirty-work tests should be reused. |

## P1 — useful work and delivery

| ID / evidence | Friend's situation and possible failure | Response / smallest experiment |
| --- | --- | --- |
| 17 H | Setup depends on ignored files, dependencies, venv, direnv, generated files, local database or selected toolchain. Fresh worktree fails before coding. | adapt: project with a setup script and ignored synthetic configuration; run its documented first command. Placement alone is insufficient. |
| 18 H | Parallel worktrees run services on the same port or share a mutable database/build cache. Isolation appears stronger than it is. | adapt/choice: launch two local services and tests; observe collisions and shared writes without inventing a universal environment manager. |
| 19 C | GitLab/Bitbucket or enterprise hosting. PR creation, stored identity and reconciliation are GitHub-specific. | boundary now / candidate adaptation: trace create, observe, update, review handoff and completion as one contract. An open-command override alone cannot satisfy it. |
| 20 C | Team needs merge commits/rebase, a merge queue, or manual merge only. `pr.rs` requests `gh pr merge --squash --auto`; submit does offer a manual boundary. | choice: explicit supported modes against differing repo policies; do not infer merge authorization from readiness. |
| 21 H | Required reviewers, CODEOWNERS, draft policy, issue prefixes, DCO or signed commits. Generated branches/commits/PRs violate team rules or trigger a hidden prompt. | choice: representative hook and protection fixtures; preserve user config and surface the actionable failure. |
| 22 H | PR spans stacked work, another contributor updates it, or a reviewer merges outside lf. State may drift or follow-through may run twice. | adapt: out-of-band merge, close, reopen and head changes; reconcile fresh evidence without replaying delivery. |
| 23 C | Malformed `.lf/config.yaml`. `load_config_or_default` discards load errors and falls back to defaults for its callers. | adapt: syntax/type errors in selected config; surface them rather than silently changing provider or policy. Audit callers before assigning full scope. |
| 24 C | Launcher user wants no planning system. This branch's `init` skill still frames onboarding as distributed control with Linear-backed Tasks. | adapt, owned by `levels-of-lf`: prove plain launch and taskless Flow without enrolling in Work. Coordinate rather than duplicate its edits. |

## P1/P2 — machine, provider and recovery

| ID / evidence | Friend's situation and possible failure | Response / smallest experiment |
| --- | --- | --- |
| 25 H | Provider installed but logged out, wrong account selected, subscription expired, API-only or enterprise gateway. Installed executable is mistaken for usable access. | adapt/boundary: fake provider rejection plus configured-account demo; never silently charge a different account. |
| 26 H | Provider upgrades its protocol, unsupported model is requested, or only an older Git is installed. Launch fails cryptically. | adapt/boundary: version and handshake fixtures with an actionable supported-version message. |
| 27 H | GUI/SSH shell has different PATH and no TTY; Git signing or credential helper opens a dialog. Headless work hangs. | adapt: scrubbed environment, closed stdin and bounded credential/signing stand-ins; distinguish missing prerequisite from agent failure. |
| 28 H | Proxy, private CA, SSO, restricted egress or API rate limits. Repeated retries hide a permanent configuration failure or duplicate work. | adapt/boundary: local transport fixtures for auth, TLS, throttling and timeout; classify retries by outcome. |
| 29 H | Read-only home/store, full disk, database lock or interrupted migration. Even low-level use may fail before showing useful recovery. | adapt: disposable storage faults; preserve prior store and make read-only diagnosis work. |
| 30 H | Laptop sleeps, terminal closes, machine reboots or SSH drops during work. User cannot distinguish paused, lost and still-running work. | adapt: interrupt each boundary; inspect and resume from a second process without guessing from stale PIDs. |
| 31 H | Two installed lf versions, upgrade during active Sessions, or restored home on another machine. Runtime/store identity may disagree. | adapt: existing installation harness, release upgrade and live-session preservation proofs. |
| 32 H | Another tool deletes or moves a worktree, or a user renames the checkout. Resume points at a missing path. | adapt: move/remove between turns; retain history and recover to a known actual checkout without overwriting other work. |
| 33 H | User runs Windows, a container, network filesystem or unusual path encoding/spaces. Platform assumptions fail. | boundary/adapt: declare supported OS first; exercise supported-platform paths and storage semantics before promising more platforms. |
| 34 H | Monorepo, many worktrees or long histories. Status/prompt assembly becomes slow or captures excessive context. | adapt: measure time, filesystem reads, network calls and prompt size on representative large fixtures. |
| 35 H | Large Flows or concurrent agents consume unexpected money, disk or API quota. No clear stop or attribution. | choice: bounded workload with cancellation, usage ownership and retained work verified. |
| 36 H | Beginner sees an internal schema/process error and cannot tell what changed or what to retry. An agent exit is mistaken for completed work. | adapt: deliberately fail install, launch, test and publication; output must identify outcome, preserved work and a concrete next action. |

## Selected priorities — 2026-10-11

Jack Heart selected 01, 03, 06, 10, 12, 13, 14, 15 and 16 as concerns and
explicitly deprioritized 02, 04, 05, 07, 08, 09 and 11. Remaining choices are
delegated to this conversation. The earlier proposed probe order below is
superseded by that selection.

This implementation pass addresses 01/03/12/15 together as repository behavior:
repository-only opt-in for destructive scratch cleanup; no implicit force-push
retry; directory inference independent of branch naming; truthful checkout-hook
failure. Investigate 06/10/13/14/16 and retain concrete unresolved boundaries.
Choose environment setup (17) only where it overlaps checkout correctness.
Do not expand into the explicitly deprioritized scenarios.

## Earlier proposed next probes

1. Preservation: foreign scratch, mixed contributions, rejected push and failing
   setup hook. These can cause damage or false success, so they outrank cosmetic
   placement improvements.
2. Ordinary Git variety: no remote/offline, master without origin/HEAD, fork with
   upstream, and the four external worktree layouts.
3. First useful action: native provider configuration, repository setup and
   interruption/resume. Exercise one real friend's repo after isolated fixtures.
4. Delivery: manual-review GitHub first; decide explicitly whether a second host
   belongs in the supported offering. Do not build a provider framework first.

The selected implementation is in [the current design](how-will-lf-break.md).
This inventory retains the original source findings as evidence; it does not
mean every listed path remains unchanged. No follow-up Tasks were filed.
Absence of a probe is unknown, not a pass.

Check: inventory derived from source and official documentation; implementation
verification is recorded in the current design. No third-party application
interoperability claim; fixture paths do not prove host lifecycle integration.
