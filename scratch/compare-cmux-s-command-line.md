# Desktop control on shared Work — LOO-427

**Status (October 10, 2026):** Jack Heart accepted this diff for queue, then
Task-completing shipping. Implementation and focused headless evidence are retained;
native/configured proof is not claimed. The owners below replace the earlier demo
hold. Jack Heart’s updated October 10 brief accepts the earlier LOO-412
integration; its newer unmerged reduction is not a shipping prerequisite.

LOO-418/#1499, LOO-406/#1503 and the earlier LOO-412 integration through
`96f714bd6` are composed locally. Work tabs, content-owned locations, ordinary pane
selectors, shell-default Split and lf3 bounded reads remain the accepted baseline.
[Findings](findings.md), [questions](questions.md) and the
[comparison](../docs/reviews/terminal-command-comparison.md) retain evidence and limits.

## Shipping scope — Jack Heart, October 10

Jack Heart reviewed the complete diff, said it was good enough for now, and
requested ownership of every unestablished item followed by queue then ship.
This supersedes the earlier demo-only/no-publication boundary. It accepts delivery
with the following explicit follow-ups, not claims that their behavior is proved:

- LOO-397 (Infrastructure), broadened: remaining effect-free invocation explanations
  and CLI-schema/running-app/current-eligibility discovery. Existing command-map work
  here is the baseline, not another implementation request.
- LOO-391 (Product), broadened: native Work-tab/draft/focus retention, targeted input
  and bounded reads, configured remote Session/Files/log/shell behavior, and planning
  exchange while a Flow runs with disconnect/reconnect and execution preserved.
- LOO-414 (Infrastructure), broadened: exclusive shared first-start admission alongside
  retry-safe detached remote launch. Current no-owner refusal remains until an explicit
  admission authority and unavailable-owner recovery are implemented. No started-Task transfer.
- LOO-416 (Infrastructure), broadened: saved repository/Wave/Task workspaces and
  content-owned Machine locations across relaunch, distinct from in-window retention.
- LOO-452 (Product), already sufficient: shortcut customization and terminal chords,
  including Ctrl-C; literal text stays separate.
- LOO-453 (Product), already sufficient: exact-view readiness/waits/events, failures,
  stale target and reconnect behavior; receipts alone never promise usability.

LOO-412's current source/design retains `lf planning status` and key recovery,
but replaces connect/use/select and per-Wave enrollment with one configured
transport per repository (Linear or personal/shared Git). Mixed Linear/Git exchange
is superseded, not an unfinished feature to file. This checkout contains an earlier
412 integration. Jack Heart’s updated October 10 brief keeps newer unmerged
transport/domain changes with LOO-412, outside this baseline’s shipping prerequisites.
The old setup is implemented here, not the target design; the newer configured
transport is not delivered here. Actual main conflicts and build/test failures remain
queue work. No live planning migration, installation,
provider login or real-data publication is authorized by this shipping request.

Recorded proof limits remain honest. Headless gate runs once for the final tree;
shipping requests Task-completing landing after queue. Native/configured proof is
owned above and does not reimpose a demo hold Jack Heart has released.

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

## Work tabs — accepted October 10; implemented locally

**Build:** one retained workspace per repository, Wave or Task, shown as a tab
named for that Work inside its repository window. Jack Heart accepted this internal LOO-427 slice in one PR, not its native experience. This section supersedes checkout-led presentation below;
existing execution/location safeguards remain. Admission is independent and
unresolved, not a prerequisite for these tabs.

**October 10 correction:** Jack Heart selected Split → shell, with no empty-pane
option (comment `ff61b6bc-f40c-4545-a203-82ced3afd335`). Implemented through the
existing shell-location owner for CLI, shortcuts and pane menus; missing remote
location refuses before layout mutation. Work-keyed tabs, content-routed Flow logs
and exact-target DTOs are now cut over together. The outer layout and its inspection
DTOs are deleted. Files resolves remote content directly, without link priming.

**Completed prerequisites:** Session/shell and Files locations are content-owned;
local Files retains its opening placement through missing planning and Close/Undo.
Remote Files owns observation and transport reuse. Bare Task navigation acquires
no Session; explicit Session/Changes still does. Evidence and failed attempts:
[findings](findings.md#content-location-cuts--october-10). These prerequisites are
not native acceptance. The registry/DTO and navigation cut below now consumes them.

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

### Implemented owner cutover

`SessionsWorkspaceRegistry` now keys entries by `WorkWorkspaceKey(repository,
kind, id)`, with ordered open Work and an optional selection. Hiding drops only
an open reference. WorkModel normalizes planning/runtime Task aliases through its
shared projection before registry lookup, preserving an existing entry when a
canonical ID arrives. Repository association rekeys references without replacing
native owners. Titles refresh from planning; missing planning retains the entry.

Each workspace owns the existing multiplexer, documents, details state and a
`WorkNavigation.WorkspacePresentation`. Navigation references that presentation;
there is no separate per-tab conversation selection to synchronize. Model selection
opens/reuses Work synchronously. Sidebar, palette, Task links and conversation
opening use it. Shell-default Split resolves content location; an unplaced empty
Task cannot borrow the repository's cwd.

One Session has one presentation occurrence. Ordinary opening reveals its retained
Work; an explicit eligible linked Work may relocate presentation without binding
or acquiring another client. Shared membership readings, not path changes, prune
local occurrences. Peer physical Task IDs cannot disprove local correspondence;
partial/foreign evidence retains content. Exact location changes renew content
incarnations. Window-owned surfaces remain shared by the retained entries, never
by different windows. Tab selection/hide revokes native input before view removal.

`DesktopWorkspaceInspection`, internal targets and receipts name Work. Pane leaves
carry Machine/checkout location. Rust scopes by resolved Wave/Task identity, not
checkout; caller inference still validates its original PTY and content Machine.
Mutations validate Work/window/content lifetimes; text/read additionally use the
surface lifetime. Explicit Focus reveals the Work; other mutations preserve it.

Flow logs consume their content location. A matching local reading reuses the
window's Work stream; a different retained location reads captured Flow inventory
and details through the existing query transport, with explicit Refresh, no new
poller or planning reader. Remote reads reuse the same before/after owner validator
as Files. Missing owners do not fall back locally.

### Delete — completed and retained boundaries

Deleted: outer `WorktreeLayout`/store/view and inspection, checkout picker,
Session sidebar rows, checkout-keyed registry, path-based membership pruning,
path-only pane targeting and enclosing-checkout location equality. Work details
remain available inside each tab; there is no alternate all-terminals tree.
`PaneContent.location` also replaces the separate shell/Session/file location
accessors and repeated inspection/Split fallback chains. Terminals keep their
captured location; Files/logs keep recorded-owner precedence, including unavailable
owners. No wire fields or retained content are removed.
The previous implementation proposal and its historical review references survive
at `2feed22e1109:scratch/compare-cmux-s-command-line.md`, **Work tabs**.

The cut remains one PR with the rest of LOO-427. Saved relaunch layout belongs to
LOO-416. LOO-452 owns richer key chords, LOO-397 deeper capability discovery.
First-start admission is independent and still refused; this UI supplies no
arbitration authority. Configured peers, native draft/focus retention and the
rendered exercise below belong to LOO-391, not a LOO-427 delivery hold.

### Checks and demo acceptance

The cutover is checkpointed in `39698225f3`; `37fa81df13` consolidates content
location reads. These supersede the earlier feedback that the registry still keys
by checkout. Recorded focused checks cover the implementation; full affected
coverage remains with gate/CI and rendered acceptance with demo/review.

Acceptance across `WorkNavigationTests`, `WorktreeWorkspaceTests`,
`WorkDestinationTests`, `DesktopPaneControlTests` and DTO fixtures remains:

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

The implementation was reviewed under the earlier demo boundary; October 10
shipping approval now governs. The current command tree and examples belong in [the CLI guide](../docs/lf.md#discover-commands)
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

**Replacement owned by LOO-412, not this shipping cut:** configured transport and
its typed writer supersede `PlanningCommand::{Connect,Use,Select}` and manual
enrollment. This accepted baseline still implements them. The newer dependency
also deletes association/recovery paths and retains `TaskSource`; wholesale
integration would require deliberate reconciliation with correspondence, authored
delegation and no-owner refusal. Jack Heart’s updated October 10 brief leaves
that unmerged reduction with LOO-412. No second exchange engine is introduced.

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

### Admission — deferred to LOO-414

`task_location::resolve` and `require_task_planning` retain shared first-start
refusal. The common `repositories::task_is_shared_in` predicate shares membership,
not admission. Local-only work, planning saves/acquisition and retained execution
remain usable. Remote companions, selectors and Work tabs are checkpointed; none
supplies cross-Machine arbitration.

`4c43550c2`'s transport counterexamples show divergent revisions and independent
refs both confirming. Confirmation, negative peer readings and local transactions
cannot reserve execution. Later delegation cannot move a started Task. The proposed
pinned admission Machine/key remains unreviewed: establishment, associated identity
and unavailable-owner recovery need a decision in LOO-414. Two-Machine allocation,
retry/disconnect and no-replay proof belongs there.

Earlier sequence, proposal and counterexamples:
`16e9c950b9:scratch/compare-cmux-s-command-line.md`, **Latest direction — October 9**.
Jack Heart's October 10 shipping decision defers this implementation; it does not
accept the delegated first-start path as complete or authorize Task transfer.

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

**Remaining ownership:** LOO-414 owns exclusive first-start admission; LOO-391
owns configured remote panes and planning exchange during a running Flow. Repeat
exchange must retain controls, divergent execution history and pending inputs
through disconnect/reconnect, comments, completion and causal reopening. Seeded
Workflow/PR/placement fixtures and retained-checkout opening do not prove that path.
No central Workflow write or remote arrival acknowledgement is introduced.

#### Planning transport dependency

Each invocation, including `task run` and Workflow transitions, uses its execution
Machine's ordinary store. Only portable planning synchronizes. Incoming completion
cannot move a Workflow, stop a Process or clean a checkout. No real-data publication
is authorized.

LOO-412's `2588b1ca27` replaces manual connect/use/select and mixed-provider
exchange with configured repository transport and a typed writer. The newer
inspected commit `58de9907b6` includes domain flattening and main's module/context
changes; its plan reports focused checks, not a combined-tree gate. Neither cut
is integrated here. `PlanningCommand` and `ops/planning_peer.rs` still implement
the earlier enrollment baseline; configured transport is replacement direction only.

The earlier pinned `lf sync --manual 2588b1ca277e1a1a080488b3a31abaae82505685`
attempt conflicted in 33 files and was aborted back to `16e9c950b9`. That remains
failed integration evidence, not a current shipping blocker: Jack Heart's updated
October 10 brief explicitly accepts this older baseline and leaves newer unmerged
transport/domain changes with LOO-412. No dependency retry or peer edit occurred
in this reconciliation. Queue still owns actual main conflicts and failing checks.
The historical mixed design remains at
`16e9c950b9:scratch/compare-cmux-s-command-line.md`, **Mixed command**.

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
Move stays within one Work workspace and preserves each leaf’s content location;
it does not transfer execution. Native proof remains.

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

**Queue scope:** gate the accepted baseline after resolving actual main conflicts
and build/test failures. Newer unmerged LOO-412 changes and deferred proof owners
are not shipping holds. No final-tree gate has yet been recorded. One PR.
Gate owns affected Rust suites and
`cargo test -p loopflow --test dto_fixtures` for Rust wire fixtures, plus
`cargo test -p loopflow --test documented_commands` for command ambiguity only.
Headless Desktop coverage uses `scripts/test_desktop.sh -Xswiftc -gnone --no-parallel`.
Gate also owns full `session_lifecycle_tests` and the disposable installation
migration harness for the changed association/schema, isolated per TESTING.md;
empty filters are not proof. LOO-406's `planning_reconnect_tests` owns Linux
work-watch/Flow reconnect
fixtures, with dependency evidence in slice 2. LOO-391 owns running-Flow
Git-exchange and configured Desktop-selection proof,
including selection changes during foreground sync; no second reconnect harness
is needed. Mixed Linear/Git exchange is superseded, not missing acceptance.

LOO-391’s native exercise follows **Work tabs → Checks and demo acceptance**,
including remote-location explanation and exact input/readback; no real accounts
or live user terminals are authorized here.

Prior checks: `7a36df4ea2:scratch/compare-cmux-s-command-line.md`, §5.
Current focused result: [findings](findings.md).
