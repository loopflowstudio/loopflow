# Open and restore a Loopflow workspace in Rex

**LOO-498 · implementation approved · November 3, 2026.**

Jack Heart explicitly approved building the agreed workspace scope ("let's go!")
after confirming readiness. Continue the existing feature Graph through pursue,
then stop at its interactive demo here. This approval supersedes the earlier stop;
it does not authorize Auto-Ship or merge. The LF Task/workflow terminal view is
included in this Task, not a separate build.

The earlier mistaken run 6029ca25-00da-4e54-b11d-f3005e88e458 was interrupted at
implement. Inspect and reuse or correct its partial session-view/transport edits;
they are not completed implementation. Preserve this checkout and Task identity.

[Exploration evidence](explore.html) illustrates the accepted experience. The
current sections below are the implementation brief; retained kickoff evidence
is historical and does not override subsequent decisions. Simulations are not
runtime acceptance evidence.

## Accepted scope

- A Task workspace is a Rex tab/window with splits. A Wave workspace is a Rex
  session containing its Wave conversation and selected Task tabs. Rex is primary;
  Herder (`herdr`) is secondary with an explicitly bounded, demonstrated adapter.
- Work identity, recorded execution Machine/checkout, existing LF Session and
  provider history are shared. Workspace open/restore never clones a provider,
  replays a turn, starts a Task Graph, or silently takes conversation input.
- LOO-717 revised A: each host owns its ordinary shells and active layout.
  Rex-native shells have no additional lf shell-viewer/retention layer. Exact
  ordinary-shell attachment from Rex to Desktop/lf is excluded from this delivery.
  Desktop keeps its own shell retention owner. Conversation attachment remains LF's.
- Conversation and Shell are terminals in both hosts. Files, Changes and combined
  Task/workflow intent have native Swift views and terminal renderers as appropriate.
- Jack requested a small shared tool vocabulary, configurable terminal rendering,
  and Wave default layouts. Reuse editor and Git settings where they exist.
- Rex's active split tree must not be mirrored into Desktop or maintained as a
  second authoritative LF layout. Semantic layout intent/templates are shared;
  native presentation state belongs to its host. Rex shells are never presented
  as Desktop shells. Persist host occurrence identity and observations, not a
  duplicate shell runtime or continuous active-geometry copy.

## Core open experience — accepted direction

Jack Heart asked which command builds a Rex setup around open Tasks, which layouts
it loads, and whether newly created Tasks can open automatically. Jack accepted
the proposed entry, layout resolution and following behavior on November 3
(“that works for me”), then asked about host capabilities. Jack subsequently
selected only started, unfinished Tasks for
automatic inclusion; backlog remains explicitly selectable.

- Make the existing lf open route the entry: repository-scoped open --host rex
  opens the selected working set; --wave narrows to a Wave; --task opens/focuses
  one Task. These are accepted expanded workspace semantics, still to implement.
- Jack Heart explicitly clarified during implementation: bare lf open must detect
  that its caller is in Rex and open in the appropriate Rex server/session/Work
  location without --host or destination flags. This must also work when an LLM
  inside Rex invokes lf open through its tool shell. Preserve/resolve host context
  through agent subprocesses and existing live viewer placement, including resumed
  conversations; do not assume the agent's startup environment always names its
  current viewer. Reuse the existing routing owner, explicit host overrides and
  Work/Machine resolution. Inspect actual supported Rex environment/API context;
  do not scan running apps or guess a local destination for remote Work. Keep
  conversation input authority unchanged and repeated opens idempotent.
- Repository opening groups Tasks into Wave sessions (plus a standalone group),
  with per-Machine host sessions where needed. Each Task is a tab with tool splits.
- Existing host occurrences reopen unchanged. New occurrences use Task semantic
  override, then Wave default template, then builtin fallback. Host live geometry
  remains native. The Wave defines tools; the host resolves their renderers.
- While the workspace is open, observe shared Work selection changes. A new match
  adds a passive Task entry/tab without focus theft or eager agent/shell launch.
  Unstarted Tasks have no checkout yet and initially show Task/brief state.
- Default selection includes started, unfinished Tasks, as selected by Jack Heart
  on November 3. Merely creating a backlog Task does not add it automatically.
  Jack then accepted automatic additions when Tasks launch ("ok sounds good"):
  observe shared Work state, add a passive Task tab to its matching open Wave
  host container on first start, and reuse that occurrence on later Graph launches.
  Preserve focus and resolve Task layout → Wave default → builtin. Background
  arrival does not start another provider or eagerly run ordinary shell/tool
  processes. A closed workspace reconciles new eligible Tasks on its next open;
  launching a Task does not open a host app unexpectedly. Explicitly selected
  single-Task workspaces do not acquire sibling Tasks.
- Returning to a workspace reconciles missed arrivals. Missing observations retain
  current host work. State changes do not reflow layouts; completed-Task retirement
  uses the separately agreed LOO-717 presence/grace contract.

## Host capabilities — November 3 discussion

Jack Heart requested comparing Desktop, Rex, Herdr and cmux by capabilities,
especially Task layouts and splits. A host being accepted by lf open does not
prove the complete workspace integration. Keep these two claims separate.
Jack accepted this capability-based support model on November 3 ("sounds good"):
create, reopen, restore, move/copy, follow and lifecycle observations are separate
support claims. Rex remains first, Herdr second; cmux coverage remains open.
The subsequent explicit build approval covers Rex first and bounded Herdr support.

| Host | Native layout capability | Candidate Task / Wave mapping | LF integration today |
| --- | --- | --- | --- |
| Desktop | Saved Task pane layouts, split ratios and tool views | Task workspace / existing Wave navigation | Existing Task workspace and conversation opening |
| Rex | Sessions contain windows with split block trees | Task window/tab / Wave session | open route explicitly unsupported; native layout creation already probed in isolation |
| Herdr | Workspaces contain tabs with pane layouts; layout apply/export API | Task tab / Wave workspace | Conversation opening; full LF layout/default/arrival adapter remains to build and demonstrate |
| cmux | Workspaces contain split panes, each with surface tabs; declarative layouts | Task workspace; Wave grouping needs adapter validation | Conversation opening; no LF Task-layout adapter; CLI absent on this Machine |

cmux documentation also describes workspace groups, a possible Wave mapping.
That is public documentation evidence, not installed-version or runtime proof.
Do not make equal-priority cmux delivery a requirement from this comparison alone.
Rex remains primary and Herdr secondary until Jack changes delivery scope.

Evaluate separately: layout creation; reopening the same occurrence; saved-layout
restoration after host restart; live process retention; native reorder versus
cross-container move; passive additions; reliable focus/close observations for
LOO-717. Restoring a layout never proves retention of the same running processes.
The shared Wave defaults and LF-aware automatic arrivals are new integration work.

Sources: local docs/surfaces.md and lf/commands/open.rs;
[cmux layouts](https://cmux.com/docs/custom-commands),
[cmux concepts](https://cmux-docs-release.vercel.app/docs/concepts),
[Herdr API](https://herdr.dev/tui/docs/socket-api/), and the installed Rex probe below.

## Shared tools, target data and renderer selection

Use five semantic intents, reconciling the existing portable model:

| Tool | Desktop | Rex |
| --- | --- | --- |
| Conversation | Terminal attached to existing LF Session | Terminal attached to same LF Session |
| Shell | Desktop-owned terminal/shell | Rex-owned terminal/shell |
| Files | File tree/editor | Selected terminal file browser/editor |
| Changes | Native diff view | Git diff/pager or Git difftool |
| Task & workflow | Task overview/workflow view | LF terminal Work view |

Files carries the directory/file/selection. Changes carries the exact working-tree,
branch or PR comparison; those targets are not interchangeable. Task/workflow
combines brief/planning facts, Task workflow position, PR, associated run history
and selected-step result/log inspection. Task graph position and all associated
Graph processes stay distinct; latest does not grant primary authority.

`lf task view` is the working name for the terminal Work renderer; engineering may
choose the shortest unambiguous CLI shape consistent with the existing tree. It
uses shared readers already exposed by `task status --watch --json` and graph
process detail, rather than a second store or an agent. Opening it is passive.
Reuse existing native TaskWorkView/WorkSurfaceView concepts. Start with useful
read/navigation; consequential actions route through existing Work command owners.

Honor conventions on the Task's execution Machine/in its repository:

- Ordinary file editing uses VISUAL/EDITOR. Git's editor selection is GIT_EDITOR,
  core.editor, VISUAL, EDITOR, compile-time default. Git should resolve its own
  policy (`git var GIT_EDITOR`); a commit editor need not define a file browser.
- Diff-tool invocation delegates to Git's diff.tool and difftool.<tool>.cmd/path.
  Plain patch viewing delegates to Git's pager: GIT_PAGER, core.pager, PAGER, default.
  Use ordinary Git diff/pager for basic viewing and expose Git difftool as the tool
  action/mode. diff.external/GIT_EXTERNAL_DIFF concern generation, not this selector.
- A terminal file browser has no comparable universal selector; provide a narrow
  configurable binding and an honest useful fallback. Do not invent another broad
  editor/diff registry. Existing user-selected GUI programs may open outside Rex;
  report that accurately rather than claiming embedded terminal rendering.
- Git setting values may be shell command strings. Let Git interpret its own
  settings. Keep LF's resolved file/target arguments separate and correctly quoted;
  do not concatenate arbitrary paths into shell text or treat all settings as a
  single executable filename. Create tool blocks with direct argv, not keystrokes
  injected into existing terminals. Reopen never reruns a live tool command.

Verified references: [Git variables](https://git-scm.com/docs/git-var),
[Git difftool](https://git-scm.com/docs/git-difftool),
[Git configuration](https://git-scm.com/docs/git-config).

## Wave defaults and layout resolution

Wave owns a default Task-layout template: semantic tools, groups, order and split
ratios. Task owns a saved/customized semantic instance; each host owns its active
realization. Template/config details are engineering choices within this model.
Do not add a selection language or parallel layout representation.

Resolve in this order:
1. Reopen an existing destination occurrence unchanged.
2. Otherwise use the Task's saved semantic layout.
3. Otherwise instantiate its Wave's default.
4. Otherwise use the builtin fallback for unconfigured/standalone Work.

Preserve today's useful default arrangement when migrating the model; avoid
expanding every saved background tab into a split or starting every possible tool.
A visible selected face is the conservative source mapping; explicit inclusion can
add others. A Wave template can deliberately select more tools/shells. The preview
must state which new shells/tool processes an explicit creation starts.

Wave template edits affect future materialization. Applying a default to existing
work is explicit and preserves live input/tools until separately handled. Reopening
must not reset a host's user-changed arrangement. A native host action may inspect
or copy its own geometry transiently; that does not authorize an ongoing LF mirror
or writing Rex's active split tree over Desktop's saved layout.

Today WorkPaneLayout is JSON in work_pane_layouts, keyed by repository/kind/Work ID,
in $LF_HOME/loopflow.db (default ~/.lf/loopflow.db). Desktop autosaves pane intent,
focus/hidden state and file drafts. Existing kinds are Session/Shell/Browser; Files
lives inside Browser. Reconcile these types and consumers, including retained URL,
Graph-log and draft state, rather than adding a second implementation. No migration
may silently drop these existing contents. The LOO-498 layout read returned null
in this environment; a type/store existing does not prove a Task has saved intent.

## Create, move, copy and partial outcomes

Create/open materializes existing Work into the destination using its tool renderers.
A Wave consumes existing shared selection, not another membership table. New query
members may append passive entries without starting providers or stealing focus;
partial observations never imply departures. Unstarted Tasks show their brief and
explicit start affordance. One Rex server owns one session: mixed-Machine selections
need per-Machine sessions and visible outcomes, never silent local execution.

Repeated open reuses the same occurrence, tools and Session identities. Ordinary
open leaves input control with the existing controlling viewer until an explicit
handoff. Native tab/session reorder keeps native shell identity and input intact.
Cross-host/session move materializes the supported semantic view then explicitly
hands off the selected conversation; source shells, jobs and unsaved Files remain
reachable under their owner. Source input is unchanged if preparation fails.
A Wave move does not take all conversations' input just because it opened their tabs.

Copy creates another identified presentation occurrence of the same Work and LF
Session references. Default to observing and copying semantic intent, not shell
memory or source commands. New shells require explicit counted creation. Reopening
that copy reuses it; making a further copy is a distinct action. Selection remains
shared, with host-native layout independent. Do not change actual Task Wave
membership when moving its presentation. Prefer literal CLI labels/receipts where
“move” could falsely imply native process migration.

Installed Rex session.move_window only reorders inside a session; session.move_block
also requires both blocks in that session. Cross-session live-tab migration is not
established. Do not fake it by destroying and recreating shells. Keep unrelated
panes and source-host work intact, and return per-item materialized/reused/showing/
unavailable/unsupported/uncertain outcomes. Materialization is not input readiness.
A lost create reply remains uncertain until inventory reconciles it; no duplicate
retry. Reuse existing open serialization and Work resolution.

## Session lifetime and cleanup coordination with LOO-717

Latest shared direction (comments 0f320e9f-4267-4cf1-ac2b-12ed3116b60d and
554951b7-e207-4153-961e-2f7fc94a445f, November 3) supersedes earlier retention
proposals. LF treats Desktop and Rex equally: a host retaining a Session makes
LF retain it. Each host owns its release policy. LF does not encode a special
Desktop tab/selection rule, and Desktop should not favor completed Sessions.
Host retention of a conversation is separate from ownership of ordinary shells.
Rex still owns its ordinary shells and active layout; no LF shell-viewer layer.

LOO-498 supplies Work-to-host occurrence identity, retained/released/closed/use
observations and native cleanup actions. Unknown host state is not proof of
closure/release. Copies participate; close only the Task's occurrences, never
an entire Wave container containing other Tasks. Resolve the equal-host contract
with LOO-717 without inventing a parallel lifetime owner or timer.

Jack proposed the completed-and-away sequence: eligible idle agent ending under
the existing two-minute rule, with checkout and shells retained thirteen more
minutes (fifteen total). Return during that interval can resume the same
conversation in the retained checkout. Exact deadlines/release policy and APIs
remain LOO-717's work. Active requested turns and Graph work finish before cleanup.
Explicit view closure may end Sessions after that work; unfinished Task membership
alone does not retain execution. Preserve Task/conversation history; closing an
unfinished Task view must not delete its checkout or branches.

Jack explicitly accepts eligible completed-Task cleanup even with dirty/untracked
files; those are not a veto. This does not decide moved/unmerged committed refs.
LOO-498 does not perform destructive cleanup during probes or invent that policy.
Automatic reconstruction of a deleted checkout was an agent-introduced hypothetical,
not a requested feature or dependency. Do not build it. Simulated review advice
from LOO-717 is evidence to consider, not additional user decisions.

Independent layout/tool/host work can proceed while LOO-717 finishes its contract.
LOO-717 subsequently received build approval (comment
f7c1b9d7-d3f7-4ab3-bcda-5ddd2b3e5569): the two-minute eligible idle-agent ending and
fifteen-minute total completed-and-away cleanup are now accepted. Its Graph
f520421e-c378-4928-920f-962f8a5585fa is implementing shared lifetime/Desktop
execution. Coordinate that implementation; LOO-498 retains its native Rex adapter.

## Approved implementation boundaries and slices

Preserve this checkout and existing work. Core entry, layout sources, default
selection, automatic arrivals and Rex-first/Herdr-second priorities are settled.
Jack approved starting this implementation after the readiness review.
cmux parity is outside this build. LOO-717 still owns lifecycle policy/API details;
that integration dependency does not prevent starting the independent slices.
Continue through publication to the feature Graph’s demo; no Auto-Ship or merge.

1. Reconcile the five-tool semantic model, Wave default template storage/resolution,
   existing Desktop persistence/renderers, and DTO fixtures. Keep active layouts
   host-owned. Use one migration for this Task if required, with correct revisions.
2. Implement terminal tool resolution using existing editor/Git conventions and
   the LF Task/workflow viewer. Resolve exact Task location, not ambient cwd/home.
3. Replace Rex's unsupported open branch through existing resolution/serialization.
   Materialize Task tabs/Wave sessions; record occurrence identity, reconcile repeat
   and partial opens, support explicit movement/copy and handoff boundaries.
4. Inspect/prove native Rex event/use/close capabilities and integrate the occurrence
   contract with LOO-717. Do not mirror active split geometry or retain native shells
   through lf. Missing lifecycle proof is a specific integration gap, not permission
   for guessed cleanup. Use the supported public host APIs; no competitor source.
5. Implement the bounded Herdr consumer with real runtime checks and capability
   receipts; update CLI help and docs/surfaces.md. No generic multiplexer platform,
   Lua runtime, mandatory Rex backend, cmux parity or Mobile work.
6. Run affected tests and required format/lint checks once for the final tree,
   publish via pursue, then stop at feature's interactive demo in this conversation.

Session recovery and passive observation remain shared with LOO-709/672. Existing
ViewContent.argv can resume/start a provider and cannot simply be run to paint
historical Sessions. Do not introduce eager startup. Current task_shell delegates
to an lf owner and is not the Rex-native launch route. Validate the recorded checkout
before native creation; Rex itself falls back home on invalid cwd. Installed public
APIs and current delivery of related Tasks must be reread before integration;
kickoff evidence below is dated, not proof of a newly delivered dependency.

## Acceptance and verification

- Exercise bare lf open from a Rex shell and from an LLM tool subprocess in Rex.
  Both resolve the appropriate existing server/session and selected Work location;
  explicit --host still overrides auto detection. Cover resumed/moved viewer
  context, duplicate opens, and remote routing without silently opening Desktop
  or a local checkout. Verify via the actual caller path, not only mocked env reads.
- Wave defaults determine semantic Task layouts; Task customizations override them;
  repeated open preserves the host arrangement. No Rex active layout mirror.
- Each tool renders with the same Task/target meaning in Swift/terminal as applicable.
  File/editor and diff renderer selection honor standard settings in the correct
  Machine/repo; PR, branch and working-tree comparisons stay distinct. Task/workflow
  view shows shared facts and graph position without creating Work or running agents.
- Real Rex select/open → work in a Task → return → close/reopen uses the intended
  Task tab/Wave session shape. Repeated/concurrent opening creates no duplicate
  Session/provider/shell and replays no turn. Preserve unfinished input.
- Native reorder preserves execution; cross-host move/copy gives honest target
  renderers/fresh shells and source retention. Copy starts passive; explicit input
  handoff does not create another provider conversation.
- Failed remote/checkout/host operations never open a local/home shell. Partial
  updates preserve source/unrelated work, stay inspectable, and retry without
  duplicate or destructive rollback. Host-restarted fresh shells are not treated
  as the original execution despite stable block IDs.
- Occurrence observations correctly distinguish use/left/closed/unknown, aggregate
  copies, and permit only the owning completed Task's cleanup under LOO-717 policy.
  Other Task tabs in the Wave remain intact. Do not delete resources as part of this
  prototype/design conversation; lifecycle proof uses isolated fixtures.
- Demonstrate bounded Herdr behavior and explicit adapter limits. Real configured
  app focus, remote visibility and provider usability remain demo evidence.

Use focused meaningful tests at each slice, then the repository affected-suite gate,
format and clippy checks. Keep one Cargo command at a time and clear inherited LF_*
for fixture tests, with stdin /dev/null, per AGENTS.md. Extend the existing shared
selection/open/DTO/transport tests rather than testing mock wiring. Isolated host
contract trials should be retained as actual tests where useful; the scratch probe
alone is not full feature acceptance. No GUI requirement blocks earlier Graph work.


## Retained kickoff evidence

The following evidence is retained from kickoff. Its old session-per-Task
recommendation is superseded by the accepted tab/session mapping above.

### Evidence from kickoff

Inspected checkout: `2bc29a72e0c3479dff528a4232e8d9d18268a0b2`.

### Shared selection and current launch paths

- LOO-497/#1580 and LOO-489/#1594 are done/merged according to November 3
  `lf task status --json`. Their delivered foundation is
  `work/selection.rs`: `Selection`, `Dashboard`, `Arrangement`, `ViewPlan` and
  `WorkPaneLayout`; SQLite selection readers use a coherent Work snapshot.
  `lf desktop view EXPRESSION --plan` and `desktop plans --watch` exist. They
  are passive despite their command namespace. There is no need for a second
  workspace query language or an imagined older `--plan` API.
- `ViewPlan.contents` describes a Task's primary/zoomed face, not its complete
  pane layout. The full `WorkPaneLayout` holds the chosen Session, shell tabs,
  hidden panes, focus, ratios and file drafts. A host needs both. A missing
  primary is not permission to allocate one. `ViewContent.argv` currently uses
  `human_session_argv`, which resumes a conversation; executing every entry
  would violate lazy opening. Reading a passive plan does not make its argv
  passive.
- `lf/commands/open.rs` resolves Work once and has host routing, placement and
  an opening lock. Its 30-second starting receipt is not durable workspace
  restoration. Herdr currently creates a shell and types a resume command into
  that newly created shell. Rex is an explicit unsupported branch.
- Shell ownership is already split. `SessionsView.swift::shellCommand` returns
  an empty command for local Desktop shells (native terminal owns the shell),
  but remote shells run `lf task shell --checkout ...`. In
  `ops/task/workspace.rs::task_shell`, that path validates the recorded checkout
  and calls `transport::delegate_current(false)` before launching the shell.
  `ShellTab.lf_process_id` exists, but is not a complete local/remote recovery
  contract. Merely calling that function from Rex would add an lf owner;
  describing the result as a Rex-owned ordinary shell would be false.
- `transport.rs` already separates execution ownership from revocable viewer
  input. `resume` can acquire/control or start a provider. `--prepare` can select
  or create a primary conversation. Neither is a passive workspace paint API.
  Saved Ghostty screens restore display, not editable provider state.

### Rex: verified installed API, not inferred from another terminal

Installed CLI/server API: `0.0.1793377904-ge6592f`, revision 8. The installed
`api describe`, `block_creator.list` and app `client.inspect` were read without
changing the person's workspace. Public references:
[CLI model](https://www.superlogical.com/rex/docs/automate/cli),
[API discovery](https://www.superlogical.com/rex/docs/automate/server-api),
[remote access](https://www.superlogical.com/rex/docs/linux/remote-access).
No competitor implementation was copied.

The isolated experiment in `scratch/rex-host-probe.py` used a separate Unix
socket, empty configuration and temporary state directory, and stopped its server:

1. A Rex **session** contains **windows** (app tabs); each window contains a
   split tree of **blocks** (terminals). `session.create(initial_windows)` and
   `session.new_window(layout, focus:false)` create layouts. Tree direction and
   ratios are supported; layout creation starts new blocks, never adopts old IDs.
2. Detaching and reinserting a block preserved its ID, program output, input and
   focus in a two-block window. Each CLI connection exited between operations;
   the server kept the program. An invalid new-window request left the existing
   session snapshot unchanged.
3. Detaching the last block removes its window. `move_block` requires a placed
   anchor, so an all-detached session cannot be rebuilt solely by moving its
   existing blocks. The probe used one session for its test workspace; preserve native containers on hide; do not implement hide
   by detaching everything or introduce a permanent dummy terminal.
4. After a server stop/restart, saved session/window/block IDs survived, but the
   original `/bin/cat` command did not: the foreground program was a **new zsh
   login shell**. The test command's start counter stayed at one. Stable block
   identity is not execution identity. One immediate stop also lost a just-created
   session; a two-second settling interval retained it. No crash-durable save
   acknowledgement or exact persistence interval was established.
5. The installed terminal creator explicitly substitutes the home directory
   when `cwd` is invalid. Never rely on `cwd` alone to enter a Task. Direct argv
   execution is supported through `shell:"none", command:[...]`; avoid
   `initial_input` and command injection into existing terminals.
6. `client.perform(session.select)` with `session_id`/optional `window_id` is
   declared by the installed Mac app, as are collapse/expand. Server focus alone
   does not prove an app is showing that session. No client action was invoked
   in this headless run. App refusal, disabled remote control, multiple clients
   and an absent app remain distinct from successful server materialization.
7. The inspected creator runs on its server; no per-block Machine selector was
   found. Target a configured Rex server on the Task's recorded Machine. The
   public remote documentation says CLI host configuration does not synchronize
   to the GUI. A working `rex -S ...` therefore proves neither GUI enrollment nor
   visible remote opening. Do not silently replace this with a local SSH shell.

### Herdr: secondary scope and proof limits

Installed `herdr 0.9.3` exposes bundled API protocol 22. Schema inspection proves
`workspace.create`, `tab.create`, `layout.apply`, `pane.process_info` and explicit
metadata reporting exist. `LayoutNode` can name existing panes or new command
argv, and `layout.apply` defaults to preserving focus. Workspace metadata has
a TTL (at most one day), so it cannot own durable LF identity. Unlike Rex, pane
IDs change when moved across workspaces; do not key execution by a pane ID alone.
No matching non-destructive pane-hide method appeared in the installed schema.

The documented distinction between
[live retention and snapshot restore](https://herdr.dev/tui/docs/session-state/)
matches the needed contract: client detach retains programs; server restart
restores layout but does not preserve arbitrary processes. Native agent restore
must not independently resume an LF-managed provider. The
[remote model](https://herdr.dev/tui/docs/persistence-remote/) runs panes on the
remote server. Runtime mutation and restart behavior have not been tested here.
Herdr parity cannot be claimed from schema inspection. Its current scope is
select/open/reuse/leave/reopen/inspect/explicit cleanup, not automatic hiding,
Rex's exact geometry, or a second agent-restoration system.


## Delete — do not maintain

Removed in this branch; keep these paths retired:
- `open.rs::open_herdr` keystroke-based resume and its exclusive
  `herdr_types_connect_into_a_newly_created_pane` test; native direct argv and
  passive Session observation replace them.
- Rex's unsupported `open_one` route; native workspace opening owns Rex.
- Workspace execution of `ViewContent.argv`; use existing Session identity.
- Separate open/follow Wave-group construction and a second local selection
  plan. Both consume `task_workspace`; one plan supplies Machine routing and
  local opening, including unavailable evidence once.
- `SqliteStore::host_occurrences` bulk loading for one window lookup;
  `host_occurrence(id)` reads the exact occurrence.

No remaining predecessor deletion target. Preserve URL/Graph-log tabs, file
drafts and background selection in the existing pane model. The native layout
renderer and bounded Herdr protocol now live in `open/workspace/layout.rs` and
`open/workspace/herdr.rs`; no extra semantic tree or host runtime was introduced.

## Remaining implementation

November 3 implementation: native open/reuse, named observing copies (explicit
fresh-shell opt-in), native Shell/Files/Changes commands, passive Session viewing,
Task/workflow/log browsing, Wave templates, and shared Desktop tools are present.
Rex opening routes each execution Machine through its existing LF route. Occurrence
locks serialize CLI creation with the native selection follower. Host state owns
geometry. These are branch implementation facts, not acceptance of the full build.

The independent implementation is ready for review; restoration and lifecycle
integration need a design amendment before dependent work can continue:
- LOO-717 returned from compress to implement with no PR or delivered integration
  API in the final November 3 read. Integrate its equal-host retention/release contract. No separate
  cleanup owner, grace timer, or checkout deletion belongs in this change.
- Installed Rex `client.inspect` exposes attached session IDs and connection state;
  `session.view` exposes each session's active window. Neither establishes app
  foreground use or a person leaving a Task. Reliable use/away evidence remains
  unresolved; unknown inventory cannot authorize retirement.
- Restart preserves native block IDs but can replace tools with fresh login shells.
  The adapter preserves these blocks. It cannot yet distinguish untouched restored
  shells from new native work well enough to restore tools safely. Replaying source
  commands or replacing retained blocks would violate accepted input/state
  preservation. The full restoration claim remains unimplemented, pending that
  authority decision and a supported host signal.
- Passive arrival currently keeps its Task view on explicit reopen. Activating
  saved tool intent from that placeholder, while preserving subsequent native
  edits, still needs an explicit transition; it is not full workspace restoration.
- Following runs in the native Workspace tab. Closing that tab stops it; an explicit
  open reconciles missed arrivals, but restarting a closed follower inside an
  otherwise retained container is unfinished. Remote grouping is implemented but
  lacks a real peer acceptance run, including newly relevant Machines after opening.
- Integrate native scoped cleanup and presentation handoff receipts with LOO-717.
  Named copies observe; existing `lf resume` is the explicit conversation control
  action. Cross-host ordinary-process migration is not promised.
- Actual LLM tool invocation after viewer resume/move, app visibility and remote
  provider usability still need demo evidence. Source routing tests cover current
  viewer placement outranking stale startup context. Gate owns affected suites;
  pursue/publication and interactive demo remain later Graph steps. No Auto-Ship.

The isolated native Rex check now covers direct bare-open detection without
REX_SERVER, retained renamed windows/blocks, explicit repository scope from inside
a Task checkout, passive first-start arrival without focus change, and reuse of
that arrival on explicit Task opening. Isolated Herdr covers create/reuse/close.
Both create zero LF Sessions. Rex's configured default selects its local endpoint;
a custom server whose default differs requires `--server`, and a missing caller
block produces an error without Desktop fallback.
The Rex fixture passes headlessly outside `scripts/test_network.py`; inside that
network sandbox, its native caller produces neither a receipt nor an exit/error
file before the deadline. Herdr passes inside the network sandbox. The Rex caller
fixture now captures stderr and exit status so command failures are inspectable.

Check: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p loopflow --test integration work_open_tests::` (15 passed), `cargo test -p loopflow --lib lf::commands::open::workspace::` (2 passed), and both opt-in `native_workspace_` checks pass (Rex outside the network sandbox). Prior observer/editor/Task-view/Wave-default/committed-diff checks and 22 `scripts/test_desktop.sh --filter 'WorkSelectionTests|RegistryQueryTests'` tests remain applicable to their unchanged paths. Gate owns full affected suites and remote acceptance.
