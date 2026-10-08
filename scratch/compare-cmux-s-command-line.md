# Desktop control on shared Work — LOO-427

**Status (October 8, 2026):** implementation authorized by Jack Heart; slice 1
implemented at `21ce4e495`, simplified at `f8d3386da`.
One PR through demo review; slices 2–5 remain. Jack selected execution-machine local operations and custom Git-ref Task sync
on October 8, superseding the planning-machine/callback proposal.
The implementation sequence below replaces the earlier combined architecture
slice. [Findings](findings.md) hold the source evidence;
[questions](questions.md) hold only unresolved product decisions.
The [comparison](../docs/reviews/terminal-command-comparison.md) retains all command
judgments and behavioral evidence.

## Accepted outcome

Jack Heart selected one repository window across machines, built on one shared
Work model, “all fromone place,” with the ability to “Delegate” a subtree to a
Machine. Machines execute that Work. They do not supply independent trees for
Desktop to merge. LOO-430–432 are canceled; their identity, arrangement and
terminal-I/O outcomes remain in this PR.

An agent can identify work, open its repository window, arrange retained panes,
read displayed output and deliberately send input to the exact pane. Closed
repositories need no window; repeated opens reuse one. Work identity, execution
location and visual placement remain distinct. Mac view control is macOS-only;
Linux gets an actionable error before launch or Work changes. Ordinary lf remains
cross-platform. No live planning migration is authorized.

## Architecture and ownership

| Fact | Authority | Derived consumers |
|---|---|---|
| Repository/Work identity, hierarchy, authored delegation | Ordinary local planning model; portable Task planning synchronized by LOO-412 | CLI resolution, repository window, scheduling/routing |
| Workflow position, checkout Machine/path and Process history | Execution Machine, independent of replicated planning | Task files/status, Session association, runtime observation |
| Window, panes, focus, retained native surfaces | Existing Desktop registry, multiplexer and surface pool | Programmatic inspection/arrangement/input |

Adapt `Placement` and existing Work routing. Do not add repository-pairing groups,
a Desktop Work database, a second layout store or a terminal emulator. Network
loss retains locally committed planning and visible pending synchronization.
LOO-411's old SSH `--repo` removal is not a veto on Work-directed routing.

LOO-406 owns the local planning lifecycle; LOO-412 supplies remote checkout/adoption
work and custom Git-ref Task synchronization; ordinary operations remain local. LOO-426 owns Work
opening; LOO-416 saved panes; LOO-387 draft preparation; LOO-415/424 relay/resume;
LOO-422 host status; LOO-402/403 Waiting/shortcuts; LOO-397 command discovery.
These boundaries are integration constraints, not separate replacement projects.

## Implementation sequence — one PR

### 1. Separate recorded checkout location from delegation — Implemented locally

`tasks.checkout_machine_id` records location beside `worktree`. The one Task draft,
`task_checkout_machine.sql`, backfills existing placement evidence and leaves
missing Machine evidence unknown. New checkouts record the preparing Machine;
creation-time Work assignment still inherits from the Project.

`task_checkouts` no longer joins `work_placements`. Status, Task-file access,
comparisons and Session workspace resolution consume recorded location. SQL
Session/Process membership now includes Machine, while explicit bindings survive.
Git read-ahead skips remote/unknown checkouts. Existing DTOs already carry an
optional Machine; wire shapes and Swift decoding need no change.
Comparisons read the validated checkout and its PR through one store, without
hydrating the Task again. Session resolution uses cwd and explicit Work; its
unused recorded-root argument is removed.

Unknown location cannot authorize local file reads or infer
membership from an unavailable path. No delegation inheritance or live move was
introduced. Later slices must edit the same migration draft.

Focused fixtures cover migration/history retention, reassignment A→B with retained
files/status/Session identity on A, creation on the preparing Machine, unknown
history, alias pagination and explicit bindings. No installed store was changed.

### 2. Shared Work identity, delegation and routing

**This slice. Depends on 1 and the common APIs from LOO-406/412.** Preserve
shared planning identity through local reads/writes, synchronization and delegation. Add the repository
root identity within the shared model, including repositories with no Tasks.
Reuse LOO-406's storage primitives rather than implementing its local lifecycle
again. Preserve connected-plan ownership and existing historical identities.

Resolve Work → effective delegation → execution checkout. Define explicit versus
inherited provenance, assignment changes and pending/conflicted synchronization. A
Machine can execute work from several repositories without rewriting its saved
default per request. Consume LOO-412's transport and LOO-406's common local writer; do not add a
callback planner or another synchronization engine. Work is one logical planning
model with synchronized local records, not one physical planning-machine store.
Retain historical execution and explicit legacy-ID mappings.

**Proof:** two machine fixtures converge on the same planning identities and
changes; delegated execution reaches the correct checkout. Network failure retains
local writes and visible pending sync; reconnection deduplicates and preserves
conflicting input. Receiving planning completion never moves a local Workflow. A delegation
edit never relabels an existing checkout or claims a process moved.

#### Mixed command: `lf task run` — local operations, planning synchronization

Jack's latest October 8 direction supersedes the designated-host experiment and
host-owned Workflow proposal. `task run` executes entirely on the selected
execution Machine using its ordinary local store: Workflow selection, departure,
Flow execution and arrival all stay there. Nested create/edit/comment/completion
use the same local planning writer; operations do not call back to another store.

LOO-406 owns local planning mutations and optional Linear sync. LOO-412 owns
portable planning exchange through a single custom Git ref, provisionally
`refs/loopflow/planning`, separate from code branches. The exact ref, selected
remote, scope and merge protocol remain in that Task. Do not duplicate them here
or publish real plan data to the public code remote.

Planning changes (identity, brief, membership, comments, completion) are locally
committed with stable mutation identity and pending synchronization. Execution
facts (Workflow position, checkout paths, Sessions, Processes, claims, signal
and cleanup authority) are excluded from the ref. Delegation metadata's exchange
scope must agree with the common schema/protocol; assignment is not process state.

Git unavailability does not make ordinary planning writes depend on a laptop:
local saves succeed with pending sync, rather than unconfirmed host callbacks.
Pending data is not remotely durable until published. Existing execution keeps
its own lifecycle; no automatic turn/Flow retry is introduced. Online comments
and completion propagate semi-live through the active sync owner. Import uses the
common local writer, preserves causal reopening and conflicts, and avoids echoes
with optional Linear sync. Receiving a comment is not a command to start a Flow.

**Required mixed-operation proofs:** local `task run` writes its own Workflow and
runs locally while Task comments/follow-ups/completion synchronize to a second
Machine. Repeat exchange creates no duplicate mutations. Disconnect permits local
planning and execution with pending sync; reconnect converges supported planning
changes. Incoming completion never advances the other Machine's captured Workflow,
signals a Process or removes a checkout; delayed completion cannot overwrite newer
reopening. Existing divergent execution IDs/history remain intact. No central
Workflow write, remote arrival acknowledgement or host-failure gate remains.

### 3. One repository window and explain/inspect

**Depends on 2.** Key the window to repository identity in the shared planning model. Cut startup,
restoration, menu opening, Task links and in-window navigation over together.
Replace `LoopflowApp`'s unaddressed workspace `WindowGroup` and the separate
`WavesView` opening path. Retain utility windows and existing native surfaces.

Replace `WorkLinkRouter.deliver`'s unrelated-window fallback and single pending
URL with per-repository pending destinations. Preserve every cold-open request;
concurrent opens of one repo converge. Planning uses the common local reader; execution observations retain Machine
and freshness. Synced planning completion and local Workflow position stay distinct.

`explain_context(selection)` returns Machine/repo/checkout/Wave/Task/Session/Process
and selection provenance, including absent/unavailable values. `inspect_desktop`
returns selected work, layout, focus, supported operations and existing Rust
legal actions. Explicit targets and checkout inference share the same resolver.

**Proof:** cold opens for two repositories arriving before registration and
registering in reverse order both reach their own windows; same-repo opens reuse
one. Plain-repo and remote-unavailable cases remain useful. JSON/Swift agree.

### 4. Arrange and interact with exact retained panes

**Depends on 3; Q3 affects Session input.** One `DesktopTarget` contains repository,
window incarnation, Machine/workspace, pane and expected content incarnation.
Validate and dispatch on the main actor; delayed requests never follow new focus
or a reused surface. Replace path-only read keys and unqualified terminal keys
with their consumers/callbacks/fixtures together.

Expose focus, shell/Files/Flow-log companions, split, move, resize, zoom, hide and
restore through `SessionsWorkspaceRegistry`, `MultiplexerStore` and
`GhosttySurfacePool`. Native surfaces/drafts survive arrangement. Ending a
shell/client is a distinct effect from hiding a view or completing Work.

Expose bounded screen, scrollback and selection reads, distinguishing empty,
hidden and unavailable. Reads claim no client. Literal insertion and key submission
are separate; no silent draft replacement. Bounded observation establishes output,
not semantic Task/turn completion. Transport/spelling are implementation choices
using existing owners; no socket or cross-version negotiation is required.

**Proof:** stale targets, focus changes, surface replacement, retained drafts,
literal text versus Enter, passive reads and limits. Linux errors precede any
app/provider/Work effect. Use owned fixture terminals and fake providers only.

### 5. Complete-path verification and demo

**Depends on 1–4.** No partial slice ships separately. Gate runs affected Rust
behavior/migration suites and
`cargo test -p loopflow --test dto_fixtures` for Rust wire fixtures, plus
`cargo test -p loopflow --test documented_commands` for command ambiguity only.
Use `scripts/test_desktop.sh -Xswiftc -gnone --no-parallel` for headless Desktop
build/tests. Include the full `session_lifecycle_tests` suite and the disposable installation
migration harness for the changed association/schema. Materialized migration and
Desktop checks remain with gate; empty filters are not proof. Clear inherited execution markers per TESTING.md.

Demo: one repository window shows two Tasks on different Machines. Explain their
identity/delegation, open them, add shell/Files panes, retain an unfinished draft,
change focus, target a harmless command, read output, hide/restore, and verify the
original input target and draft survive. Review native usability separately from
headless gate. Preserve comparison dispositions and evidence limits; no real
provider accounts or live user terminals.

Check: prior compression results retained for unchanged Rust: `cargo test -p loopflow --lib` filters `task_checkout_machine`, `human_session::workspace::tests`, `task_files_`, `task_workspace_context_covers` (13 pass, execution variables cleared), fmt and Clippy pass; realign: `git diff --check` and `lf context --skill realign --json` pass; gate owns materialized/installation migrations, full Session lifecycle and Desktop; native demo remains.
