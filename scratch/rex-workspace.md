# Open and restore a Loopflow workspace in Rex

**LOO-498 · concept review revised November 5, 2026. Current direction is below; implementation still uses the superseded host registry.**

## November 5 interactive concept review — current accepted design

**Execution authorization:** after this review Jack Heart said “ok, pursue.” Follow
the pursue skill in the ongoing conversation: implement, compress, sync, realign,
review/iterate, concept-review, review/iterate, then PR publication. This supersedes
the review-only boundary of the preceding turn. Preserve Task/checkout/PR identity;
no Auto-Ship, merge or shared-store migration is authorized. No live Graph was
observed before launch.

**Reversible implementation choice:** new Tasks arriving in a live repo/Wave
container receive their normal configured split layout without focus theft or new
providers. This follows the clarified normal Task-tab hierarchy and the requested
pursuit; the earlier preview-only implementation is removed. Ordinary selected
tools may start. Keep this choice visible for the authored demo.


Jack Heart explicitly rejected LF persistence of hosts, host workspaces and host
work occurrences anywhere. Moving those records to sidecar files, config, or
another LF store does not satisfy the decision. Rex owns its native workspaces.
Jack selected **fresh launch on every `lf open`**; returning to existing work is a
Rex interaction. He then clarified the vocabulary: a Wave **or repository**, depending
on the entry context, maps to a Rex session; a Task maps to a tab; a workspace is
its set of splits. Jack clarified that “loop” was voice-to-text noise: only Task → tab is intended.
There is no additional loop, Graph-tab or conversation-tab concept. This supersedes the earlier reopening/named-copy-reuse and durable
creation-recovery contracts below. It does not authorize implementation in this
concept-review turn, publication, installation or merge.

### The experience and its three owners

1. Declare a default Task layout in configuration, including Wave-scoped shared or
   local preferences. These describe how new work should start.
2. Arrange a Task in Loopflow Desktop. Its current sticky layout stays in LF's
   existing `work_pane_layouts` database storage.
3. Run `lf open --host rex --task LOO-498`. Read that Task's current Desktop
   layout, otherwise its resolved config template, otherwise the builtin. Create
   a fresh Task tab with those splits in the current/explicitly selected native
   session, as faithfully as possible, using the Task's actual Machine, checkout
   and existing conversation identities. Without a selected container, create one
   for the requested repository/Wave scope (proposed targeting default). Fresh
   workspace means fresh Task tab/splits, not necessarily a new Rex session.
4. Rearrange, rename, return to or restore that workspace in Rex. LF keeps no
   host workspace identity, copy record, native arrangement or creation journal.
5. Run the command again to deliberately launch another workspace from the current
   LF source. It neither finds nor modifies the previous one.

**Accepted:** config templates; Desktop live layouts in the DB; external-host
ownership; each open launches fresh. **Implementation proposal:** use the existing
configuration resolver and shared semantic pane intent, with whole-layout replacement
at the winning config scope rather than merging branches of split trees. Shared
Wave config lives in `.lf/wave/<name>/config.yaml`; private overrides use the existing
Wave scope in `lf.config.local.yaml`. Repository/global defaults use normal config
scopes. Exact setting key and serialization are not selected or implemented here.
Do not invent another configuration file or a host-specific layout language.

### Actions → concepts → APIs

| Action | Concept / owner | Operation |
| --- | --- | --- |
| Set new Task defaults | Authored template / config | Existing config read/write path, extended with layout setting (not implemented) |
| Arrange this Task in Desktop | Current Task layout / LF DB | Existing Task `desktop layout` read/save path |
| Open it in Rex | One launch / native host | `lf open`; read source, compile in memory, native create, report result |
| Return, resize, rename or restore | Native workspace / Rex | Rex's own UI/APIs; no LF writeback |
| Open again | Another explicit launch | Same `lf open`; creates another Task tab and split set, potentially in the same native session |

### Consequences for the next implementation pass

- Remove `HostWorkspace`, `HostOccurrence`, `HostCreation`, their SQL readers/writers,
  and LF-side host/copy indexes. Do not move them elsewhere. Replace durable
  host-addressed observer/retry records with operation-scoped acknowledgements or
  existing LF Session transport where needed; readiness must not reconstruct a
  host registry. Live execution transport still needs its actual endpoint and
  attachment identity; that is LF execution state, not a saved native workspace.
- Move Wave templates from `wave_task_layouts` into normal configuration. Keep
  current Desktop Task layouts in `work_pane_layouts`, including sticky selections
  and drafts. Templates contain tools/arrangement, not concrete Sessions, live
  process IDs, locations or drafts. Resolve those from the Task at launch.
- Remove the unmerged `rex_workspaces.sql` draft after its consumers are replaced:
  all three added tables and their triggers are unnecessary under this model.
  No production DB rollback or shared-store migration is authorized. Preserve the
  existing private demo snapshot/history; it may contain the superseded draft.
- `--copy <name>` no longer names reusable LF state. The extra launch is already
  the copy. Simplify that API and `--new-shells` accordingly; ordinary Shell tools
  in the selected layout create fresh shells. No copied shell process state.
- No automatic retries after uncertain native creation, and no durable pending
  creation record. Report that a workspace may have appeared; inspect/return in Rex.
  A subsequent explicit open means another fresh launch, even if the prior one
  succeeded. Creating views never restarts/replays an existing provider.
- Started, unfinished Tasks and passive automatic arrivals remain accepted.
  A live follower may keep only its current invocation's native target and seen
  Tasks in memory. It may append new Task tabs to that session while alive; no
  persisted host registry or restart reconstruction. Closed tabs stay closed within
  that live follower. Opening a repository/Wave again launches its current working
  set; explicit current-session targeting can keep the container.
  Native restart and any follower command restoration belong to the host; LF
  does not repair/replay tools into restored tabs.
- Retention remains required: actual retained Rex Task Session terminals protect
  checkout cleanup, unavailable evidence never releases, and LF closes only
  Desktop-owned shells. The current occurrence-table-based `hold::task_hold`
  cannot be retained. Establish evidence through the host and LF Session lifetime
  owner without storing a parallel host inventory. If retained stopped terminals
  cannot be associated reliably through those sources, expose the capability gap;
  do not silently weaken protection or retain the rejected records. LOO-717 owns
  cleanup; this review does not select a replacement presence API.
- Preserve explicit input transfer through the existing LF Session owner, remote
  routing, agent-tool caller inference, native split cwd, five shared tools,
  Herdr's bounded adapter and Desktop behavior. New views stay passive.

### Normal path, failure and recovery

**Normal:** open → use/rearrange in Rex → return through Rex. Second open creates
another workspace; first workspace and its programs are untouched. Template edits
apply to later launches when no saved Task layout takes precedence; saved Desktop
Task state does not mutate merely because it was read for Rex.

**Failure:** native create succeeds but its reply is lost. LF reports uncertainty
and performs no blind retry. Rex is the place to find whatever appeared. Another
explicit launch may create another view, never another provider or replayed turn.

**Restart:** Rex restores what it supports, potentially with new ordinary shells.
LF stores no restoration recipe or native IDs and does not infer process continuity
from restored geometry. A fresh open reads LF's current source again.

**Counterexamples to test:** renamed or edited native workspace must remain
untouched by a later open; closed arrival tabs must not immediately reappear;
a stopped retained Task terminal must not lose cleanup protection just because
there is no LF host row; source layout drafts must not leak into executable argv.
A saved Desktop layout overriding a newly edited default is intentional current
precedence, not a host synchronization bug.

### Native hierarchy and automatic arrivals

Jack Heart clarified: “a wave equates to a session in Rex, and a loop and a task
equate to a tab”; “what I'm calling a workspace is basically a set of splits.”
He then clarified the session container can be **a Wave or a repository**, depending
on context, and that “loop” was voice-to-text noise. This supersedes treating
“workspace” as an LF-owned host container.

| LF selection | Rex presentation |
| --- | --- |
| Repository or Wave, chosen by entry context | Native session |
| Task | Native tab |
| Task workspace | Set of splits in that tab |

Working interpretation stated in the conversation: newly started Tasks arrive as
ordinary Task tabs with their configured splits, without taking focus or starting
another provider. This removes the preview/expand distinction. It can start ordinary
Shell/Files/Task tool programs selected by the template; earlier preview-only policy
is superseded **if this interpretation is accepted**. Jack's hierarchy statement
is explicit; full tool materialization is the interpretation, not a verbatim choice
of the earlier option. Keep this distinction visible until reviewed.

Container targeting is a native action resolved from caller context or explicit
selection, not a durable LF record. Normal Task opening appends a fresh tab to that
container. Opening a whole repository/Wave can create the container for that scope.
Exact targeting defaults remain a proposed implementation detail; no host discovery
or stored mapping may silently turn a fresh Task launch into reuse of its old tab.

### Known hosts — November 5 follow-up

Jack Heart requested checking Herdr/cmux against the same concepts and automatically
using recognized hosts. Extend the fresh-layout design to known workspace-capable
hosts; earlier cmux-workspace exclusion below is superseded for this direction.
Do not claim equal remote/lifecycle support from a grouping match.

| LF meaning | Rex | Herdr | cmux macOS |
| --- | --- | --- | --- |
| Repo/Wave container | Session | Workspace | Workspace group (or native window when explicitly targeted) |
| Task | Tab/window | Tab | Workspace/sidebar item |
| Task's set of splits | Block split tree | Tab pane layout | Workspace pane layout and terminal surfaces |

Evidence read November 5: installed Herdr **0.9.3**, bundled protocol **22** schema
has `workspace.create`, `tab.create`, `layout.apply`, `pane.current`. Installed
`/Applications/cmux.app/Contents/MacOS/cmux` is **0.65.0 (108), dda24fbd2**;
`workspace-group --help` confirms groups, and `new-workspace --help` confirms
`--group`, `--layout`, `--cwd`, caller-window selection and `--focus false`.
The cmux CLI is bundled but not on PATH. Discover supported installed entry points
without requiring a PATH change. No native resources were created for this review.

Sources: [Herdr concepts](https://herdr.dev/tui/docs/concepts/),
[Herdr CLI](https://herdr.dev/tui/docs/cli-reference/),
[cmux concepts](https://cmux.com/docs/concepts),
[cmux groups](https://cmux.com/docs/workspace-groups),
[cmux API](https://cmux.com/docs/api), and
[cmux CLI contract](https://github.com/manaflow-ai/cmux/blob/main/docs/cli-contract.md).
These establish documented/installed capabilities, not composed LF acceptance.
The cmux “surface tab” is inside a split region and is not the Task-level match.

**Detection contract:** explicit host selection wins; otherwise use the current
attached viewer for an agent caller, then the actual calling terminal. Recognize
Rex, Herdr, cmux and Desktop through their native context and APIs, and prefer the
innermost host when multiplexers are nested. Native pane lookup resolves moves;
stale launch environment and whichever app is focused cannot choose a different
Task destination. If no host context exists, use the configured/default route;
installed/running alone does not mean the caller is inside it. Endpoint or capability
failure stays attached to the selected host, rather than silently opening elsewhere.

The existing `open.rs::route` already recognizes Rex/Herdr/cmux/tmux/Desktop and
uses LF Session viewer placement for agent callers. It treats Rex failure as final
but still builds a fallback chain for other hosts. Revise the shared resolver,
not an additional discovery service or database host catalog. Retain existing
conversation-only routes for hosts without a workspace renderer; report that limit
when a full layout was requested. Implement fresh renderers against the host's
real grouping/split API, not literal Rex vocabulary. Native session/group handles
live only in the operation or active follower. Config can express a default host
preference; that is not a persisted inventory of host instances.

Next proof must include current-host opening in all three hosts, nested-host
selection, an agent caller after viewer movement, explicit override, and unavailable
host behavior. Reuse shared source-layout selection and execution routing. Do not
claim layout parity, native cwd inheritance or remote command safety merely from
successful detection or CLI help.

### Review result and next proof

Design changed. No unresolved choice about fresh launch remains: Jack selected it
explicitly in this conversation. “Loop” is resolved as transcription noise; the
normal-Task arrival interpretation above remains visibly identified. Docs are drafted in `docs/workspaces.md`, and the
page is `scratch/concept-review.html` (local-file browser opening was blocked by the
browser protocol policy earlier in this conversation; provide the file link, without
a workaround). No builtin skill currently teaches workspace
tracking; keep generic Work/operator skills unchanged. Proposed guidance for any
future workspace-opening instruction: "`lf open` launches a new native workspace;
return through the host. Read LF layouts/config as input, and keep no host registry."

The next implementation pass must remove the rejected persistence, implement config
templates and prove two successive launches leave independent native arrangements
without creating or replaying providers. Verify automatic and manual native Split
cwd, live arrivals without focus theft, failure reporting and shared retention.
The failed configured demo remains evidence in `workspace-demo.md`; this concept
review neither fixes nor accepts it. No new execution was launched here.

## Earlier design and implementation evidence

The sections below preserve prior rationale and verification. Wherever they depend
on LF host records, Wave templates in SQLite, named-copy reuse, repeat-open reuse,
or durable uncertain-create recovery, they are superseded by the accepted review
above. They are not next implementation instructions. Other accepted requirements
and evidence limits remain in force.

## Earlier accepted direction

### November 5 — directory behavior for automatic and manual entry

Jack Heart requested fixing directory tracking and native splitting for automatic
opening and Tasks started manually in existing terminals, without a new Rex scoped
default, global configuration or a Task-root lock. The trace and repair are under
[Native split defaults](#native-split-defaults). This continues LOO-498 and
authorizes neither shipping nor merge.

Jack Heart's November 3 outcome is familiar Loopflow Work in Rex, with existing
Task Sessions and provider history. Rex is primary; Herdr is a bounded secondary
adapter. The LF Task/workflow terminal view is included. cmux parity, Mobile, a
Lua runtime and a generic multiplexer platform are outside this build.

Jack's build approval `7088ad9c-c923-4fff-acd7-0a261921b0f0` superseded the
mistaken-launch stop. His November 4 approval
`f627e6a4-9129-40ca-9276-126af7e6da55` selected the smaller ownership boundary:

- LF owns its saved layouts and launch templates. External hosts consume them
  one way, reproducing the arrangement as faithfully as their tools allow.
- Rex owns subsequent native edits, shells, persistence and restoration. It never
  writes its split tree back to LF. Desktop keeps its own layout persistence.
- Reopening an existing destination preserves its programs, arrangement and input.
  Confirmed absence permits explicit creation from the LF template. LF does not
  repair missing programs inside retained native tabs or replay launch commands.
- A retained Rex terminal for a Task Session prevents Task cleanup regardless of
  focus, visibility or navigation. Confirmed removal of the last relevant terminal
  releases that hold subject to other holds and normal cleanup conditions.
  Failed or unavailable host inventory is unknown, not release.
- LF automatically cleans up Desktop-owned shells only. Rex away detection,
  LF-initiated native closure and the native-settlement handshake are superseded.
  Shared execution/checkout lifetime remains with LOO-717; requested work finishes
  before cleanup. Closing unfinished Task views does not delete their checkouts
  or branches. Automatic deleted-checkout reconstruction remains outside scope.

Removing named-copy reuse and coordinated destination preparation/handoff is a
separate **unaccepted proposal**. Their earlier commitments remain. Loose geometry
fidelity does not relax Task, Machine, checkout or conversation identity, native
split cwd, passive arrivals, Wave defaults or agent-tool routing.

Jack requested documentation before further implementation/publication, then moved
its destination to [docs/workspaces.md](../docs/workspaces.md). That guide is the
user contract; implementation gaps are marked explicitly. Review publication and
the authored demo are authorized; Auto-Ship, merge and Task completion are not.

## Work selection and interaction

| Concept | Responsibility |
| --- | --- |
| Work selection | Existing repository/Wave/Task selection; started unfinished Tasks by default, backlog explicit |
| Launch template | Saved Task intent → Wave default → builtin; visible selected tools and initial arrangement |
| Host destination | Native IDs locate existing views; the host owns arrangements after creation |

Repository entry through `lf open` groups Tasks into Wave sessions plus a standalone
group, partitioned by execution Machine. A Task is a native window/tab with tool
splits. Wave conversations are included only when already present. Explicit Task
entry does not follow siblings. Explicit backlog entry shows the brief without
creating execution. Opening creates no provider, submits no turn, starts no Task
Graph and takes no input. Conversation viewing is separate from explicit resume.

Bare `lf open` inside Rex must resolve the actual server/session and Work location,
including an LLM's tool shell and resumed/moved conversations (Jack's November 3
comment `288a39bd-1059-4778-b4aa-8bf5bec26fb4`). Current viewer placement outranks
stale provider startup environment; explicit host selection overrides inference.
Known Rex or remote failures cannot fall through to Desktop or local/home execution.
Running-app or socket scans and guessed endpoints are not destination evidence.

Broad workspaces add first-start Tasks passively, without focus theft, providers
or ordinary tool startup. An explicit open adds a passive arrival's tools once,
beside its retained native panes. Reruns reuse it. Following only appends: it does
not move tabs after Wave changes, remove completed Tasks or repair native programs.
Closing a Task tab stays respected while following. Explicit broad reopening
reconciles missed arrivals. Starting a Task never opens a closed host app.

Presentation move/copy changes neither Wave membership nor execution ownership.
Native rearrangement preserves processes; cross-container/host materialization
leaves source shells, jobs and unsaved Files in their original host. Conversation
handoff is explicit after destination preparation; failure leaves source input
intact. A Wave move cannot acquire every conversation's input. Named copies observe
existing Sessions and reuse their destination on repeat open. Fresh ordinary
shells require explicit counted creation (`--new-shells`); another name creates
another occurrence. Shell variables, jobs, editor memory and unfinished input
are not portable layout state.

Partial outcomes remain per-item and inspectable. Lost create replies cannot cause
blind retries or destructive rollback. Unrelated panes survive failure. Native
materialization and accepted display requests do not establish app visibility or
conversation readiness.

## What exists in this checkout

`open/workspace.rs` consumes shared selection/execution routing and serializes
native creation. `HostWorkspace` stores destination/selection/copy identity and a
pending-create receipt; `HostOccurrence` stores Task identity, destination and
`HostCreation` state. Neither owns native
liveness, layout or retention. Herdr keeps its workspace locator and pending layout
receipt. The one draft migration is `rex_workspaces.sql`; only Wave-template writes
advance shared Work revisions.

Retained tabs are unchanged; confirmed absence permits explicit recreation. Marker
recovery recovers identity without permitting command replay. Passive activation
adds selected tools once beside existing panes; uncertain split replies fence
retries. The follower's OS lock establishes its liveness; a later open may create a
new follower tab without replacing the old one. Following remembers arrived Tasks,
and repository-created Wave containers follow independently.

Shared tools render directly into each host's format. Rex launches carry the resolved
endpoint, including Wave conversations and followers. Host output spools during
commands to avoid pipe deadlocks; workspace lookup uses one query. Passive creation
validates the recorded checkout. The old Herdr keystroke-resume path is deleted;
painting never executes `ViewContent.argv`.

Caller and recorded-viewer placement resolve their endpoint once before caller
block validation. Local and remote opening share `OpenReport`; destination
preparation and tool creation use the same per-Task failure receipt, preserving
other results. Explicit Task reuse keeps its existing container across membership
changes. Test peers share JSON reply setup; uncertain replies stay raw failures.

Remote arrival, caller discovery and their evidence limits are described below.
The adapter now reports each Task's Rex hold (see the concept review below); the
shared lifetime owner does not yet consume it.

Live observers now acknowledge preparation before offering Ctrl+R to request input
on the same connection. The existing execution owner records destination placement
with that explicit claim, preserving the provider attachment token and routing
subsequent agent tools to the destination. Passive copies cannot change placement.
Preparation failures and unanswered transfer requests never submit input or retry.
Task and Wave conversations share `Host::conversation`, retaining history/errors
after exit through Rex's native `exit.on_completion=false`; ordinary shells retain
their normal native behavior.
Review found and repaired both disappearing preparation errors and stale routing
after transfer. Configured provider proof remains owed.

**Readiness (November 5).** Each `OpenReceipt` carries `conversations`: per-Session
`connected · history · ended · failed · uncertain`, separate from native `status`. The
viewer alone reports it (`session view --observer <key>`), writing a record under
`LF_HOME/open/observers/<destination>/` and holding its OS lock, as the follower
does, so a record cannot outlive a host restart. No schema change. Opening waits
at most 3s, once, for observers it launched. `--retry-observers` adds a new observer
beside the retained pane (a new Conversation window for Wave containers). A record
stays `pending` until the native reply, which fences duplicates after a lost reply
until the observer itself reports. `retry_argv` is rewritten to the source Machine's
command for remote receipts. Exit status still follows native results only.

### Shared tools

| Intent | Current implementation |
| --- | --- |
| Conversation | `session view` observes live output or history; Ctrl+R requests input after live attachment acknowledgement; `lf resume` also remains explicit |
| Shell | `task tool … shell` execs a native login shell in the recorded Task checkout |
| Files | Directory `LF_FILE_BROWSER`, then VISUAL/EDITOR/vi; Desktop retains Files documents/drafts |
| Changes | Shared working-tree/branch/PR comparison; pinned base/head, Git pager or explicit difftool |
| Task & workflow | `task view`: brief, graph position, associated runs/results; n/p selects a step, l reads its log |

`WorkPaneLayout` remains the single portable intent model: Session/Shell/Browser
panes, with Files/Changes/Task/GraphLog/URL Browser tabs. Rust, Swift and DTO fixtures
carry the same shape. URL tabs, drafts and background selections remain stored;
terminal rendering reports a selected URL unsupported. Graph-log intent uses the
Task viewer's run/log navigation. Graph position and associated run history remain
separate; latest run grants no control authority.

`TaskLayoutTemplate` stores revision/tree/hidden in `wave_task_layouts`, without
concrete Sessions, locations or drafts. `desktop layout wave-default:<id>` reads
and writes it without Desktop running. Saved Task intent wins; template edits
apply to future materialization, not existing native arrangements. Only visible,
selected tools launch. The builtin layout avoids a duplicate Task view when no
conversation exists.

Tools pin the recorded checkout on the execution Machine. Git owns pager/difftool
preferences; normal patches include untracked files, while difftool is tracked-only.
Editor commands keep resolved paths as separate positional arguments. GUI editors
may open externally. Native blocks launch direct argv, never typed shell commands.

## Evidence and capability limits

### Rex

Recorded public API evidence: `0.0.1793377904-ge6592f`, revision 8. Sessions contain
windows/tabs and split block trees. Detach/reinsert can preserve a live block, but
detaching the last block removes its window. `move_window` and anchored `move_block`
operate within a session; live cross-session tab migration is unproved.

Isolated tests cover create/reuse, rename/reopen, repository selection overriding an
ambient Task checkout, passive first-start arrival without focus change, activation
once and follower recovery, with zero LF Sessions created. Pending-create fixtures
cover lost replies and marker recovery. These are not tests of an actual provider's
unfinished input or configured remote interaction.

The remote-arrival fixtures exercise the receiving command locally: correct cwd,
passive creation, repeat reuse, respect for a closed tab, wrong-Machine rejection,
and backlog/missing-checkout handling. A sleep-process stand-in retains its recorded
attachment token and pending-input flag; this proves no attachment mutation, not
preservation of an actual provider's unsent text. The outbound failure fixture checks no local fallback and no forwarding loop.
A connected fixture now runs the actual source follower and destination CLI with
two isolated Machine stores, a destination-only checkout and simulated SSH/Rex.
It disconnects, then drops a successful receipt, and observes reconciliation reusing
the single destination after both failures without local Work changes. It does not
prove configured SSH, native Rex or a real provider's unfinished input.

Restart preserved native IDs but replaced `/bin/cat` with a fresh login shell;
its launch counter stayed at one. One immediate shutdown lost a newly created
session; a two-second settling interval retained it. Neither process continuity nor
crash-durable saving follows. Native IDs must never authorize command replay or
establish that a terminal still represents the same Task Session.

Invalid native cwd falls back home, so LF validates checkout before creation.
`shell:"none", command:[…]` supports direct argv. `client.perform(session.select)`
requests app display; no/multiple capable clients, disabled control and app refusal
remain distinct from server creation. The real-data demo observed a transient
`client.inspect` 404 after successful materialization; successful automatic app
selection remains unproved. Client attachment and selected windows prove no
foreground use, and foreground use is no longer required for retention.

Native children supply REX_SESSION/REX_BLOCK without REX_SERVER, including on an
isolated non-default server. LF-created tools carry the resolved endpoint through
argv; current-viewer placement still outranks provider startup environment.
Unqualified `rex whoami` with native caller variables retained identified a different
endpoint in the November 4 probe, so those variables alone remain insufficient.

Native caller discovery now reads the actual ancestor Rex
server's explicit `--listen` argument, preserving argument boundaries and excluding
process environments. The isolated process probe confirmed Rex → login → shell
ancestry. There is no running-app or socket scan. Explicit server selection still
wins, and opening validates the caller's block at the resolved endpoint. The native
fixture rejects unqualified Rex CLI requests instead of injecting its endpoint;
it passed custom-server bare opening, rename/reuse and passive arrival with zero
LF Sessions created. Missing ancestry now returns an actionable `--server` error
before querying the configured default. A CLI fixture moves a simulated agent's
viewer between two Rex endpoints while preserving stale startup variables; each
open reaches the current endpoint without changing the Session attachment token.
This does not prove the actual LLM tool path after resume/move, or discovery when
the server listener cannot be read.

### Native split defaults

Jack's November 3 comment `691eb6f2-0bcc-4054-8625-536f1de23feb` requires native
shortcut/menu/API splits to start at the source Task's worktree root on its recorded
Machine. Two Tasks in one Wave produce their own directories, including after
reopening; explicit host directory choices remain explicit.

**November 5 trace.** Rex's settings text: new tabs and splits "use the source
terminal's working directory when available. Otherwise, they use the server's
default directory." `rex split` and raw `session.new_split` with cwd omitted do not
inherit; only the app action can, which is why the omitted-cwd result was no verdict.
LF-issued creation already passes cwd. The app's `TerminalMetadata` carries both
`workingDirectory` and `reportedWorkingDirectory`; which it prefers is unobserved.
Rex exposes those sources as block `pwd` (last OSC 7) and `process.foreground.cwd`.

| Pane | Foreground cwd | Report before | Now |
| --- | --- | --- | --- |
| Manual launch/resume from another directory | stale caller directory | Task (or none until more output) | both Task |
| Task view, Files, Changes, history-only Conversation | unreadable (`login` wrapper) | none | reports Task |
| Live Conversation pane | unreadable | Task | unchanged; selected directory on later attach |
| Shell | correct, follows `cd` | none | unchanged, deliberately |
| Shell after a viewer leaves | correct | stale Task | report withdrawn |

Jack's live server showed the mismatch directly: viewers in `loopflow.codex-resume`
reporting Task worktrees. His zsh reports nothing, so ordinary splits must use
process cwd; this is the terminal disagreement he observed, and LF was causing it
for manual launches.

Repairs: the owner publishes its directory to viewers (`Event::Directory`); viewers
enter it; controlling viewers retire their report with an empty OSC 7 on leaving
(Rex clears `pwd`), while retained observers keep it. Later attachments use the
Session's selected directory, so a conversation bound after launch reports its
Task. Reports queued by a read now flush after it: a provider that printed once
and waited left `pwd` empty indefinitely. Rex tool panes announce their directory
at launch; shells do not, since a launch report would outlive `cd`. Machine identity
is unchanged: viewer and owner share a Machine, and reports name the executing host.

### Herdr

Recorded Herdr 0.9.3/protocol 22 evidence covers local explicitly selected Task
create/reuse/focus, native close/reopen and direct-argv layouts. Its isolated check
created zero LF Sessions. Wave following, remote forwarding, move/copy parity and
restart recovery are not implemented in this bounded adapter. Metadata TTL and
moving pane IDs cannot own LF identity. Configured return/reopen and split-default
proof remain incomplete.

### Configured demo and upstream changes

[workspace-demo.md](workspace-demo.md) preserves dated observations. Initial branch
opening failed at schema/catalog compatibility. Later disposable real-data backup
opening/reopening preserved the native layout and created no extra shells; a
manually wired observer socket displayed the existing conversation. Jack requested
real Tasks/Waves/conversations for acceptance; synthetic and manually wired fixtures
remain supporting evidence. No native Split result or observed-behavior acceptance
was obtained.

Main's `dfd454cfc7` (#1815), now merged into this checkout, changes the installer to
keep Desktop and CLI on the same build. It supersedes treating the earlier installer
failure as an unchanged implementation gap. It does not prove installation success,
repair the separately observed branch migration-order diagnosis, or establish a
successful configured workspace demo. The dated demo history remains intact; the
current supported build/install path needs verification before renewed native review.
No migration or installation was performed during this reconciliation.

The synced base also includes #1818 (`ae3c91abee`): resume, rollover and lifetime
share the judgment that an active turn, draft or pending request protects the
existing provider even when attachment fails. Input is retained before delivery.
Workspace handoff must use that Session owner; native destination creation cannot
replace its acknowledgement or justify restarting protected work. This improves
the existing foundation without delivering coordinated workspace handoff.

#1805 (`a5000ef889`) moves preferences to authored files and adds a released
settings migration. Wave tool templates remain in `wave_task_layouts`; they are
saved arrangement intent, not provider preferences. The earlier branch-catalog
failure remains dated evidence, and a compatible current build/store still needs
configured demonstration after this sync.

#1817 (`589b7ac3c4`), in the November 5 sync, makes `pursue` begin by editing user
docs and lets an attached Desktop present them in Files through `lf open --file`.
docs/workspaces.md already follows that order; this realign added Rex to the docs
index and to the `lf open` help text. No workspace behavior changed. The later
November 5 sync (`2e9ffc2aed`, #1825 and #1826) touched settings migration,
installer help and ops docs only; it did not reach workspace code, docs or tests.

The `c6fa1074c0` sync includes #1807 (`c59cfcc631`): promotion/binding
preserves the conversation and immediately updates its selected Task directory,
while an already-running provider may keep its original cwd. Transport now replays
provider launch-directory reports to newly attached viewers. Those reports describe
execution. November 5 work extends those reports to bound conversations through
the existing owner; no second binding or cwd owner was added to the adapter. Gate
owns checks of the combined transport and migration changes after this sync.

## Shared lifetime dependency

Status and source rechecked November 5 confirm LOO-717 remains open at PR #1811, at
demo, unchanged since November 4. Its clean checkout at `b81df60579` still requires `--native-settled`
on every `task presence` write and native closure
before retirement. It also offers exact retaining-process exit observation through
optional owner identity. Those are implementation evidence for the older contract,
not permission to reinstate Rex closure. Client disappearance, lease expiry and
network failure do not establish that Rex's retaining server/terminals ended.
The November 4 coordination comment `04388b59-a9ab-434c-a1d2-c120ec38b7e9`
already conveys Jack Heart's revised contract to LOO-717; that accepted direction
has not yet changed its API. No new cleanup owner or Rex-closure requirement follows.

This checkout has no Task presence API. Retention integration needs the shared
owner to support the accepted smaller contract: a hold independent of focus,
confirmed final release and unknown inventory preserving uncertainty. The concept
review below settles which evidence is the hold. A parallel adapter cleanup loop,
layout mirror or shell runtime would conflict with the accepted ownership boundary.

Desktop's accepted two-minute eligible-idle/fifteen-minute completed-and-away
policy is unchanged. Final requested work, drafts and pending requests retain their
existing protections. Eligible cleanup may discard uncommitted files; moved/unmerged
committed refs retain the shared owner's safeguards. The Rex adapter has no separate
authority to settle or delete those resources.

## Previous remaining-work account (superseded where noted above)

### Concept review (November 5) — next implement direction

The branch now answers two different questions about a Rex terminal with one kind
of evidence, and the plan had reached for a third. Separate them:

| Question | Evidence | Owner |
| --- | --- | --- |
| Does the Task keep a retained view? (cleanup hold) | Its `HostOccurrence` destination is present in a successful Rex inventory | Rex adapter reports; LOO-717 decides |
| Is this view's Conversation live? (readiness) | Observer record plus its OS lock | `lf open` receipt only |
| May LF replay or repair a pane? | Nothing; never | — |

Settled by accepted decisions, not new ones: Jack Heart's contract releases a hold
only on *confirmed removal* of the last relevant terminal, regardless of focus;
unknown inventory never releases. A tab Rex keeps after its observer exits
(Ctrl+], finished conversation, host restart with fresh shells) is still a view the
person retained. Native IDs therefore *do* establish the hold, while still never
authorizing replay or proving process continuity. This replaces the earlier claim
that IDs cannot stand in for holds after restart, and removes the need for a new
terminal-to-Session association: `HostOccurrence` already records Task →
destination. Terminals where someone ran `lf resume`/`lf task run` by hand are
already held by their live attached viewer through the shared lifetime owner; once
that exits the pane is an ordinary shell and holds nothing.

Implemented (November 5): `workspace/hold.rs` reads every Rex occurrence of a
Task (copies and Wave containers included) into `held · released · unknown`;
held wins, unknown beats released, a pending creation or unread inventory is
unknown. `lf open` reports it per Task as `rex_hold` with no cleanup decision.
It finds containers and windows exactly as reopening does (`find_native`: recorded ID, else
the `[lf:<id>]` label marker), so a restored relabelled view still holds.
Check: lib `open::`/`transport::` pass; `work_open_tests::`+`dto_fixtures::` 78/79, the
moved-viewer Rex case passes alone (intermittent, gate); clippy clean.
Observer records stay readiness-only; a stopped observer now reads `ended`,
`failed` only for a reported attachment error. Remaining: LOO-717's shared owner
consumes `hold::task_hold` (or `task status` evidence surfaces it) once its API lands.

**Second pass (November 5, headless) — next implement direction.** The hold
landed on the wrong reader. `lf open` prints `Rex views: held (a retained view keeps
this Task)` on each receipt. That answer is nearly always `held`, because opening
just created or reused the view. It also claims an effect nothing enforces yet: the
docs say retention integration is not delivered, and no cleanup reads `rex_hold`.
The receipt is per destination while the hold is Task-wide across copies and
servers, and each opened Task costs another session/window inventory. Fix, settled
by "Rex adapter reports; LOO-717 decides" and "docs claim only what exists":

- `lf open` text says what the inventory saw: `present`, `confirmed closed`,
  `unreadable`. It makes no keep/cleanup claim. The JSON field keeps its
  `held · released · unknown` values for the shared owner and the demo, and its
  doc comment says it is Task-wide.
- `hold::task_hold` stays the single adapter read. No `task status` field, timer
  or new command until LOO-717's API names its consumer: inventing that read here
  would choose LOO-717's surface for it.
- No other change. Readiness, splits and routing match the accepted contract.
- Implemented (November 5, checkpoint 40cb6dca36); realign reconfirmed the text, doc comment and single reader. Check: `cargo test -p loopflow --lib open::` 17/17, `--test integration work_open_tests::` 31/31 (3 ignored), fmt + clippy clean.

**Third pass (November 5, headless): no change.** Code matches the second pass
(`hold_text`: present · confirmed closed · unreadable; one `hold::task_hold` reader;
docs mark retention integration undelivered). origin/main has nothing past the
`384505b120` base and LOO-717's PR #1811 is still at `b81df60579`, so no new
consumer exists to design against. No open product choice beyond the counterexample below.

Counterexample kept open: a restored but forgotten Rex tab holds a completed Task's
checkout until the person closes it. That follows Jack's "retaining a view is the
reason to keep" direction; if demo shows it is unwelcome, that is a new product
choice for Jack, not something the adapter should paper over with away detection.

### Outstanding acceptance

1. **Retention integration:** shared lifetime owner plus Rex occurrence
   inventory (above) must enforce the revised hold/release contract, including
   unfocused terminals, unavailable inventory and restored tabs. No LF closure of
   Rex shells.
2. **Native split verification:** LF's directory path is implemented and both
   entry paths have native-Rex cwd evidence on an isolated server: automatic open
   (`rex_native_workspace_keeps_window_and_block_identity`: tool `pwd` = Task,
   shell foreground cwd = Task with no report) and manual resume from another
   directory (`rex_manual_resume_elsewhere_reports_the_session_directory_until_it_leaves`:
   block `pwd` and the viewer's lsof cwd = Task, Ctrl+] clears `pwd`, provider
   launched once). Pressing Split in the Rex app from each pane remains owed to
   the demo: it needs a focused app window, and on November 5 this run was inside
   Jack's live Rex with active input, so it was not driven. Which source the app
   reads, and whether it accepts LF's report host, stay unobserved. Herdr, Desktop
   and remote split proof also remain owed.
3. **Bare-open and remote routing:** native ancestor-based custom-server discovery
   passes the isolated host fixture; moved-viewer CLI routing passes with a
   simulated provider. Actual agent-tool invocation after viewer resume/move still
   needs proof. Unreadable ancestry requires explicit `--server`; it no longer
   queries a configured default. Explicit overrides and no local fallback remain
   required. Remote arrival routing/reconciliation and known-server launch propagation are
   implemented. Connected source/destination fixtures cover delivery and retries
   without local Work changes; configured two-Machine interaction still needs proof.
4. **Configured reuse and lifecycle proof:** real open → work → return → close/reopen,
   renamed destinations, native arrangement, unfinished-input/provider-history
   preservation with an actual provider, app selection and remote usability.
   Restart/relaunch without replay passes on an isolated Rex server (November 5).
   Herdr needs its bounded configured return/reopen demonstration.
5. **Move/copy:** readiness, explicit retry and the composed path are delivered.
   `rex_failed_observer_reopens_then_transfers_input_after_acknowledged_preparation`
   runs a real owner and isolated Rex: stalled preparation retained, repeated named
   copy, source input preserved, retry beside the pane, Ctrl+R transfer with the
   provider attachment token unchanged. The provider is a shell stand-in. Native
   reorder/source-state preservation still needs end-to-end proof. Dropping
   coordination or named copies remains unaccepted.
6. **Validation:** gate owns affected suites, migration/DTO coverage and the two
   host-fixture deadline failures below. Authored demo owns configured interaction
   and remaining product judgments; source/fixture passes cannot substitute for it.

## Verification

Gate owns affected work-open, workspace, CLI parsing, migration and DTO suites,
including earlier 2s host-fixture deadline failures in
`available_conversations_need_no_question_and_are_never_taken_over` and the cmux
resume test. Authored demo owns configured interaction and unresolved product
judgments. Native cwd proof uses launcher-written evidence: Rex telemetry can
retain the login wrapper, and stable pane IDs prove neither cwd nor continuity.

Earlier complete plans/results: `cc607dc3e1:scratch/rex-workspace.md` and
`df3b5e810d:scratch/rex-workspace.md`. Earlier checkpoint:
`7b886d04cc:scratch/rex-workspace.md`; dated demo evidence is in
[workspace-demo.md](workspace-demo.md).

Check (November 5 implement → compress → realign): lib `transport::`/`open::` 70, `work_open_tests::` 31, `documented_commands::`/`dto_fixtures::` 52, `prepared_destination` 1 and native `rex_manual_resume_elsewhere…` 1 passed; clippy passed; `transport_tests` hermetic to `REX_*` 67 passed, 1 intermittent (`contended_input_retention_keeps_control_and_delivers_once_after_unlock`, 3/3 alone); native Rex restart, handoff and identity proofs passed one at a time, identity once missed its 15s arrival deadline beside restart. This realign reran nothing: origin/main has no commits past the `384505b120` base and LOO-717's PR #1811 is unchanged at `b81df60579`. Gate owns the combined run and the intermittent cases; the authored demo owns the app's Split.
