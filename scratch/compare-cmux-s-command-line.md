# Desktop control on shared Work — LOO-427

**Status (October 10, 2026):** Jack Heart authorized implementation through demo
review, in one PR. Recorded checkout location, prototype delegation/routing,
repository-keyed windows, exact retained-pane arrangement and text/key input are
implemented locally. `9cc0707b0`/`92d95edfe` add owner observation and shared
Task/Desktop location explanations; `faebb042e`–`5d55957c6` compose remote Task
links through the existing Session/Files owners. Passive reads use the published, checksum-verified lf3
fixed-buffer reader with exact pane/surface validation; native proof remains.
Native acceptance and delivery remain.

LOO-418/#1499 is integrated; completion/checkout previews and creation receipts are reconciled.
LOO-406/#1503 and LOO-412 through `96f714bd6` are composed locally;
`1ece5cc52` reconciles transitive ancestry. Section 2 retains remaining composition.
Completion/reopening, delegation exchange and remote companions are composed locally.
Work-centered tabs are accepted but unimplemented (THIS SLICE below). Exclusive admission, mixed-provider exchange, broader explanation and configured/native proof remain. Mixed Linear/Git
stays disabled; native proof includes request-scoped Task/Session-plus-Changes opening.
[Findings](findings.md), [questions](questions.md) and the
[comparison](../docs/reviews/terminal-command-comparison.md) retain evidence and open choices.

## Accepted outcome

Jack Heart selected an agent-facing software engineering CLI: Work leads,
Sessions/panes support it, and orchestra language stays top-level. The accepted
demo includes CLI-driven Session-plus-diff opening with usable-target results. Execution on the Mini and presentation on the Mac remain separate.
Started Tasks stay on their Machine across later runs and resumes; delegation
applies to open, unstarted and future Tasks only. No Task transfer is introduced.

Jack Heart selected one repository window across machines, built on one shared
Work model, “all fromone place,” with the ability to “Delegate” a subtree to a
Machine. Machines execute that Work. They do not supply independent trees for
Desktop to merge. LOO-430–432 are canceled; their identity, arrangement and
terminal-I/O outcomes remain in this PR.

Agents identify Work, reuse its repository window, arrange retained panes, read
output and deliberately target input. Closed repositories need no window. Work,
execution and visual placement remain distinct. Desktop is macOS-only; Linux refuses
before launch/Work mutation with a terminal alternative. Ordinary lf stays cross-platform.
No live planning migration is authorized.

## Work tabs — accepted October 10; THIS SLICE

**Build:** one retained workspace per repository, Wave or Task, shown as a tab
named for that Work inside its repository window. Jack Heart accepted this internal LOO-427 slice in one PR, not its native experience. This section supersedes checkout-led presentation below;
existing execution/location safeguards remain. Admission is independent and
unresolved, not a prerequisite for these tabs.

**October 10 correction:** Jack Heart selected Split → shell, with no empty-pane
option (comment `ff61b6bc-f40c-4545-a203-82ced3afd335`). Implemented through the
existing shell-location owner for CLI, shortcuts and pane menus; missing remote
location refuses before layout mutation. Work-keyed tabs, Flow-log query routing and exact-target DTO
cutover remain unimplemented. The outer layout and its inspection DTOs are deleted. Files now resolves remote content directly, without link priming.

**Completed prerequisites:** Session/shell and Files locations are content-owned;
local Files retains its opening placement through missing planning and Close/Undo.
Remote Files owns observation and transport reuse. Bare Task navigation acquires
no Session; explicit Session/Changes still does. Evidence and failed attempts:
[findings](findings.md#content-location-cuts--october-10). These prerequisites are
not Work-tab completion or native acceptance. The remaining implementation is the registry/DTO and navigation cut below,
not another preparatory location refactor.

### Experience and boundaries

The sidebar is durable Work organization; the tab strip is the open working set;
the pane tree is the contents of one Work. Repository, Wave and Task tabs use
stable Work identity, never a title, Machine or checkout path. Names update from
planning without replacing tabs. A Session remains a durable conversation, not
a tab or workspace. Several conversations can occupy panes in the same Task;
conversation names and rename actions belong on those panes. Machine labels
explain contents' execution locations, not a second navigation hierarchy.

Opening Work selects/reuses its tab, or reveals its retained hidden workspace.
It does not by itself ensure a Session, prepare a checkout, run a Workflow or
acquire a client. New conversation and explicit Session links keep their existing
launch/connect semantics. A Task without placement opens its description/Workflow
and an empty workspace with the existing explicit preparation actions. Repository
and Wave details remain accessible in their workspace; no new planning-pane type
or replacement plan reader is required. Session/Changes links open the Work tab
and requested panes together, preserving request-scoped readiness/error reporting.

Closing a tab removes it from the open set only. Keep its multiplexer, selected
pane, zoom, file documents/Undo, unsent terminal input and native surfaces alive;
reopening restores them without replay. Switches revoke hidden native input focus
without releasing clients. Closing the last tab leaves the sidebar and an empty
working area, not a newly launched repository conversation. Window close/app quit
retain their existing lifecycle: this is not a promise to preserve live PTYs or
unsent provider drafts across process exit. Saved layout integration belongs to
LOO-416, not a second persistence system here.

### Current owners → minimal cutover

October 10 deletion cut: `WorktreeLayout`/store/view, checkout picker and
`showsRetainedTerminals` are removed, including Rust/Swift outer-layout inspection.
Navigation now renders the selected contents through the existing multiplexer;
New Session selects repository content rather than an alternate checkout route.
The registry still keys workspaces by `WorkspaceIdentity`; Flow log still ignores
its content location. This is an internal deletion checkpoint, not implemented
Work tabs. Rekeying, tab retention/navigation and exact-target DTOs remain together.
The October 9 review is historical:
`scratch/pr-review.html` (retained local artifact, not tracked in Git). Its stopped-pursuit account, mutation-wide
inspection replies and pre-caller-inference examples are superseded; it is not
the current demo or a reason to defer the independent tab implementation.

Registry remains checkout-keyed.
`retainedPane` still requires content location to equal the enclosing checkout;
`reconcileMembership` removes Sessions by path change, not Work membership.
Both must change with the registry/DTO cutover, not survive as Work-tab rules.
Files consumes content-owned `TaskLocationObservation` through the pinned query;
first open no longer depends on `prepareLinkedFiles` (deleted). Session-only links
retain the owner without allocating Files. `d86e32d466` reuses retained transports
for same-owner refreshes; changed owners refuse rather than redirect drafts.
`b295ffba2f` makes `TaskFilesStore` own remote observation and await one cleanup
path on disconnect, stale frames or owner loss; release cancels observation.
Tab hide must retain that store, not cancel it or recreate its transport. Local Files now retains its opening location through layout/Undo and missing planning; its CLI reads still use Task-owned placement. Flow log still ignores content location and reads through the window model. Finish Flow-log routing with the Work-key cutover. Empty-pane
shell creation also still uses checkout identity; bare unplaced Work cannot inherit
that fallback. Preserve explicit preparation without making navigation acquire it.
Owner inventory: `31ec76b01:scratch/compare-cmux-s-command-line.md`, this heading.

**Proposed API:**

`WorkWorkspaceKey(repositoryID, subject)` uses the resolved repository plan ID
and a repository/Wave/Task subject with a canonical stable ID. Adapt the existing
Work key where appropriate; do not add a second Work resolver. Planning Task IDs
and durable runtime IDs must converge before registry lookup, including remote
correspondence. A repository subject is explicit, not the current nil-selection
accident. The scene's retained locator remains distinct from this identity.

Reshape `SessionsWorkspaceRegistry` into the Work-keyed owner, with ordered
`openWork`, optional `selectedWork`, and retained workspace entries. These are
presentation references to the existing entries, not another layout store.
`openWork(key)` creates at most one entry and selects/reveals it;
`selectWork(key)` restores that entry's view state; `hideWork(key)` removes only
its open reference. Keep per-Work detail visibility and selected conversation
with the workspace; derive global breadcrumb/actions from the selected entry,
not another independent selected-Session variable. Titles come from Work readings;
retain last-known titles with truthful unavailable state if planning is missing.

Finish removing execution location from workspace identity. Session/shell contents
retain Machine/path and the exact recorded Task/repository observation for shell construction. Files/log panes retain their Task/location query. Reuse `WorkspaceIdentity`
as a location value, not a tab key, and existing `TaskLocationObservation`,
`TerminalIdentity` and pinned query owners. Shell exit callbacks locate the exact
pane across retained entries; they must not close a different same-path Work.
Pane moves stay within a workspace and existing execution compatibility rules;
no cross-Work transfer, new Machine assignment or arbitrary remote repository/Wave
shell launch is introduced. Unavailable location stays explicit.

A Session can have several shared memberships. Explicitly selected Work is the
presentation context if membership permits; otherwise use the shared subject
projection. This creates no new bind. One native surface must never be mounted in
two Work tabs at once: reveal its retained occurrence when already displayed;
for an explicitly different eligible Work, relocate that presentation occurrence
without restarting the client and remove the old occurrence. Keep this reversible
presentation choice in questions.md; membership itself remains unchanged.

`openSession(record)` and `openLinkedSession(destination)` first resolve Work,
then insert/reveal the named conversation in that workspace. Async preparation
captures Work key, pane occurrence and request generation; completion may update
that entry's readiness but cannot select a newer tab or steal its responder.
Opening another Work cannot cancel or discard the first Work's draft.

### Delete — do not maintain in this slice

Deleted: outer `WorktreeLayout`/store/view, registry `layouts`/`inspectLayouts`,
checkout-picker/split-slot controls and `showsRetainedTerminals`.
`WorkspaceIdentity.swift` retains location only. Exclusive outer-slot assertions
are removed; native lifetime, shell and multiplexer coverage survives. Add
Work-tab retention assertions with the pending rekey; do not restore the outer tree.

Delete Session-as-page/global-selection branches and ordinary Session hierarchy
rows where they compete with Work tabs. Keep Session discovery through Work's
conversation actions, Waiting and diagnostic orphan inventory. Keep sidebar
started/history filtering and current legal actions. No Session tab names, no
Machine tabs, no parallel “all terminals” workspace. Render Work details and its
panes from one selected entry rather than maintaining both old and new screens.

Cut over `Models/DesktopInspection.swift` inspections and Rust
`lf/commands/desktop.rs` plus their DTO owners/fixtures in the same slice:
expose Work workspace identity, open/hidden state,
selected Work and exact pane tree; move location facts to contents. Internal
`DesktopPaneTarget` must address Work workspace plus window/content/surface
lifetimes, not path alone. A receipt names the affected Work/pane, never readiness.
Existing `--repo`/`--wave`/`--task` scope and `--pane` resolve through this snapshot;
explicit pane, PTY-validated caller, then one eligible pane remains the order.
No new public workspace-ID selector, focus fallback or cross-Work ambiguity guess.
Update `docs/lf.md`, `swift/README.md` and the comparison's presentation rows at
implementation cutover; preserve behavioral evidence and external-host judgments.
LOO-397 owns richer CLI-build/running-app capability discovery; LOO-452 owns richer
key chords including Ctrl-C. Neither is part of this slice.

### Internal implementation order

1. **THIS SLICE — Work workspace cutover.** Rekey the retained registry and
   location-bearing contents; add Work tabs after the outer-layout deletion;
   route sidebar, palette, links and companions through it. Update exact-target
   inspection/DTOs together. Outer-layout deletion is checkpointed as `5727532b1d`.
   No temporary dual layout authority or tab bar over checkout identities.
2. Exercise retention, stale targeting and composed opening through the changed
   owners; adapt existing headless fixtures and docs. This is the same UI slice,
   not permission to postpone its deletion or wire consumers to another PR.
3. Demonstrate the rendered Work/tab/pane experience at the existing demo review
   boundary. Native judgment remains separate.

### Checks and demo acceptance

Extend `WorkNavigationTests`, `WorktreeWorkspaceTests`, `WorkDestinationTests`,
`DesktopPaneControlTests` and DTO fixtures to prove:

- Repo, Wave and two Tasks open separate Work tabs even when paths match;
  repeated/concurrent opens and aliases converge, renames do not duplicate.
- Several named conversations share one Task tab; Work selection restores its
  focused pane/layout. A retained Session is never mounted twice.
- Tab hide/reopen retains exact layout, pane/content IDs, document draft/Undo,
  zoom and surface references; last-tab close launches nothing. Partial Session
  inventories and missing planning retain panes rather than proving absence.
- Local/remote Task tabs share one repository window, never rewrite Machine
  defaults, and keep peer paths out of local file/shell operations.
- Delayed opens, text, keys and companion replies after switching/hiding tabs
  remain attached to captured Work/content; replacement refuses stale targets.
  Hidden native surfaces cannot receive ordinary focus-based typing. Passive
  inspection starts nothing; an unplaced Task tab requires no first-start claim.

Focused implementation command:
`scripts/test_desktop.sh --filter 'WorkNavigationTests|WorktreeWorkspaceTests|WorkDestinationTests|DesktopPaneControlTests|DTOFixtureTests' -Xswiftc -gnone --no-parallel`.
Gate adds affected Rust Desktop/selection tests, `cargo test -p loopflow --test
dto_fixtures`, documented-command checks and existing §5 suites; expect nonempty
passing tests with Rust/Swift wire agreement. These checks are headless, not proof
of provider drafts or mounted native usability.

Demo uses owned fixture Work and shell stand-ins, no accounts/live user sessions:
open repository, Wave and two Task tabs (one recorded remote fixture); reopen a
Task to show reuse; name two conversation panes; add Files/Changes and shell;
retain an unsaved file edit and unsent terminal text; arrange/zoom, switch Work,
hide/reopen the tab, and observe the same content and focus. Target literal text
then separate Enter while another tab is selected; only the captured pane changes.
Show unknown/unavailable remote state without local fallback, then close all tabs
without stopping the owned children. Native replies and visible draft retention,
not command success alone, establish acceptance. Clean up owned fixture processes
explicitly afterward. No rendering or demo acceptance is claimed by this design.

## Command map — accepted October 8; remaining composition

Jack Heart authorized implementation through demo review, not merge. The current
command tree and examples belong in [the CLI guide](../docs/lf.md#discover-commands)
and generated reference, not a second scratch specification. The complete reviewed
map, flag table, repository-lookup decisions and prior fixture references remain at
`26593ce39f:scratch/compare-cmux-s-command-line.md`, **Command map — October 8 review**.
The [comparison](../docs/reviews/terminal-command-comparison.md) retains behavioral
judgments and external-host evidence. LOO-397 owns deeper command discovery.

Preserve the implemented cut during the Work-tab replacement:

- Wave owns current/historical planning, Workflow selection and chapter editing;
  internal Project IDs, KRs and captured Workflows survive. Root Project/roadmap
  and duplicate run dispatch stay deleted. Explicit skill/Flow launches do not
  advance a Task Workflow. A separate cross-Wave backlog surface is optional.
- History owns the chronological feed, Process detail/replay and aggregate usage,
  reusing existing records and retaining capture IDs, input/results, filters and
  paging. Replay starts new execution; native history stays under Session.
- Desktop open/list own opening/inspection; Session uses connect. Bare open is
  ambiguous with PR opening. Exact commands win; ambiguous shortcuts have no
  effects. Skill/Flow shorthand collisions remain open. Discord is unchanged.
- Repository/execution selection, Work selection, context inputs, agent/account
  selection and execution settings remain independent. Shared selection resolves
  explicit and inferred subjects; contradictions never silently retarget.
  Max-turns/no-loopflow stay deleted. Steer cursors filter each node's repeated
  input, not later drivers: retain
  `a_repeated_node_receives_only_task_direction_newer_than_its_last_run`'s
  inclusion/omission/later-direction assertions. Its earlier inspection was not
  a test run (`859c414d1`, **Flags: three independent concerns**).

### Effect-free preview

`--context` shares launch assembly and Work enrichment; non-agent commands refuse
it before effects. Flow preview shares compilation/native arguments and exposes
only initial skill/router input. Future/repeated input stays unavailable, and no
predecessor executes. Main's October 10 refreshable context supersedes frozen
input and budget gates: checkout Wave/ancestor Markdown and scratch refresh;
Work direction remains a launch reference. Preview shows current system/first-turn
input and reference contents, never promises future hook bytes. Task workspace
input lists quoted paths and change states, not a duplicate per-file hash/size
projection. Patch bounds and captured-reference receipts are unchanged. Docs selects
references, not documentation generation; account/permission behavior is unchanged.

`--explain` reads invocation selection/actions before admission or preparation,
including JSON and explicitly addressed Machines. It creates no Work/checkouts,
claims, sync, login, publication, logs or agents. Missing facts stay unavailable;
execution re-resolves them. Help describes syntax, not current eligibility.
Broader action explanation remains in §3; complete-path checks remain in §5.

### Machine-local repository lookup

`--repo NAME` resolves under Machine-local `repo_root` (default `~/src`); explicit
paths retain the selected checkout and existing common-directory identity.
Explicit Machine routing forwards the selector unchanged; automatic Task routing
uses repository plan ID. Opaque `--repository ID` is identity, not a path. No
remembered-name registry, scanning, registration, checkout creation, changed
Machine defaults or implicit plan joining/publication is added. Remembered names
remain Jack Heart's open choice; reversible path rules remain in questions.md.
Scoped full Task/parent IDs and repository checks survive dispatch. Read-only
lookup distinguishes absence from unreadability. Configured remote execution and
the composed usable endpoint remain unproved by lookup/wire fixtures.

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

LOO-406 owns the local planning lifecycle; LOO-412 supplies custom Git-ref Task
synchronization. Its started-Task transfer is excluded; remote launch composition
remains in LOO-427, using existing Task preparation. Ordinary operations remain local.
LOO-426 owns Work opening; LOO-416 saved panes; LOO-387 draft preparation; LOO-415/424 relay/resume;
LOO-422 host status; LOO-402/403 Waiting/shortcuts; LOO-397 command discovery.
Reuse these owners.

## Delete — do not maintain

`42990dea3` completed both remaining deletions: the stale October 8 walkthrough
and the duplicate sharing query. The reviewed simulation remains at
`4c43550c2:scratch/design-walkthrough.html`, not as current status or native acceptance.
Routing and transactional preparation share `repositories::task_is_shared_in`;
both first-start refusal boundaries remain pending an accepted admission owner.

- Keep `TaskSource`, started-Task transfer, copied child assignments and remote callback
  planning deleted. Shared planning never supplies checkout, Workflow or control authority.
- Keep duplicate run/Project dispatch, preparation-before-preview, WAL-writing fallback,
  separate Files priming/Changes branches and path-only peer keys deleted. Task placement
  owns branch/base without requiring a PR; common receipts retain historical creation input.
- Keep scalar/list replay separate from accepted causal frontiers; retained/rejected heads
  never parent saves. Correspondence, provider savepoints, original journals and private
  holds survive. Retention and identity cannot reserve first start.
- Remote companion composition removes the local-only shell refusal and disabled remote
  observation. Existing multiplexer/documents retain drafts and native lifetimes. SSH carries
  script arguments, leaving stdin for drafts or shells; peer paths never become local cwd,
  watchers or recovery-folder opens. File readers reuse the owned-line transport, not another
  Work stream, planning poller or layout store.
- Public `--target`, window/content/surface tokens and destination/toward JSON are
  deleted, without aliases. Pane selectors capture internal tokens from one inspection.
  `ResolvedPane` retains the inspected workspace; the second window/workspace lookup
  for move/resize peers is deleted. Remote URLs reuse the validated owner/repository pair.

## Implementation sequence — one PR

### Latest direction — October 9

Jack Heart's comments `0631ef7e`, `dfe4bc38`, `95ebc58c` and `22d93d69`:
finish/checkpoint the active remote companion edit, then simplify Desktop selectors,
then finish exclusive first-start admission. October 10 selects the independent Work-tab UI slice above while admission authority remains unresolved. Existing delegation exchange and recorded-owner
observation must not be redone. Stacking on LOO-412 is authorized if necessary: inspect
its committed API/integration state and use supported lf operations, never dirty peer code.

Remote companions (`7aa8a5eb5`) and selectors (`2c5610ca4`/`1f0f64150`) are
checkpointed. Work resolution plus one Desktop inspection captures lifetime tokens;
older feedback naming these as unimplemented is superseded. No stack/sync or
dirty-checkout edit occurred.

#### Admission authority — unresolved October 9

`task_location::resolve` and `require_task_planning` still refuse shared first
start. The shared membership predicate removes duplicated SQL, not either
boundary. Local-only work, planning saves/acquisition and retained execution
remain usable. Jack Heart's requested delegated-start outcome is unfinished.

**Contrary evidence:** `4c43550c2`'s `planning_git_tests` shows divergent revisions
both confirming after reconciliation, and independent clones concurrently
confirming conflicting delegation through different refs. These are transport
counterexamples, not Task allocation or running-Flow proof. LOO-412 `a60d5594a`
has identical transport code; stacking supplies no missing arbitration operation.

Local locks/transactions and negative peer readings cannot reserve cross-Machine
execution. Choosing among visible destinations also fails: Machines can see
unequal sets, or an older claimant can disconnect before delegation changes.
A late conflict cannot undo a running Flow; exchanging the winner as planning
would violate the accepted execution boundary.

**Unreviewed proposal:** pin one admission Machine/key for shared Task identity,
independent of delegation, repository locators and planning destinations. Its
existing store serializes an explicit admission operation and retains the winner
before checkout preparation. Execution stays on the winning Machine; import
copies no claims and starts nothing. Lost replies read the same decision;
unavailability cannot elect a replacement.

Review must settle who establishes that immutable binding, how historical and
explicitly associated identities converge on it, and unavailable-Machine recovery,
or choose another single arbitration domain. No current receipt, registry or
destination selection supplies that authority. Silently choosing one adds a
permanent availability dependency, not a reversible default.

**Required proof:** two independent Machines allocate/start exactly once; retries
and disconnects retain the winner; later delegation cannot move it; repeat import
cannot replay execution. No runtime replication, Task transfer, duplicate Flow,
publication or merge is authorized.

### 2. Shared Work identity, delegation and routing

**Locally composed, incomplete.** LOO-406's common writer and LOO-412 through
`96f714bd6` supply exchange, foreground sync and repository Work-stream receipts.
Peers use Machine/repository keys. `repo identity --bind ID` associates established
roots, preserving prior local locators, Work, mappings, effects and execution;
it selects/publishes nothing. Imports retain authored delegation without creating
Machine connections; unassigned Waves retain inheritance. Independent-root
and provider fixtures prove retained identity/lookup, not running controls or
exclusive start. History: `28fc5274a`, this section.

Known started Tasks route by recorded checkout Machine; locally unstarted Tasks
use effective delegation. Neither changes Machine defaults. LOO-412's TaskSource
transfer/preparing selector stays excluded: cold planning acquisition and ordinary
Task preparation survive, but remote first start still needs exclusive admission.

Git-selected Tasks, including user-keyed refs, without retained checkouts refuse
first start before Git preparation and at placement. Local-only work, saves,
acquisition and retained execution remain usable. Local absence, negative peer
readings and SQLite transactions provide no cross-Machine reservation.
Jack Heart accepted nearest-ancestor inheritance with narrower overrides (Q2, October 9).

**Retained identity/routing composition:** common-store correspondence/full-ID
lookup preserves physical IDs, local locators, journals and execution in one
snapshot, never a reservation. `PlanningKind::fields()` exchanges grouped authored
delegation, not RepositoryId; unknown Machines create no connections. Association
is explicit, not inferred from names/remotes. One selected repository ID serves
all destinations.

Live-window association on open/restore keeps the scene/queue/receiver and renews
its targeting token; delayed identity reads cannot roll it back. Owner observation
uses one snapshot, request nonce and a sequential 20-second deadline per added
peer. One positive overrides delegation despite unavailable peers; conflicting
positives refuse, negative evidence never admits start. Remote links retain the
local repository scene; Session/Files/shell transport pins the owner's Task and
checkout, preserving drafts and stdin. No local path fallback or automatic takeover.
Detailed APIs, fixture limits and prior status:
`e4a324636:scratch/compare-cmux-s-command-line.md`, **Remaining identity/routing
composition (October 9)**. Headless proofs establish neither automatic background
identity refresh, configured SSH nor mounted native surfaces.

**Remaining implementation:** exclusive cross-Machine admission must precede
first-start allocation; observation is not a reservation. The existing refusal is
an incomplete delegated-start path, not acceptance. Mixed Linear/Git exchange,
per-Work sharing/recovery presentation remain. Remote companions have focused proof, not native acceptance. Running-Flow/repeat-exchange composition still needs proof;
retained-checkout opening does not close these obligations.

**Required proof:** two machine fixtures converge on planning identities and
changes; retained-checkout routing now has two-CLI/simulated-SSH proof, while
delegated first start and running-Flow composition remain. Network failure retains
local writes and visible pending sync; reconnection deduplicates and preserves
conflicting input. Receiving planning completion never moves a local Workflow. A delegation
edit never relabels an existing checkout or claims a process moved.

#### Mixed command: `lf task run` — local operations, planning synchronization

Jack Heart selected execution-Machine-local operations: `task run` and its
Workflow selection/departure/arrival stay in that Machine's ordinary store.
Nested planning uses LOO-406's writer/optional Linear sync, never a callback
planner. LOO-412 independently exchanges portable planning through user-keyed
Git refs by default, explicit shared refs by opt-in. Connection selects/publishes
nothing; Wave selection includes descendants/history. No real planning publication
is authorized. Workflow, checkout paths, Sessions, Processes, claims and cleanup
stay local; importing completion cannot move or stop execution.

Local saves retain stable mutation IDs and pending synchronization while offline;
connected comments/completion use the existing foreground sync/Work stream, not a
second poller. Associated origins retain original journals, receipts, creation
inputs and private selection; only accepted joint frontiers parent saves. Readback
cannot exclude unseen Linear reopening. Per-Work sharing/authorship/assignee and
losing-edit recovery presentation remain unfinished. Seeded reconnect/delegation
fixtures are not running-Flow or exclusive-admission proof. Integration revisions,
ref spellings, superseded mechanisms and failed attempts:
`e4a324636:scratch/compare-cmux-s-command-line.md`, **Mixed command**;
[findings](findings.md#retained-evidence-and-limits).

**Required mixed-operation proofs:** execution-local `task run` and its Workflow
must coexist with repeat Git exchange, disconnect/reconnect, comments/follow-ups,
completion and causal reopening. Seeded state is insufficient: preserve running
controls, divergent execution history and pending inputs. Rejected provider facts
must preserve displayed projection, not just provider cache; unresolved membership
must remain visible without blocking unrelated imports. No central Workflow write
or remote arrival acknowledgement.

### 3. One repository window and command explanation

**Local mechanics and remote Task opening composed; native proof remains.**
Missing recorded remote checkouts still refuse. Bare remote Task links open the
Task page without acquiring a Session; explicit Changes requests without a Session
still use the primary-Session owner.
`desktop open/list` own launch/inspection; `--explain` reads local Work selection,
Task-run actions/impediments and Session connection intent. Startup,
restoration, menu opening and Task links use repository-keyed windows. Per-repository
queues and delivery tokens preserve cold-open destinations and fence canceled
completions. Utility windows and retained surfaces keep their own lifetimes.
Inspection forwards Task and Session actions independently, retaining Task dates;
Session observation time stays unavailable. Saved/failed readings grant no actions.
Configured remote/native proof and broader remote controls remain.
**Explanation coverage, October 9:** run/move/Workflow restart, Desktop open,
Session connect and interactive message-less resume have operation-specific readings.
Create/edit/comment/refile/save share read-only writer validation without recovery,
initialization or provenance allocation. Writes revalidate. Covered cases and
counterexamples: `a44ddcb5f` and [findings](findings.md).

Checkout reads Task-owned placement without requiring a PR; restoration and
preparation share validators without fetches or leases. LOO-418 removes move/run
`--force`; arrival records a completion request before settlement checks PR gates
and eligible cleanup. Preview performs neither.

**Remaining explanation work:** abandon/delete and interrupt still omit cleanup or
control impediments; complete/reopen/follow-up and design-handoff explanation remain. `wave ensure` still
initializes by name: a UUID argument can create a UUID-named Wave; resolve addressed
identity before initialization when completing that command's broader resolution. `history show` and agent invocations
remain identity-only; `--context` previews input. §2 composition and native proof remain.

**Required proof:** cold opens for two repositories arriving before registration and
registering in reverse order both reach their own windows; same-repo opens reuse
one. Plain-repo and remote-unavailable cases remain useful. JSON/Swift agree.

### 4. Arrange and interact with exact retained panes

**Controls and Jack Heart’s October 9 selector change implemented locally.**
Public `--target` and `--surface` are deleted; existing `--repo`/`--task` plus optional
`--pane` select retained Work. Jack Heart accepted the Rex conventions on October 10:
explicit pane wins, then the caller's PTY-validated terminal within selected Work,
then one eligible pane; multiple matches return choices without effects.
`DesktopPaneTarget` and surface tokens stay internal. Resolve through existing
Work/pane owners; validate the captured window/content/surface before acting,
never retarget by later focus. Unique/ambiguous/explicit selectors and retained lifetime
refusals have headless proof. Typed Apple events retain synchronous MainActor validation.
CLI arrangement preserves selection; focus is explicit. Split creates a shell
through the existing execution-location owner; UI splits focus the new shell.
Move stays within one Machine/checkout. Native proof remains.

Lost replies leave uncertain outcomes. Hide/restore are idempotent; split and Shell
can allocate again, so inspection must precede any deliberate retry. Close/Undo
renews content identity; the surface lifetime is separate. Passive reads allocate
or clean up nothing, and neither token alone authorizes input. Earlier detail:
`0dba18b79:scratch/compare-cmux-s-command-line.md`, section 4.

Companions are implemented locally: `lf desktop shell/files/flow-log` dispatches
through the same exact-target command. Shell opening uses the recorded Machine;
Files/Flow-log require the named Task's recorded/prepared checkout to match.
They fill an empty target or split beside occupied contents, preserving selection,
zoom and Work navigation. Files/Flow-log reuse retained panes and drafts; Shell
allocates a new pane, so a lost reply cannot authorize replay. Toolbar and CLI share insertion, publishing the shell command before notification.
Native surface/draft retention remains unproved.

Passive inspection returns a native `surface` incarnation only for an existing
surface, including a retained exited surface until its lifecycle owner releases it.
Missing surfaces remain unavailable without allocating. The token comes from
`ProgramStatusSurface`, not pane occurrence or a parallel generation store. A
released view cannot create a second native surface with its old lifetime.

**Bounded read and exact input are composed, not natively accepted.**
`Package.swift` pins checksum-verified `a60e9e2-lf3`. Fixed caller storage returns
complete UTF-8 prefixes for screen/scrollback/selection, including command blocks;
empty succeeds, bounds are 1–1048576 bytes (default 65536). Visibility is pane-local,
not compositor proof. No unbounded fallback, acquisition or cleanup.

`desktop text/key` synchronously validate the captured Machine/surface, never
clipboard, focus fallback or client takeover. Missing/exited/replaced surfaces
refuse. Jack selected literal cursor insertion into the existing draft; Enter is
separate. C0/C1 controls (including newline/Tab) refuse; IME must finish first.
No silent clear, escape decoding or occupied-draft refusal. Uncertain replies
cannot authorize replay. Richer key chords now belong to LOO-452.

Full mechanism, owned two-shell fixture, failed builds and earlier evidence:
`e4a324636:scratch/compare-cmux-s-command-line.md`, **Bounded extraction** and
**Exact input**. That native fixture remains unexecuted; headless checks do not
establish draft/responder fidelity. No user terminal or installed app changed.

**October 10 adaptation:** mutations return a `DesktopPaneReceipt` naming the affected
pane and repository/window/Machine/checkout. Split returns the new leaf; Files and
Flow-log return the reused or created leaf. This replaces mutation-wide inspections;
`desktop list` remains the snapshot reader. Receipts establish no surface readiness.
`lf help PATH --json` derives argument metadata from Clap; Desktop JSON result
schemas derive from Rust DTOs using Schemars. It reads no running app or store;
installed syntax, running-app support and Work legal actions remain distinct.
LOO-397 is unstarted; broader discovery remains there. Jack authorized separate
LOO-452 shortcut customization and LOO-453 readiness/events explorations.

### 5. Complete-path verification and demo

**Depends on 1–4.** One PR. Gate owns affected Rust suites and
`cargo test -p loopflow --test dto_fixtures` for Rust wire fixtures, plus
`cargo test -p loopflow --test documented_commands` for command ambiguity only.
Headless Desktop coverage uses `scripts/test_desktop.sh -Xswiftc -gnone --no-parallel`.
Gate also owns full `session_lifecycle_tests` and the disposable installation
migration harness for the changed association/schema, isolated per TESTING.md;
empty filters are not proof. LOO-406's `planning_reconnect_tests` owns Linux
work-watch/Flow reconnect
fixtures, with dependency evidence in slice 2. LOO-427 still
needs mixed Git-exchange and Desktop-selection coverage, including selection
changes during foreground sync; no second reconnect harness is needed.

Demo follows **Work tabs → Checks and demo acceptance**, including remote-location
explanation and exact input/readback; no real accounts or live user terminals.

Prior checks: `7a36df4ea2:scratch/compare-cmux-s-command-line.md`, §5.
Current focused result: [findings](findings.md).
