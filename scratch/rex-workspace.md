# Open and restore a Loopflow workspace in Rex

**LOO-498 · revised build authorized · November 4, 2026.**

## Accepted ownership simplification

Jack Heart requested broader simplification and then approved building the revised
boundary. LF owns its layouts/templates; Rex owns native mutations and shells.
Automatic shell cleanup applies only to Loopflow Desktop-owned shells. A retained
Rex terminal for a Task Session is enough to prevent Task cleanup, regardless of
focus, visibility or navigation away. LF does not close Rex shells. This removes
Rex away detection and native closure/settlement from the accepted implementation.
Normal cleanup eligibility can resume after the last such terminal is confirmed
gone; failed or unavailable host reads do not establish absence. Retention protects
Task resources through the shared lifetime owner, not merely layout records.

The named-copy/handoff reduction below remains a proposal; the build approval is
for the clarified ownership/retention contract and its necessary simplifications.
Do not remove other accepted behavior merely to reduce implementation size.

**Accepted experience:** select Work, launch its template in the current host,
then use that host normally. Reopening finds the existing destination and leaves
it alone. If it is gone, an explicit open starts from LF's template again.
"As faithfully as possible" is deliberately loose, as Jack clarified; do not
turn it into a new fidelity taxonomy or exact geometry contract.

### Three concepts

| Concept | Responsibility |
| --- | --- |
| Work selection | Existing repository/Wave/Task selection; started unfinished Tasks by default, backlog explicit |
| Launch template | LF's saved Task layout, then Wave default, then builtin; tools and initial arrangement |
| Host destination | Where to find or create native views; Rex owns everything arranged there afterward |

The five tool intents stay: Conversation, Shell, Files, Changes, Task & workflow.
They do not require a new cross-host workspace object. Desktop keeps its own
layout persistence. External adapters read LF templates; they never write native
arrangements back. Git/editor preferences and the LF terminal Task view remain.

### Three reductions

1. **Keep host records for discovery and creation.** Native IDs survive renamed
   tabs and preserve named-copy reuse. Pending receipts prevent duplicate creation
   and tool replay after lost replies. These records contain no native split tree,
   host liveness, or retention policy. The remaining Task-window locator stores
   only its Task ID; Herdr needs only its workspace locator. See the deletion list
   for the removed observations and unused storage.
2. **Make automatic arrivals an append-only opening service.** It watches the
   existing Work selection and adds newly started Tasks without taking focus.
   It does not continuously reconcile layouts, move tabs when Wave membership
   changes, remove completed Tasks or repair native programs. Keep passive arrivals
   and explicit tool opening as the current requirement. Their one-time creation
   still needs duplicate prevention; the proposal does not establish that all
   creation receipts can be deleted. Closing a tab stays respected during the
   subscription. Explicit reopen can reconcile missed arrivals.
3. **Observe retention; never retire Rex shells.** Accepted November 4: a
   retained Rex terminal for an LF Task Session prevents cleanup. No foreground,
   away, idle-shell or active-window inference is needed. LF never closes native
   Rex shells or sends a native-settlement acknowledgement on their behalf.
   Automatic shell cleanup covers Desktop-owned shells only. Use the shared LF
   lifetime owner for the retention hold and normal eligibility after confirmed
   release. Unknown host state cannot release a hold. No separate adapter timer,
   cleanup executor or shell runtime is introduced.

Move and copy follow the same ownership: native rearrangement uses host controls;
opening another view launches the LF template and leaves the source intact.
Propose dropping LF's durable named-copy lifecycle and coordinated cross-host
handoff from this adapter. Explicit conversation resume remains the existing LF
operation. This does not transfer running shells, unsaved editor state or input.
Named-copy reuse and coordinated destination preparation were earlier commitments;
their removal is a second product choice, not an implementation cleanup.

### What simplification must preserve

- Bare `lf open` inside Rex, including an LLM tool call, resolves the actual
  current destination. A stale locator cannot override current viewer placement.
- Task tools launch on the recorded Machine and checkout. Native later splits
  must still use the source Task's worktree; the unverified host capability is
  an engineering question, not automatically waived by loose layout fidelity.
- Opening/reopening creates no provider, submits no turn and takes no input.
- Existing native edits and unfinished input survive; no command replay after
  server restart. Missing programs are not repaired by reopening a retained tab.
- Creation uncertainty stays visible. Inspect host state before retrying, retain
  the smallest receipt needed to prevent duplicates, and preserve partial work.

### Normal path, failure and recovery

`lf open --wave product` finds or creates a Rex session and Task tabs using LF
templates. Jack rearranges a Task in Rex. Opening it again focuses that tab
without applying the template. A newly started Task arrives passively.

If Rex is unavailable, opening reports that failure. If creation times out,
inspect native state before another create; never erase unrelated panes. If Rex
restarts, its own restoration decides which tabs and programs survive. A found
tab is left alone; an absent tab can be created by an explicit open from the LF
template. Rex-only edits are not recoverable through LF.

Counterexample to a completely stateless adapter: the current fallback marker is
part of a visible native title. Renaming can remove it. Native IDs in a small
locator may therefore remain necessary for reliable reuse. Hidden host metadata
support has not been established. Closing and reopening a broad workspace also
requires a clear boundary between reconciliation and respecting closed tabs;
do not promise indefinite close suppression without somewhere to retain it.

### Review and next proof

Jack Heart approved implementation of the revised ownership and retention boundary.
Continue the existing feature Graph at implementation, then stop for its authored
demo. No Auto-Ship, merge or Task completion approval was added. The separate
named-copy simplification is optional and unaccepted; preserve that behavior.

Delete superseded Rex lifecycle machinery across storage, adapters, commands and
tests rather than relocating it. Keep only necessary discovery and uncertain-create
state. Prove retained Task Session terminals prevent cleanup while unfocused;
confirmed final removal releases that hold without LF closing any Rex shell;
unavailable inventory preserves uncertainty. Verify real Task open, rename/reopen,
native arrangement, passive arrival, partial creation and restart without replay.
Preserve native-split and actual-agent routing proof obligations.

## Layout ownership correction — November 4

Jack Heart selected template launching into Rex with best-effort reproduction:
"try to recreate this as faithfully as you can." Layout state in LF's database
belongs to Loopflow's own UI and templates. External hosts may consume it;
Rex mutations do not write it back. Rex owns subsequent arrangement, persistence
and restoration. LF is not responsible for storing or recovering Rex edits.
This supersedes the earlier requirement for LF-managed full native restoration.

Existing native work must still survive a repeated open. Recreating a template
is an explicit launch operation, not reconciliation that overwrites Rex edits.
Task, Machine, checkout and conversation identity remain exact; geometry and
host-specific tools are best effort. No new approval to replay turns, transfer
input, merge or ship follows.

The branch stores semantic Wave defaults, native destination IDs and creation
receipts; it stores no Rex split tree or host-retention state. IDs remain necessary
for native renames and named-copy reuse. Pending receipts prevent duplicate
creation and tool replay after lost replies. The unused occurrence timestamp and
persisted new/closed/retained observations have been removed. Automatic arrivals
and shared execution retention remain separate responsibilities.

Implementation has reached a review boundary. The raw Rex split API does not
inherit Task cwd when omitted, but the app's Split action has not yet been proved
to behave the same way. The interactive demo also found the branch executable
incompatible with the installed LF store. See [workspace-demo.md](workspace-demo.md)
for the configured-path failure and the prepared native Split test. Do not infer
a required scope reduction from the raw split probe alone. Scope remains accepted
and unfinished.

Jack Heart approved implementation after readiness review in comment
`7088ad9c-c923-4fff-acd7-0a261921b0f0`, superseding the mistaken-launch stop.
The LF Task/workflow terminal view belongs in this Task. Review publication and
demo are authorized; Auto-Ship, merge and Task completion are not.

Jack's later comment `b97b5f14-864e-45e5-8d56-29476bb1e800` prioritizes user
documentation before further feature implementation or publication.
[docs/workspaces.md](../docs/workspaces.md), moved into docs at Jack's explicit
correction, is the reviewable user
contract, distinguishing the agreed experience from delivery gaps.

The implementation at `3d61147152` provides opening, tools, passive arrivals,
one-time arrival activation and follower recovery. Since `af774cada6`, passive
arrivals also validate and use the Task checkout, and the builtin layout avoids
duplicating Task & workflow when no conversation exists.
Template relaunch, lifecycle coordination and the complete configured experience
remain unfinished. Earlier exploration is in [explore.html](explore.html);
simulations do not establish runtime acceptance. The complete pre-reconciliation
plan and kickoff evidence are preserved at
`cc607dc3e1:scratch/rex-workspace.md`.

## Accepted outcome and constraints

Jack's November 3 direction is to return to familiar Loopflow Work in Rex,
with existing Task Sessions and provider history. Rex is primary; Herdr is a
bounded secondary adapter. cmux parity, Mobile, a generic multiplexer platform
and a Lua runtime are outside this build.

- Repository, Wave and explicit Task entry use `lf open`. Repository scope groups
  started unfinished Tasks into Wave sessions plus a standalone group; each Task
  is a native window/tab with tool splits. Unstarted backlog remains explicitly
  selectable and shows its brief without creating execution. An explicit Task
  workspace does not follow sibling Tasks.
- Bare `lf open` inside Rex must select the correct server/session and Work
  location, including an LLM's tool shell and resumed/moved conversations
  (Jack's clarification `288a39bd-1059-4778-b4aa-8bf5bec26fb4`). Current viewer
  placement outranks stale provider startup environment; explicit host selection
  overrides inference. Known Rex/remote failures cannot fall through to Desktop
  or local/home execution. No running-app scan supplies destination identity.
- Work, execution Machine/checkout and conversation identity remain shared.
  Opening never starts a Task Graph, clones a provider, replays a turn or silently
  takes input. Passive history/viewing is distinct from explicit `lf resume`.
- Rex owns its native ordinary shells and active layout; Desktop owns its own.
  No LF ordinary-shell viewer, cross-host shell attachment or continuous Rex
  split-tree mirror is included. Semantic templates are shared, active geometry
  is not. Files/drafts and source shells stay reachable under their owning host.
- Reopening preserves an activated occurrence unchanged; explicitly opening a
  passive arrival adds its selected tools once beside existing native panes.
  New occurrences resolve
  saved Task layout → Wave default → builtin. Wave template changes affect future
  materialization; applying them to existing work requires an explicit action.
  Selected visible tabs materialize, not every background tool or shell.
- An open broad workspace follows shared Work changes. First start adds a passive
  Task entry without focus theft, providers or ordinary tool/shell startup. Later
  Graph runs reuse it. Reopen reconciles missed arrivals; starting a Task never
  unexpectedly opens a closed host app. Missing observations retain native work.
- Presentation move/copy does not change Task Wave membership or execution owner.
  Native reorder preserves processes. Cross-container/host materialization retains
  source shells, jobs and unsaved Files; conversation handoff is explicit after
  destination preparation. Failure leaves source input intact. A Wave move cannot
  acquire every conversation's input. Named copies observe existing Sessions;
  fresh ordinary shells require explicit counted creation. Further copies are
  distinct occurrences; reopening a copy reuses it.
- Partial outcomes remain per-item and inspectable: created/reused/display
  requested/unavailable/unsupported/uncertain. Lost create replies cannot cause
  blind duplicate retries or destructive rollback. Server materialization does
  not establish app visibility or input readiness.

## Shared tools and implementation

### Native new-split directory — accepted November 3

Jack Heart's comment `691eb6f2-0bcc-4054-8625-536f1de23feb` requires splits
subsequently created through a host's own shortcut, menu or
native API in an LF-opened Task workspace to start in that Task's recorded
worktree. Setting cwd only on the panes LF initially creates is insufficient.
This covers Rex and the other supported adapters, including Desktop and bounded
Herdr. A Wave session can contain several Tasks: resolve the source Task tab/pane,
not one Wave/session-wide cwd. New splits must not land in a sibling Task, main
checkout, provider working directory or home. Keep execution on the recorded
Machine for remote Tasks. The default is the Task worktree root; explicit directory
choices remain explicit host actions.

Implementation needs supported per-occurrence split inheritance without changing
global preferences or unrelated work. Acceptance covers native splits beside
Conversation, Shell, Files, Changes and Task/workflow tools: two Tasks in one Wave,
switching between them and checking actual new-shell pwd/checkout identity, then
reopen and supported remote paths. An unavailable scoped policy is a host capability
gap requiring explicit evidence, not an accepted reduction in scope.

Rex code supplies cwd only in created block options; Herdr also supplies
workspace.create cwd. Passive Rex arrivals now validate and use the recorded Task
checkout before creation, just like activated tools. Activation preserves that
block. This fixes the initial pane, not subsequent native split inheritance.

The isolated `scratch/rex-host-probe.py` now creates two Task directories in one
Rex session, then calls native `session.new_split` with each source block as anchor
and no explicit cwd. Both real new shell processes start in the isolated HOME,
not their source directory, verified by each shell writing `pwd -P`. The installed revision-8 public schemas for
`session.create`, `session.new_window`, `session.new_split` and the terminal creator
offer per-new-block `options.cwd`, but no per-window/session split-default setter;
the complete method inventory exposes no such operation. Config is account-wide
Lua, excluded from this design. This is a counterexample to accepted native API
acceptance, not permission to substitute an LF-only split command. Keyboard/menu,
Herdr, Desktop and remote parity remain unproved; broader repetition cannot fix
the missing native API policy. Supported host changes or a revised contract need
Jack Heart's judgment before dependent implementation continues.

| Intent | Shared meaning | Branch implementation |
| --- | --- | --- |
| Conversation | Existing LF Session and provider history | `session view` observes a live owner or reads history; explicit resume owns input |
| Shell | Native shell at the Task's recorded location | `task tool … shell` execs a login shell; Rex owns it |
| Files | Selected directory/file, with source drafts retained | `LF_FILE_BROWSER` for directories, then VISUAL/EDITOR/vi; Desktop Files retained |
| Changes | Exact working-tree, branch or PR comparison | Shared comparison DTO; pinned base/head for branch/PR; Git pager or explicit difftool |
| Task & workflow | Brief, workflow position, PR, associated runs and selected step result/log | `task view` reads shared Task status; n/p select step, l reads captured log |

The portable `WorkPaneLayout` remains the single model: Session/Shell/Browser
panes, with Files/Changes/Task/GraphLog/URL Browser tabs. Five semantic tools do
not require five top-level pane kinds. Rust and Swift consumers and DTO fixtures
were updated; URL tabs, drafts and background selection remain stored. Terminal
rendering of a selected URL is unsupported and reported without changing source
state. Graph-log intent uses the Task viewer's run/log navigation.

`TaskLayoutTemplate` stores revision/tree/hidden in `wave_task_layouts` and
instantiates through the shared pane reader. Templates carry no concrete Session,
location or file drafts. Saved Task intent wins. `desktop layout wave-default:<id>`
reads/writes the template. The Task's single draft migration is `rex_workspaces.sql`.

`task tool` pins the recorded checkout. Git remains responsible for pager and
configured difftool selection; normal patch viewing includes untracked files,
while difftool is a tracked-file operation. File editor settings can be shell
commands but resolved paths remain separate positional arguments. A configured
GUI editor may open externally. Native blocks launch direct argv, never typed
commands in an existing terminal. Task graph position and associated Graph
process history stay separate; the latest run grants no control authority.

## Host implementation and evidence

`open/workspace.rs` uses shared selection and execution routing, persists host
occurrence IDs and uncertain-creation receipts, and serializes CLI/follower
creation. `layout.rs` renders semantic intent; `herdr.rs` owns its bounded native
socket adapter. The unsupported Rex branch and Herdr keystroke-based resume
path were removed. Workspace paint never executes `ViewContent.argv`.

Rex explicit Task entry can reuse an existing occurrence or the caller's session.
Repository entry groups by Wave, with one native session per execution Machine.
Remote opening dispatches through that Machine's existing LF route. The native
Workspace tab follows local shared selection; explicit open reconciles misses.
Named copies omit ordinary shells unless `--new-shells` is selected. These are
source/fixture facts, not complete native acceptance.

Installed public Rex API evidence (`0.0.1793377904-ge6592f`, revision 8):

- Sessions contain windows/tabs with split block trees. Creation starts new
  blocks; detach/reinsert can preserve a live block. Detaching the final block
  removes its window, so detaching everything is not a safe hide implementation.
- `session.move_window` reorders within a session; `move_block` also needs a
  placed anchor in that session. Cross-session live-tab migration is unproved.
- The isolated restart probe preserved session/window/block IDs but replaced
  `/bin/cat` with a fresh login shell. Its launch counter stayed at one. One
  immediate stop lost a newly created session; a two-second settling interval
  retained it. Neither crash-durable save nor process continuity was established.
- Native creation falls back home for invalid cwd; LF validates recorded checkout
  before creating tools. `shell:"none", command:[…]` supports direct argv.
- `client.perform(session.select)` can request app display. Multiple/no capable
  clients, disabled control, app refusal and remote enrollment remain distinct
  from successful server creation. `client.inspect` attachment and a session's
  active window do not establish foreground use or a person leaving a Task.
- The native caller exposes REX_SESSION/REX_BLOCK without REX_SERVER. LF asks Rex
  for its configured endpoint and checks the caller block. A custom server whose
  configured default differs still needs `--server`; that falls short of the full
  accepted bare-open requirement, though failure stays on the Rex route.
  The isolated non-default-server probe reconfirmed `REX_SERVER=unset` in native
  children. Neither that context nor the public session IDs supplies the socket.
  The existing current-viewer route can use a recorded endpoint, but the first
  direct call in an arbitrary custom server remains unresolved; a process/socket
  scan or a guessed default is not accepted destination evidence.

The isolated native test covers bare open without REX_SERVER, preserved renamed
windows/blocks, repository scope overriding an ambient Task checkout, passive
first-start arrival without focus change, and explicit reopen reusing the arrival.
It creates zero LF Sessions. Reusing a placeholder is not restoration of its tools.

Herdr 0.9.3/protocol 22 evidence supports a local explicitly selected Task:
create, reuse, focus, native close/reopen and direct-argv tool layouts. Its isolated
check covers create/reuse/close with zero LF Sessions. Wave following, remote
forwarding, move/copy parity and restart recovery are not implemented. Herdr
metadata has a TTL and moved panes can change IDs; neither owns durable LF
identity. Full secondary lifecycle proof remains with integration/demo.

## LOO-717 lifetime coordination — revised November 4

Jack Heart superseded the earlier equal-host shell-retirement design:

- LF automatically cleans up only Desktop-owned shells. Rex owns its shells.
- A retained Rex terminal for a Task Session is a retention hold on that Task's
  resources. It counts even when hidden, unfocused or navigated away from.
- Confirmed removal of the last relevant Rex terminal releases that host's hold;
  all other holds and ordinary LF cleanup conditions still apply. Missing or
  failed inventory is unknown, not release. Requested work must finish first.
- Rex observation does not mutate LF layout state. A destination ID by itself
  is not proof that a terminal still represents the same Task Session after
  native restart or reuse; observe the association without replaying commands.
- No Rex close operation, away timer, native settlement handshake or second
  cleanup owner belongs in this adapter. Shared execution lifetime and checkout
  cleanup remain with LOO-717. This does not change Desktop's accepted policy.

The November 3 `task presence` API and native-settled proposal are historical
integration evidence, not the revised contract. Inspect LOO-717's current owner
before integrating the smaller retention observation; its earlier publication
alone proves neither current API availability nor configured behavior. Creation receipts and native IDs are not live retention holds.

## Delete — do not maintain

The remaining occurrence locator is Task-specific: `task_id` replaces the
duplicated repository/kind/id in `work_json`, and SQL reads it directly.
Herdr's unused tab occurrence record and whole-record serde implementations are
removed; Herdr reuses its workspace ID and preserves the pending layout receipt.
The six host-record revision triggers are removed and the tables classified as
operation-only receipts: opening a native tab does not invalidate shared Work.
Wave-template revision triggers remain because those defaults affect layouts.
`docs/workspaces.md` now owns the workspace guide; the duplicate surfaces section
(including its superseded native-closure requirement) is removed. No accepted
named-copy, passive-arrival or uncertain-create behavior was dropped.

Removed in the November 4 cut: workspace `observation` (new/closed/retained/unknown)
and occurrence `observation`/`updated_at` from `rex_workspaces.sql`,
`store/sqlite/host_workspaces.rs`, and both native adapters. Keep native IDs for
rename/reopen and named copies, a pending container-create receipt, and only the
four tool-creation outcomes needed to prevent replay. Missing native IDs are
judged from inventory; no durable closed or retained state is needed. Preserve
passive one-time tool activation and close suppression while following. Following
remembers arrived Tasks for its subscription so a Wave move cannot copy their tabs.

No Rex away, shell closure or settlement implementation exists in this checkout;
those were planned dependencies, now removed. LOO-717 is still open at PR #1811.
Its current `transport/README.md` requires native-settled on every presence write
and native closure before retirement. That contradicts the accepted smaller Rex
contract; retention integration needs the shared owner to expose that contract,
not a second cleanup implementation here. Native split/default-server capability
gaps remain independently unresolved.

Removed: the Herdr keystroke-resume adapter and its exclusive test; native-window
and container early returns that bypass activation or follower recovery;
`run_host`'s wait-before-draining pipe capture; and the workspace reader's
ID-list/per-ID query round trip. Passive layout rendering now also removes its
early return around checkout validation. The local Task-ID set and second plan
filter are removed: route once and pass the selected contents to rendering.
Retained IDs and recovered markers share one activation path; marker recovery
still grants no replay permission. The builtin layout no longer duplicates
Task & workflow when no conversation exists. No remaining predecessor path needs maintaining.
Preserve native blocks, uncertain-outcome receipts and explicit-close behavior.
No replacement ordinary-shell relay or cleanup authority is introduced.

## Recovery implementation, November 3

Explicit opening activates a passive arrival once by adding the selected tools
beside its existing native panes. The pending activation receipt prevents command
replay after a lost split reply. Subsequent opens preserve the resulting layout.
A follower holds an OS lock for its lifetime; reopening can add a fresh follower
tab after that process exits, without replacing the old native tab. Launch-pending
receipts prevent blind retries. Repository-created Wave containers now also follow
independently. These locks describe follower liveness, never Task retention.
Recorded isolated native checks cover activation once, retained block identity,
repeated no-focus open and follower recovery, with zero LF Sessions created.
Host command output
is spooled to temporary files while running, so native inventories larger than a
pipe buffer cannot deadlock the timed wait. The initial isolated bare-open timeout
exposed that wait-before-drain failure; the host-inventory regression test passed.

Container reuse and creation converge on one follower check. Activation uses the
container already resolved under the occurrence lock. Workspace lookup reads
complete rows in one SQL statement; no cross-statement snapshot or second lookup
is needed. Layout traversal handles hidden panes and collapsed splits separately
from tool selection, with one split-axis translation for the Rex renderer.

## Remaining work and decisions

1. **Template launch and native reuse.** Destination records now contain only
   locators and creation receipts, preserving named-copy reuse and uncertain-create
   safety. The follower remembers arrivals during its subscription instead of
   copying a Task after its Wave membership changes. Rex owns restoration of
   native mutations. Configured restart/relaunch and native-move proof remain;
   familiar IDs must never authorize command replay.
2. **Retention integration.** Integrate the revised LOO-717 boundary above:
   retained Rex Task Session terminals prevent cleanup, independent of focus.
   Confirmed final removal releases the Rex hold; unknown preserves uncertainty.
   Do not implement native Rex closure, away inference or native settlement.
   Automatic shell cleanup remains limited to Desktop-owned shells.
3. **Bare-open and remote coverage.** Resolve non-default Rex caller endpoints
   without required flags. Demonstrate actual LLM tool invocation after viewer
   resume/move, explicit overrides and no local fallback. Existing tests cover
   source placement routing and direct native shell invocation, not that full
   agent path. The local follower skips remote Tasks; newly relevant Machines
   need following/reconciliation, not merely a new explicit open. Real peer/app
   visibility and provider usability remain unproved.
4. **Move/copy and secondary lifecycle.** Named observing copies and explicit
   `lf resume` exist. Coordinated destination preparation/handoff receipts are unfinished. Native reorder and source-state
   preservation need end-to-end proof; cross-host ordinary-process migration is
   excluded. Herdr's bounded scope needs the complete configured return/reopen and
   cleanup demonstration, without claiming unsupported Wave/remote parity.
5. **Native split defaults — capability review required.** The two-directory native
   probe disproves inheritance for the supported Rex API. Initial passive pane cwd
   is corrected, but no scoped host default is available in the inspected API.
   This needs a supported host capability or an accepted design revision; no
   global Lua patch, alternate split command or scope reduction was introduced.
6. **Gate and demo evidence.** The complete select/open → work → return →
   close/reopen path must preserve unfinished input and provider history, with
   no duplicate provider or replay. Concurrent/partial/uncertain outcomes must
   preserve unrelated panes. Gate owns affected suites and migration/DTO coverage;
   demo owns configured interaction and the exact unresolved product choices above.

## Verification

Review finding: Rex's background `process` reply retained the root-owned `login`
wrapper even while captured output showed the running Task viewer. The native
fixture now records cwd in a launcher that immediately execs the real viewer;
native telemetry and displayed checkout text cannot substitute for that proof.

Earlier focused passes recorded in `cc607dc3e1` remain evidence for unchanged
code: 15 work-open tests, 2 workspace unit tests, observer/editor/Task-view/template/
committed-diff checks, 22 Swift WorkSelection/RegistryQuery tests, format/clippy and
both opt-in native host tests. Rex passed outside the network sandbox; inside it,
its caller produced no receipt or exit/error file before the deadline. Herdr passed
inside. That sandbox failure remains unresolved and the real-agent/remote path
has not been demonstrated. `af774cada6` records the later 17-test work-open pass
and template check. At `3d61147152`, the work-open suite passed 15 tests and
both isolated opt-in Rex/Herdr tests; the Task-view test, formatting and clippy
also passed. The Task-view link emitted a macOS unwind-size warning. The native
probe established split HOME fallback and restart command replacement, not
configured acceptance.
Check (compress, November 4): `cargo test -p loopflow --test integration work_open_tests:: -- --test-threads=1` — 15 passed, 2 native checks ignored, 2 host-fixture deadline failures (`available_conversations_need_no_question_and_are_never_taken_over`/Launch Services and `cmux_opens_the_task_conversation_through_session_resume`, both 2s). Both isolated Rex/Herdr checks passed in the preceding `--include-ignored` run; revision coverage, fmt, clippy with warnings denied, migration history and diff checks passed. Gate owns the intermittent host deadlines and broader acceptance; native split, retention integration and configured agent/remote routing remain unfinished.
