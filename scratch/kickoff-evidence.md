# Kickoff findings — 2026-09-30

These are source observations and local probes, not configured desktop
or provider proof. The accepted direction is in `intent.md`; the implementation
plan is `growth-thoughts.md`.

## Current reconciliation

Read-only source review at `171d6f2e2`, 2026-09-30; no new behavioral test or
configured desktop/provider run. The first-cut proof recorded below is reused
as historical evidence. The working tree was clean at review start.

- Upstream `3dc89bc9a` removed the resident/listener, Wave chat surfaces, external
  bridge, and turn claim/requeue implementation. Earlier findings below about
  those paths describe the pre-removal design baseline. The surviving
  `controller/wave/journal.rs` only formats short IDs. The outbox and its
  pending/delivered methods remain; source search finds their callers only in
  store wrappers/tests, with no production dispatcher. Unit 2 must establish
  primary-owned claims and receipts after proving native turn delivery, not
  migrate a live resident consumer or revive the removed service.
- `engine::worktrees::ensure_agent_worktree` survives independently of the
  deleted Wave placement wrapper. Its create path still selects
  `agent_base_ref(..., true)`; the offline primary placement requirement remains.
- The first cut's Rust association and Swift `workspace.taskId` grouping agree.
  The registry and outer worktree selectors remain path-keyed; the old Task
  terminal owner remains. No collapse API, interaction projection, or raw Ask
  key exists yet. Remote Flow workspace readings remain explicitly unavailable
  locally; owning-Home resolution is not established by the local resolver test.
- Product memory now carries Jack's accepted participation/primary-Session
  direction and the no-Project/no-PR, same-revision access lessons. Earlier
  orphan-only binding and scratch/Changes-only browser guidance is superseded.

The plan and questions now use this baseline. Unit 1 still requires its remaining
workspace, participation and Ask implementation plus the configured desktop
demo. `uv run python scripts/test.py --ui-host` and the real-provider Ask/Flow
handoff remain unavailable in this headless run's supplied no-rendering
environment. No compilation or simulated transport result substitutes for them.

## Initial Session grouping findings (before the first cut)

`WorkspaceProjection.init` joins `SessionRecord.work` to `task.runtime.workId`.
It does not associate by checkout. `SessionFlowMembership` separately identifies
captured occurrences and must remain separate from display grouping.

`ops/task.rs::task_for_checkout` queries by the current branch. Reusing it as the
new grouping rule would not prove checkout identity. `CanonicalRepo` intentionally
collapses linked worktrees to main, so it is suitable for repository scope but
not Task grouping. Use resolved worktree roots against recorded Task placement,
on the owning Home. Keep recorded Run attribution intact.

Ordinary `lf` already attempts implicit Task binding in `commands/run.rs`, which
explains why many independently launched conversations group correctly today.
The missing behavior concerns unbound/historical/otherwise-attributed Sessions
in Task checkouts, not an assertion that every independent launch is orphaned.

## Existing terminal ownership

`SessionsWorkspaceRegistry` owns worktree workspaces per window and retains one
shared Ghostty surface pool. `SessionsWorkspace` retains its multiplexer and
Task file documents across view replacement. `SessionsView` switches between
details and terminals and optionally displays files; `TaskWorkspaceView` also
has a separate Changes/Terminal presentation and terminal store.

`MultiplexerStore.toggleZoom` changes only the zoom/focus selection.
`close` removes the pane and discards its shell launch command. Undo restores
layout but deliberately does not replay that command. `GhosttySurfacePool`
retains hidden live views; surface release destroys the process surface.

Probe compiled the current production layout/store directly, with no provider
or terminal launch:

```sh
swiftc -parse-as-library swift/Loopflow/Models/MultiplexerLayout.swift \
  swift/Loopflow/Models/MultiplexerStore.swift scratch/multiplexer-probe.swift \
  -o /tmp/loopflow-growth-multiplexer-probe
/tmp/loopflow-growth-multiplexer-probe
```

Observed exit 0:

```text
PASS: zoom/restore preserves layout, pane identity, and shell command
PASS: close removes pane and launch command; undo does not replay it
CONCLUSION: collapse must preserve panes; close/undo is not collapse
```

The probe proves model behavior only. It does not prove terminal input retention,
focus, accessibility, rendering latency, or live process survival.

## Files before execution

`task_file` and `task_save` call `file_context`, which demands an active Task PR.
Their bytes/save-recovery paths use the checkout and do not need its PR base.
Separate checkout access from diff-base selection rather than inventing a base
commit for a file read. The existing save protocol retains concurrent edits and
must remain the sole writer. `TaskFilesView` currently lists scratch and changed
paths; it is not a whole-worktree browser.

## Flow projection and source selection

`FlowGraph` carries occurrence keys, human flags, nested route paths, repeat
targets, and composed Flow parent names. `PinnedTaskFlow` adds the cursor-derived
current/completed keys and return counts. This is enough to derive the interaction
view without another authored graph. Named Flow parents can span multiple human
stages: a visible edge may be a segment of a Flow, not a separately launchable file.

Current `feature` composes `task-design`, `pursue`, and `queue`. `pursue` contains
automated implementation/review cycles and a direct interactive demo. Filtering
out non-human nodes without retaining route/repeat structure loses behavior.

Task invocation capture reads `task.worktree`; the UI catalog reads `repoPath`,
and preparation of a new Task validates against main. Saved source and captured
execution are intentionally distinct. Any Flow editor must refresh from the
execution checkout and distinguish a future definition from a pinned invocation.

## Initial primary Session lifecycle findings (historical baseline)

Provider Session identity and resume commands already exist in `run_record.rs`,
`ops/human_session.rs`, and `commands/util.rs`. Session launch locks and prepared
Run capture already exist. There is no current scope-to-primary-Session record.
Do not resurrect the removed Task/Project Session execution tables: these were
replaced by durable Work and Runs.

`GhosttyMetalView.keyDown` currently forwards control keys to the terminal.
`handleSurfaceClose` reports any child exit, including replacement or crash.
An exit notification alone cannot identify Jack's Ctrl-C reset intent. Intercept
that key only in a primary Session view and invoke an explicit replacement
operation; direct Flow/Ask Sessions and shell input retain existing semantics.

Wave chat's resident launches a fresh responder per turn. That path is distinct
from governance cadence and listener transport. Moving native desktop Wave
conversation to a primary Session can remove its dependency on the resident
responder without deleting the external channel and scheduler in the same change.
See `ask-evidence.md` for Ask completion and recovery details.

## Verification boundaries

`TESTING.md` requires both SwiftPM/Ghostty and Xcode/fallback compilation for
native view/terminal changes. Existing `SessionsInteractionTests`,
`MultiplexerStoreTests`, `WorkspaceNavigationTests`, `WorktreeWorkspaceTests`,
and `TaskFlowProofTests` are the extension points. Rust/Swift shared Session and
Flow fixtures must move together. Planning did not run their suites or launch
an external agent. The focused model probe above and checkout probe below are
the executed local evidence; neither is a configured application demonstration.

## Initial primary roles and requested handoff (historical baseline)

Jack clarified active Wave operation plus design capture, and repo onboarding plus
last-resort help. Current `wave/chat` is a selective responder, while wave/operate
and design supply the required judgment separately. `init` supports direct skills
without a Wave, but its full setup workflow should not run merely on discovery.
`ops/run.rs` Wave binding supplies canonical repo cwd and executive-loop context;
new primary roles need explicit scope context. `ensure_agent_worktree` creates or
reuses placement separately from `move_default_agent_to_worktree`, which relocates
main edits. The Wave resident helper wraps that generic placement operation; its
reuse does not require retaining the resident service. Offline startup must avoid
making its optional default-branch fetch a conversation prerequisite.

After Jack requested Task handoff, `lf wave list --json` identified Product's shared
workspace responsibility. `lf wave status product --json` returned `chapter: null`
and Tasks unavailable: “Wave product needs its first chapter”. Three retained Task
records have Projects absent from the current snapshot. This does not establish
an empty backlog. Product MEMORY records retained chapter-bearing Homes and warns
against treating a missing local chapter as permission to rotate planning.

`lf flow show task-design` returned kickoff then review-design with an interactive
review boundary; the authored YAML agrees. `feature` continues from task-design
through pursue, queue, and Task-completing landing. Select task-design for the
requested planning handoff. LOO-291's retained status has no active Task Flow and
an older provider credential blocker; it does not provide the current authored
brief or prove that this new direction belongs to that Task. No auth was changed.
No Task was filed or launched. Jack then selected kickoff in the present
conversation; the implementation plan remains in this checkout. No Task brief was
created. Chapter recovery and Task reuse are outside this local kickoff.

## Checkout identity probe

The earlier working plan referred to a checkout-root probe without retaining its
command here. This follow-up supplies a reproducible probe and a fresh result:

```sh
uv run --no-sync python scratch/checkout-root-probe.py
```

Observed exit 0 on 2026-09-30:

```text
PASS: root, subdirectory, and symlink cwd resolve to one checkout
PASS: existing main and linked checkouts resolve to distinct roots
LIMIT: no Task association, remote Home, or missing-path behavior tested
```

`engine/git.rs::worktree_root` calls `git rev-parse --show-toplevel`; the probe
uses that command and canonicalizes its result. It reads the supplied linked
checkout and existing main checkout and creates only a temporary symlink. It does
not call the Rust resolver, create a worktree, or prove nested-repository,
placement-unavailable, cache-budget, or Task grouping behavior.

## TaskSession Flow switching — source findings

Inspection on 2026-09-30; no worker was launched or interrupted. Existing test
cases cited here were read, not executed.

- `durable.rs::FlowPosition` has one captured `QueuedInvocation`, cursor, claim,
  and failure, but no pending replacement. `engine/invocation.rs::QueuedInvocation`
  already allocates identity while capturing expanded steps. Reuse that object
  for the queued target rather than retaining only its Flow name.
- `engine/execution.rs::ExecutionCursor` nests selected XOR paths. Its `finish`
  propagates child iteration increments into the parent. Loop boundaries are
  repeat edges, not explicit loop frames. A root pass number alone therefore
  cannot identify a future endpoint across nested repeats.
- `engine/flow.rs::validate_repeats` requires a preceding target in the same
  body, but does not require properly nested intervals. The existing
  `transitions::tests::overlapping_edges_can_revisit_a_completed_section` uses
  `[start, middle, inner-review→start, outer-review→middle]`. At `middle` there
  is no unique containing inner interval. The plan now names this case rather
  than assuming an arbitrary valid Flow supplies a loop stack.
- `controller/task/mod.rs::drive_task` settles a boundary and immediately claims
  the next one. `store/sqlite/durable.rs::claim_task_worker` checks invocation,
  position version, and existing claim in an immediate transaction. Deferred
  switching must participate there and in settlement/preparation; polling from
  a primary Session cannot prevent the next decider Run.
- `ops/task.rs::restart_task_async` validates the requested Flow, forces a PM
  refresh, checkpoints, stops, clears the position through `restart_task_flow`,
  then calls `launch_task_process` with the selected Flow name. The replacement
  capture is therefore later than validation. `ops/commit.rs::checkpoint_task_restart`
  also calls a push helper despite its local commit options. Simply reusing this
  sequence would add a network dependency, miss writes after the checkpoint, and
  fail to preserve an accepted target across a stop/launch interruption.
- `stop_task_worker` distinguishes live/dead/unknown process identity and does
  not treat claim release or a successful signal as proof of death. Keep that
  distinction. `store/sqlite/children.rs::restart_task_flow` compares the stopped
  position and requires no claim, but deletes the position rather than installing
  a captured successor. Extend this existing authority; a new Session-owned
  scheduler would duplicate it.

Review conclusion: serialize switch acceptance with claims, retain the request
through worker settlement, checkpoint after confirmed stop, and promote one saved
successor without an empty-position interval. The plan specifies proofs for these
changes; source inspection is not a passing implementation test. The native
provider wake/interactive-input integration remains unproved as already recorded
in Unit 2. No primary launch or live Ask demonstration occurred in this follow-up.

## First-slice placement and retained file access

Further source inspection and a local filesystem probe on 2026-09-30. No provider,
desktop or Task was launched, and no application test suite was run.

- `store/sqlite/children.rs::TASK_COLUMNS` reads Tasks through an inner join to
  `projects`; `list_tasks` also applies the planning-deletion filter. The existing
  `file_context` uses the same Task hydration and requires an active local
  `TaskPr`. Thus merely removing the PR lookup does not make file access independent
  of Project metadata. This is a structural query finding, not evidence that a live
  database currently contains orphaned Task rows. The plan chooses a minimal batch
  reading of existing Task/placement columns without Project hydration for checkout
  association and access. It does not introduce another store or change planning
  visibility rules.
- `human_session::list` returns `Result<Vec<SessionRecord>>`; boundary enumeration
  and attributed label reads can fail. `SessionsStore.reconcile` removes Sessions
  absent from its input, and `SessionsView` releases their surfaces. The desktop
  already has `PodiumReading.unavailable(lastGood:reason:)`, and
  `PodiumModel.refreshSessions` retains the previous value on failure. Preserve
  that error boundary when adding the resolver; swallowing a failed placement
  query as an empty successful inventory would destroy retained surfaces.
- `task_checkout` passes `launch: false` into existing preparation. Local
  `TaskPr` history and an opened hosted PR are different facts. Existing checkout
  restoration uses the recorded branch/base and errors if no active local PR can
  supply them. No-PR file access should not become a second checkout recovery path
  or guess missing history. New checkout preparation still uses existing PM/base
  resolution; this inquiry does not establish offline preparation.
- `TaskFileSnapshot` has content state and revision but no access reason.
  `TaskFileDocument.loadSnapshot` derives editability from `.text`; `reconcile`
  returns early for an equal revision. `canSave` is also the autosave gate. A new
  access field must reach all three paths, including already-open documents.
  Updating only initial rendering would miss an access change with identical bytes.

Reproducible probe:

```sh
uv run --no-sync python scratch/file-access-probe.py
```

Observed exit 0:

```text
PASS: replacing a regular file with an internal symlink preserves its revision
PASS: O_NOFOLLOW rejects that same readable content
PASS: a regular leaf can traverse a symlink parent; leaf metadata is insufficient
LIMIT: filesystem probe only; no Rust snapshot/save or Swift editor was exercised
```

The probe creates temporary files and symlinks, compares SHA-256 content revisions
and performs a read-only `O_NOFOLLOW` open. It does not invoke the production writer
or reproduce the native UI. The resulting plan adds an optional read-only reason
to the existing file snapshot, refreshes access independently of revision, and
retains drafts while disabling editing and autosave. Production proof must include
same-content replacement and symlink-parent cases through Rust and Swift.

Review finding: both proposed shortcuts—hydrating every checkout through a full
Task, and treating equal content hashes as equal file state—would retain hidden
dependencies that contradict the selected experience. The revised first slice
removes those dependencies at their existing readers and editor owner. Unit 2's
native automatic-turn integration remains unproved; this headless run has no
rendering environment for the required draft-input/terminal demonstration.

## Desktop consumers of checkout and file readings

Source inspection, 2026-09-30. No application tests or rendering were run.

- `TaskFilesStore.refresh` always calls `taskChanges`; `TaskFilesView` builds
  its navigator from `changes.scratch` and `changes.files`. The initial load,
  ten-second refresh, filesystem invalidation and manual-save completion all
  reach that method. Rust `task_changes` still needs the PR base. Removing the
  PR prerequisite from `task_file` and `task_save` therefore leaves the current
  desktop with no file rows and a refresh error. The plan now makes directory
  readings independent of optional Changes readings through those consumers.
- `TaskFilesView.content` shows a progress indicator whenever Diff is selected
  and `store.diff` is absent; `loadDiff` returns without a base. A missing base
  needs a visible unavailable comparison, not an indefinite loading state.
- `SessionsWorkspaceRegistry` shares one `GhosttySurfacePool` across its
  workspaces. That pool uses `TerminalIdentity`, and `release` destroys the
  native surface. `SessionsView` currently releases Sessions absent from the
  repository inventory. Preserve that inventory boundary when grouping by
  workspace: a group-to-group move cannot be treated as Session removal.

Review result: retain the existing owners, separate directory refresh from PR
comparison refresh, and keep terminal identity independent of location grouping.
The design now names no-PR browser/save/refresh and live reassociation proofs in
the existing `TaskFilesTests` and `WorktreeWorkspaceTests` owners. These are
required implementation proofs, not passes from this source inspection.

## First internal cut — implementation evidence

Implementation on 2026-09-30, following Jack Heart's accepted design. This is an
internal cut of Unit 1, not a separately shippable unit or a configured desktop demo.

- `SessionRecord.workspace` is derived from a batch SQLite reading of Task and
  Work-placement rows. The resolver caches Git checkout roots, retains recorded
  missing-checkout evidence, and leaves ambiguous or unplaced ownership explicit.
  List, open, preparation, and completion use it. Remote Flow readings retain
  their recorded Home/Task with an unavailable resolution; this Home never resolves
  their paths locally. Swift grouping consumes workspace Task identity and keeps
  recorded Run attribution and Flow membership intact.
- File reads/saves use the same checkout reading without hydrating Projects or
  requiring an active PR. Comparison data remains separate. The `task files`
  command pages immediate children and batches Git ignore classification.
- The native navigator reads directory pages independently of optional Changes,
  including invalidation and post-save refresh. An associated Session exposes
  file access even when its Task is absent from the planning outline.
- Access is refreshed independently of content revisions. Internal symlinks and
  symlink parents are readable but not editable; changing access retains the draft,
  selection and Undo while disabling manual Save and canceling queued autosave.
  The existing no-follow, revisioned writer is unchanged.

Executed focused proof:

- `cargo test -p loopflow --lib ops::human_session::`: 21 passed, including
  source association and unavailable-placement snapshot propagation.
- `cargo test -p loopflow --lib task_files`: 6 passed, including pagination beyond
  500 entries, ignore visibility, outside-link rejection, same-content symlink
  access, no-Project/no-PR read/save/list, and stale-save rejection.
- `swift test --package-path swift --no-parallel --filter TaskFilesTests`:
  18 passed. Includes no-PR directory/save/filesystem refresh, independent Changes
  errors, page reset with retained drafts, and both leaf/parent symlink save gates.
- `swift test --package-path swift --no-parallel --filter 'TaskFilesTests|WorkspaceNavigationTests'`:
  41 passed before the final added file cases; navigation proved location grouping
  with conflicting attribution and retained independent Flow membership.

Swift transport is simulated. The tests exercise native documents, observation,
views and retained surfaces; they do not connect the production Rust CLI to a real
provider or prove the opening desktop demo. One intermediate Swift build rejected
source edits during compilation; the subsequent stable-input proof passed. The
missing-Project fixture initially failed foreign-key setup; disabling constraints
only in that isolated fixture established the intended retained-row condition.
A DTO build also caught the new non-JSON cursor output formatting error; it was fixed.

Review findings fixed in this cut: PR comparison failure no longer clears file
navigation, same revision no longer hides changed access, and `.gitignore` changes
invalidate affected loaded directory readings. Final CLI parser proof passed (one
case); the Rust DTO fixture suite passed (10 cases), and
`cargo clippy --all-targets -- -D warnings` passed. `cargo fmt` and diff whitespace
checks passed. The Xcode fallback proof also passed:

```sh
cd swift
xcodegen generate
xcodebuild build-for-testing -project LoopflowSwift.xcodeproj -scheme LoopflowMac \
  -destination platform=macOS -derivedDataPath .build/xcode-derived-data -jobs 4 \
  -disableAutomaticPackageResolution CODE_SIGNING_ALLOWED=YES \
  CODE_SIGNING_REQUIRED=YES CODE_SIGN_STYLE=Manual CODE_SIGN_IDENTITY=- DEVELOPMENT_TEAM=
```

This executes the `--loopflow` suite's build commands directly without broadening
this implementation pass to the affected-suite gate. It compiled the fallback app
and test targets; it did not execute hosted UI tests.

Compression of this cut keeps directory enumeration dependent only on the
checkout path and Task grouping dependent only on Task identity. Session opening
shares its surface dispatch with other readers; workspace association still runs
after boundary preparation/resume. Missing-checkout ambiguity needs only the first
two candidates. The file view now owns directory refresh in one task, independent
of comparison selection and polling. This avoids duplicate root reads and preserves
directory pagination when the comparison base changes. Access updates remain at reconciliation
and save entry points, before revision shortcuts; the nested loader no longer
repeats the update.

Focused compression proof passed: 21 Session tests, 6 file tests, and 42 Swift
tests across `TaskFilesTests|WorkspaceNavigationTests`, using the same commands
above. Formatting, Clippy with warnings denied, and the Xcode fallback
build-for-testing passed again. These are local model,
simulated-transport, and compilation proofs; the configured desktop/provider gap
below remains open.

Remaining Unit 1 implementation: Home/worktree-keyed retained workspace ownership,
collapse and focus/restore, reassociation without surface destruction, replacement
of the old Task terminal owner, participatory graph/navigation and indications,
raw Ask keys, and the configured desktop/provider walkthrough. Do not publish this
internal cut as completion of Unit 1. The headless run has no rendering environment;
`uv run python scripts/test.py --ui-host` and the configured live Ask/Flow handoff
require the maintained desktop host. No hosted UI or live provider proof was attempted.


Next-cut source finding: `TaskWorkspaceSnapshot` and `WorktreeLayoutStore` carry
paths without Home identity. The retained-workspace cut must propagate that
identity through those existing readers/selections too; a dictionary-only change
would leave prepared Tasks and generic shell navigation ambiguous. The plan now
names this dependency. The unintegrated collapse draft was removed; production
code remains at the verified first-cut checkpoint. No new worktree or worker was
created, and no PR was published.

Required configured proof remains blocked by the run's supplied lack of a
rendering environment: `uv run python scripts/test.py --ui-host` and the opening
real-provider Ask/Flow handoff must run on the configured desktop host. The
implement skill requires stopping dependent work when required proof cannot run.
The completed compilation and fixture proofs above do not release this boundary
or make Unit 1 ready to ship.
