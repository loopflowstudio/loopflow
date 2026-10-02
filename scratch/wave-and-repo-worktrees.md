# Wave and repo worktrees

Draft, 2026-10-02. Jack accepted moving Git operations out of the Task CLI
hierarchy and removing command-path shorthand. Jack authorized a background pursue Flow through demo. Scratch remains local and
must survive publishing; implement the simplest direct resident publication path.
Scheduled distribution remains deferred.

## What to build

Give Wave and repository agents persistent, maintained worktrees and an explicit
way to publish authored documents without inventing Tasks or losing local plans.

Jack's intent: “allow PRs and WTs outside tasks”; “i mean in the cli hierarchy”;
“make sure that it has a well maintained worktree and clear instructions on how
to export its wave docs”. Memory distribution need not require PRs. A daily
sync is a possibility, not an agreed schedule.

Placement: unresolved; no owning Wave was named.

## Shape and boundary

Additive series. Keystone: literal root Git commands, Task-independent resident
workspaces, and a manually invoked document PR workflow that preserves scratch.
Implement these in internal slices; the keystone is complete only when the real
agent workflow works. Follow-up: scheduled memory distribution, with its own
design for cadence, destination, authority and conflict policy. No Tasks filed.

## Current system

- `rust/loopflow/src/lf/mod.rs`: `TaskCommand` owns `Pr`, `Wt`, `Sync`, `Commit`.
  `lf/navigation.rs::descendants` and `resolve_child` expand abbreviated and
  omitted command owners. Help and definition lookup also use this resolver.
- `ops/human_session/primary.rs::ensure` and `replace` preserve scope conversation
  identity, but `repository_session` starts in the canonical checkout.
  Wave sessions use `ops/run.rs::resolve_work_selection`, which inherits a
  caller checkout in the same repository, including a Task checkout.
- `engine/worktrees.rs::ensure_agent_worktree` already creates/reuses a resident
  worktree. Its existing-reuse path does not refresh upstream. Its placement
  test rejects a resident branch moved away from its deterministic path.
- `ops/human_session/workspace.rs::associate` hides workspace data for primary
  sessions entirely. Showing a workspace must not grant Task membership.
- `ops/pr.rs` already has optional Task context and non-Task range handling;
  preserve this rather than replacing all PR storage. Its control-plane error
  tells callers to create a Linear Task.
- `ops/land.rs::clear_scratch` deletes scratch contents during delivery.
  Wave/repo session skills permit scratch edits but provide no concrete export
  workflow. Repo session instructions do not describe durable memory upkeep.

## Data and functions

Reuse `PrimaryScope::{Repository, Wave}`, `AgentSession.cwd`, `AgentWorktree`,
`CanonicalRepo`, Git branches and worktrees. No synthetic Task or new PR owner
table. Scope chooses placement; Session retains conversation identity.

Proposed `ensure_scope_worktree(repo: &Path, scope: &PrimaryScope) ->
Result<AgentWorktree>` adapts the existing placement helper. Reuse it at primary
admission/replacement and scope-operated launch boundaries. Explicit contributions
in a supplied checkout stay there; Wave attribution alone must not relocate them.

`resolve_child` becomes immediate-child lookup. Preserve skill/Flow discovery,
explicit collision escapes, read-only help and argument passthrough.

## Workspace behavior

One resident workspace per scope on a Home, reused across conversation replacement.
Locate existing Git placement even after a move; deterministic paths are creation
defaults. Preserve commits, dirty files, scratch and conflicts. Missing placement
reuses recoverable branch state; do not claim lost uncommitted files were recovered.

Upkeep fetches and integrates through existing sync coordination at an explicit
maintenance boundary, not every message. Failed network access leaves local work
usable. Conflicts remain visible and retryable. Serial placement/maintenance must
not start a second provider or interrupt an active writer. Adapt existing primaries
at a safe driver boundary without replacing their conversation or moving another
Task's files. Pruning must retain resident workspaces and unpublished work.

## Document export

Publish selected durable documents directly from the resident worktree through
`lf commit` and `lf pr publish`. Scratch remains local: never stage or push its
contents, and preserve it through commit, sync, publication and landing. Remove
already tracked scratch from the publication index without deleting local files.
Do not create a second delivery worktree or an export receipt system merely to
protect scratch. Keep unrelated dirty files and edits made after publication.
Maintain existing Task delivery behavior unless its changes are necessary for
this contract; explicitly test both resident and Task paths.

Put the executable recipe in `wave_session.md`, `repo_session.md` and the relevant
operating skill: where to write memory, what remains scratch, how to inspect the
selected document diff, commit, publish, resolve conflicts and refresh after merge.
Document how the persistent branch remains reusable after a merged PR. Honor
existing publication authorization. PR export is supported but not mandatory for
every memory update; cron and non-PR distribution are outside this increment.

## Delete — do not maintain

1. Task-owned Git command variants/dispatch; move to root and update mechanical
   Flow callers in `ops/flow.rs` in the same cut.
2. Recursive descendant/abbreviation lookup and its exclusive ambiguity tests.
   Update `cli_discovery.rs`, `user_cli_tests.rs`, module tests, generated command
   references, skills and AGENTS shorthand guidance together. Keep help tests.
3. Primary-session checkout selection from ambient Task/main; replace with scope
   placement while preserving explicit contribution behavior and Session history.
4. Resident-placement refusal test for moved branches; replace with successful reuse.
5. Task-required PR error guidance. Preserve branch safety and Task delivery claims.

## Internal slices

**This slice:** promote root commands and remove command shorthand with consumer
and documentation cutover. Next: resident admission/recovery and workspace display.
Then: document export, upkeep and agent instructions.

Forbidden: fake sync Tasks; Task ownership inferred from primary cwd; duplicate
memory stores; dropping scratch during export; resetting unpublished edits;
silently moving live drivers; retained aliases for removed command paths.

## Demo and acceptance

Open repo and Wave conversations; edit memory and scratch, replace each conversation,
and observe the same respective workspace and files. Export a memory diff, publish
and merge its PR, refresh the resident workspace, and verify scratch and edits made
after export survive. Repeat with a moved worktree and conflicting upstream memory.

Headless gate: `cargo test -p loopflow --test cli_discovery --test user_cli_tests
--test worktree_tests --test session_lifecycle_tests`; add focused primary-session
and resident-publication tests using temporary Git repositories and mocked provider effects.
Expected: literal command paths, retained definition lookup, no Task creation,
idempotent admission/publication, and preserved files across recovery. Run
`cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`.

Check: source inspection only; no production edits or test run during design.
