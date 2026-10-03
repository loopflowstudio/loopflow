# Wave and repo worktrees

Accepted, 2026-10-02. Jack accepted moving Git operations out of the Task CLI
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

## Implemented state — reconciled 2026-10-02

- The feature CLI exposes root `pr`, `wt`, `sync`, and `commit`.
  Immediate-child parsing replaces recursive owner/abbreviation expansion;
  skill/Flow discovery remains. Compression and bounded repairs are checkpointed
  at 0ec9e2fa5. Published runtime 0.12.31 is installed and the saved Flow has passed
  its previously blocked decision boundary.
- Primary scope placement, moved-worktree reuse, selected-path document commits,
  and resident scratch-preserving delivery are implemented. Existing focused
  evidence covers these boundaries; full acceptance remains at publication.
- Upkeep is explicit `lf sync`. Primary admission does not fetch on each message.
  Wave/repo owning skills describe memory publication and reuse after merge.
- Main was merged at 562764212, followed by the incoming registry-test correction
  aabec7759. Scheduled installation still emitted the removed `lf install` path;
  realignment corrects it to `lf home install` and updates its test.
- `ops/checkout.rs::with_preserved_edits` moves resolver-created collisions to
  unused `.lf-sync-N` sibling paths before restoring the original stash. Regression
  coverage exercises failed-update retry, staged/unstaged separation, and resolved
  sync through the same preservation boundary used by the CLI.
- Saved Flow command decoding already migrates removed spellings. A populated-store
  proof checks literal CLI parsing, retained identity/cursor/claim, and stable
  reserialization. Stale builtin graph and documentation expectations were corrected.

## Data and functions

Reuse `PrimaryScope::{Repository, Wave}`, `AgentSession.cwd`, `AgentWorktree`,
`CanonicalRepo`, Git branches and worktrees. No synthetic Task or new PR owner
table. Scope chooses placement; Session retains conversation identity.

Implemented `ensure_scope_worktree(repo: &Path, scope: &PrimaryScope) ->
Result<AgentWorktree>` adapts the existing placement helper. Reuse it at primary
admission/replacement and scope-operated launch boundaries. Explicit contributions
in a supplied checkout stay there; Wave attribution alone must not relocate them.

Clap immediate-child lookup replaces `resolve_child` and its path-expansion wrapper. Preserve skill/Flow discovery,
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

Implementation covers the full keystone: literal root Git commands; stable scope
workspaces and idle primary adoption; selected-path document commits; resident
scratch preservation through publication and delivery; explicit upkeep; and owning
instructions. Independent document checkouts can use `lf wt create NAME --resident`.

Compression removed the obsolete command-expansion wrapper, impossible error paths,
and duplicate help cases. Resident instructions now use the scope’s own document
example and a shorter self-contained maintenance recipe. Deletion targets above
are implemented; captured Flow command migration is covered for stored executions.

Remaining:

- Retain the human demo for Jack. PR #1412 is published and ready at
  `0ec9e2fa5609817847b9dd822e6010f57bc22c6a`; auto-merge is off. The built
  resident-aware CLI published through an isolated Home. The remote tree excludes
  scratch; all local notes remain, including concurrent recovery updates. No merge
  or demo completion is authorized.

The saved pursue Flow is now waiting at node 6 (`demo`), Session
`session_63fc75b803664b3db1bacc9c6bb0ad23`, with no failure. Its review remains open.

No owning Wave has been identified, so no Wave memory was changed. Scheduled
distribution and non-PR memory distribution remain deferred by the accepted scope.

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

Headless acceptance evidence is recorded in `scratch/checks.md`. Publication and
configured-provider resumption succeeded; Jack’s demo remains outstanding. Jack has
authorized releasing and installing the prerequisite runtime, retrying the saved
Flow, and continuing through demo readiness. See `scratch/flow-runtime-schema.md`
for that recovery. Demo feedback and completion remain Jack’s decision.
