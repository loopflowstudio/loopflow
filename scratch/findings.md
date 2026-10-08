# Findings supporting LOO-427

Dated source evidence, not additional requirements or a second plan. The
[implementation plan](compare-cmux-s-command-line.md) owns scope, sequence and
acceptance. [Open questions](questions.md) own unresolved product judgments.
The earlier implementation attempts below found the storage coupling; the October 8 cut removes it locally.

## Identity finding and existing owners

Source inspection at `7e852defe`, not a live-store experiment:

| Owner | Existing fact and missing behavior |
|---|---|
| [`CanonicalRepo` / `RepoId`](../rust/loopflow/src/repository.rs) | The first collapses local worktrees/symlinks to a Machine-local path; the second derives `owner/repo` from origin without its host. Neither establishes cross-machine correspondence. |
| [`Machine`](../rust/loopflow/src/durable.rs), [`add_machine`](../rust/loopflow/src/store/sqlite/durable.rs) | One optional remote directory, upserted by Machine ID. No local repository or pairing; adding another repository on that Machine replaces the directory. The shared repository root must also support repositories with no Tasks. |
| [`Machine dispatch`](../rust/loopflow/src/lf/commands/ssh.rs) | Uses the registered `Machine.repo` as remote cwd. Work-directed routing must resolve its delegated execution location. |
| [`PortfolioRepo`](../swift/LoopflowMac/PortfolioRepo.swift) | Local path and last-opened date only. |
| [`RegistryQueryLocal`](../swift/LoopflowMac/Services/RegistryQueryLocal.swift), [`WorkModel`](../swift/LoopflowMac/WorkModel.swift), [`SessionsView`](../swift/LoopflowMac/Views/SessionsView.swift) | Local subprocess reads and work stream, path-keyed Session readings, and a conversation-launch rejection for another Machine. Remote reading/opening still needs implementation. |
| `WorkspaceIdentity`, [`TerminalIdentity`](../swift/LoopflowMac/Services/Ghostty/GhosttyManager.swift) | Workspace identity includes Machine/worktree; terminal identity is only a Session or shell string. Carry Machine through surface lookup, callbacks and input. |

Retain `SessionsWorkspaceRegistry` for workspaces, `MultiplexerStore` for layout
and focus, `GhosttySurfacePool` for terminal views, and `RegistryQuery` for Rust
state/legal actions. AppleScript currently captures only the key window; there
is no general Desktop layout or terminal API. The comparison records the
installed lf 0.13.9 baseline separately from newer source vocabulary.

Rechecked at `7ee1247e8` on October 7: root `lf open` already rejects non-macOS
before invoking the app launcher; Work targets and terminal guidance remain
missing. `WorkLinkRouter` has one pending URL, overwritten by a second cold-start
request and consumed by the first window to register without checking repository
membership. The cutover must preserve requests for different repositories as well
as converge repeated opens of one repository. This is source evidence, not a
native concurrency trial.


#### Storage findings — October 7, 2026

`inherit_placement` copies the parent Machine at creation; `placement_in` reads
that row directly. Replacing this with nearest-ancestor lookup is unsafe:
`store/sqlite/children.rs::task_checkouts` joins the same placement row to the
Task's existing worktree, and `ops/task.rs::task_snapshot` uses its Machine to
decide whether to inspect that path locally. Changing delegation would relabel
an existing checkout as remote without moving it. Checkout-based Session
association would then use the wrong location. Authored
delegation and observed checkout location must be separated in the existing
storage owner before changing inheritance. Historical rows record no explicit
versus inherited provenance; matching a parent is not proof of inheritance.

There is also no repository Work root to use yet: `durable.rs::WorkRef` contains
only Wave, Project and Task. The current local import mints Project IDs in
`store/sqlite/planning.rs` and Task IDs in `ops/task.rs`. Shared Linear records
alone do not establish shared Loopflow identities.

## Latest LOO-406/412 direction — October 8

LOO-412 comment `f883b34b-1195-4fc5-bfc7-1ac7d4c29be6` and LOO-406 comments
`8b45e82d-6765-490a-a3d6-44b44611cedd` / `b5402659-b825-4c12-b278-5d56ae1aa518`
select one custom Git planning ref, local operations and bidirectional planning
sync. Jack confirmed this directly in LOO-427. These supersede the earlier callback
comment `e9ef3396` and LOO-427's proposed host-owned Workflow transitions.

The current LOO-412 design was read in its checkout. Each process uses its local
store. Portable planning includes Task identity/brief/membership/comments/completion;
Workflow progression, Sessions, Processes, checkout paths and execution authority
are expressly excluded. `refs/loopflow/planning` is a provisional ref name, not a
published protocol. Stable mutation IDs, causal reopening protection, conflicts,
explicit deletion and semi-live exchange remain required. No real planning-data
publication is authorized by selecting the prototype.

LOO-406 owns ordinary local planning operations and pending effects, with optional
Linear synchronization. LOO-412 owns custom-ref transport and machine integration;
its independent isolated transport tests need not await all Linear work. End-to-end
integration consumes coherent committed writer APIs, not another planner or dirty
code copied between checkouts. Source comments/earlier briefs retain superseded
callback wording; the later accepted comments and current design govern.

Current `traverse_workflow` and `workflow_arrive` write through the local store.
That location agrees with the latest decision: preserve this local execution
boundary instead of adding central Workflow callbacks. Mixed-operation proof must
show that exported planning completion cannot replay Workflow or cleanup effects
on the importing Machine.

## Decisions these findings do not establish

- Missing shared-source implementation does not reopen Jack Heart's accepted
  one-tree/delegated-subtree direction. Jack selected local planning with Git-ref exchange on October 8; implementation
  and integration are still required.
- Removing the former SSH `--repo` option did not ban Work-directed routing.
- Matching a parent's Machine is not evidence that a historical assignment was
  inherited. Migration must preserve the observed meaning rather than invent it.
- Exact pane/content identity prevents redirection, not destructive changes to
  an existing unsent draft. Session input needs the LOO-387 interaction contract.
- A successful command receipt does not prove usable rendering. The cmux
  disposable-window probe is evidence only for that host, not Desktop parity.
- `documented_commands` tests ambiguity; `dto_fixtures` tests wire shape. Neither
  replaces behavioral storage/routing/native-model tests.

Current alternatives rejected by the accepted direction: repository-pairing
registry; independent per-machine planning trees merged by Desktop; clone-name
or remote-URL matching as Work identity; rewriting Machine defaults per request.

## October 8 implementation review

Checkout location now lives on `tasks`, using the existing `TaskCheckout` reader.
Review found two paths outside that reader: SQL membership matched only paths,
and comparisons bypassed file-location validation. Both now use recorded Machine
evidence. Unknown Machine evidence previously allowed local file reads and
missing-path Session association; it now stays unavailable, preserving explicit
bindings. No new DTO or parallel placement store was needed.

The first focused run failed on fixture setup (missing required `input_published`
and a nonexistent PR branch); both fixtures were corrected. Gate still owns
materialized/installation migration and complete Session lifecycle verification.
This is slice-1 evidence, not shared-source or native Desktop proof.

Reconciled October 8 against `f8d3386da`: compression removed comparisons'
second store/Task lookup and `TaskComparison::new`; the validated checkout now
supplies comparison identity and path. Session resolution lost the unused
`recorded_root` argument and its exclusive test. Prior focused checks still
apply; this reconciliation changes no Rust.

The integrated upstream `812d8cc55` changes installation retention, not shared
Work authority. Current `WorkRef` still has only Wave/Project/Task;
`inherit_placement` still copies its parent's assignment. `WorkLinkRouter` still
overwrites a single pending URL and falls back to an unrelated window. These
findings preserve the remaining work in slices 2–5, rather than establishing any Desktop control
as complete. Product has no child directories or child memories in this checkout;
Infrastructure's Machine/reporting notes retain the existing integration owners.

## Integration boundary — October 8

Read-only inspection from LOO-427 at `eccb65253` checked both owner branches and
their current designs. LOO-406 at `b3cd894f3` has shared comment transactions,
but `PlanningAuthority`, `project_authority_on` and personal-plan creation/edit
paths remain. Its current design explicitly says the coherent writer boundary
is unfinished; the later abandonment changes are also uncommitted. The separate
Linear reopening race is not itself a prerequisite for Git transport.

LOO-412 at `2bb5ce5c4` still ships `TaskSource::accept_planning`, which imports a
provider snapshot without carrying shared Task identity. Its selected Git design
is in the working tree; `engine/planning_git.rs` is untracked, with no committed
portable export/import or common-writer integration. No dirty dependency code was
copied and neither owner's checkout was changed.

Slice 2 needs committed common local mutations with stable pending identities,
portable export/import preserving causal reopening and local execution, and Git
exchange. Repository identity/delegation must extend that shared schema. Building
Desktop identity on today's adoption path would retain the authority being
replaced; a new local writer or Git engine here would duplicate an explicit owner.
The complete outcome remains unchanged and implementation stops at this dependency.
