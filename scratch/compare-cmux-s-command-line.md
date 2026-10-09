# Desktop control on shared Work — LOO-427

**Status (reconciled October 9, 2026):** Jack Heart authorized implementation through demo
review, in one PR. Recorded checkout location, prototype delegation/routing,
repository-keyed windows, exact retained-pane arrangement and text/key input are
implemented locally. Passive reads validate exact pane/surface identity but pinned
lf2 cannot extract bounded text; the verified native artifact remains required.
Focused local Rust and model evidence establish neither native acceptance nor delivery.

LOO-406 is integrated through main’s `d20c56daf` (#1503). LOO-412 through
`cdec83fe8` was inspected, not integrated: creation recovery and deletion receipts
exist, but retained uncertainty does not yet constrain every delivery path.
Mixed Linear/Git stays disabled pending composition.
Local Task/Session-plus-Changes opening follows request-scoped readiness; native proof remains.
Remaining implementation: Wave-owned planning,
remote/Flow input and broader explanation, shared identity/exchange and first-start
admission, and verified native bounded extraction. Full-path proof remains.
[Findings](findings.md), [questions](questions.md) and the
[comparison](../docs/reviews/terminal-command-comparison.md) retain evidence and open choices.

## Accepted outcome

Jack Heart: Loopflow is “a software engineering CLI for your agents,” used by
agents as much as for using them. Work leads; Sessions/panes support it. Orchestra
language stays top-level. Jack approved the Work/Desktop design; the command map
below incorporates his later review and labels exploratory regrouping separately.
The accepted demo includes CLI-driven Session-plus-diff opening with usable-target
results. Execution on the Mini and presentation on the Mac remain separate.
Started Tasks stay on their Machine across later runs and resumes; delegation
applies to open, unstarted and future Tasks only. No Task transfer is introduced.

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

## Command map — October 8 review

**Accepted for implementation, October 8.** After reviewing this command map and
Desktop/cmux comparison, Jack Heart requested “lets queue implementation for this.”
This remains one PR through demo review, not merge approval. LOO-397's Clap-derived
discovery owns routing. Open questions remain explicit; no follow-up Tasks.

### Target public tree

```text
lf
├── wave        list / show / workflow / chapter operations
├── task        create / list / show / edit / run / move / …
├── session     list / connect / resume / rename / …
├── skill NAME  explicit skill invocation
├── flow NAME   explicit Flow invocation; definition/execution reads remain
├── NAME        convenient unambiguous skill/Flow invocation
├── : PROMPT    inline agent request
├── monitor     ps / top / work / active
├── history     feed (default) / list / show / replay / usage
├── desktop     open / list / focus / split / move / resize / zoom
│               hide / restore / shell / files / flow-log / read / text / key
├── wt / commit / sync / pr
├── machine / account / config / repo / self
└── list / help
```

Unique descendant commands omit parents:
`lf ps` → `lf monitor ps`, `lf top` → `lf monitor top`. `open` is ambiguous
between Desktop and PR (Sessions use `connect`); use `lf desktop open`. Exact owner commands win;
ambiguous shortcuts name their choices and perform nothing. `list`/`show` generally require an owner. Explicit
`skill`/`flow` resolve naming collisions; the shorthand collision policy remains
open. Not every verb exists yet.

Jack favors Wave-owned planning, including Workflow selection, and questions
public `project` and `roadmap`. Proposed: `wave show WAVE` owns the chapter plan;
`wave list` discovers Waves, `monitor` covers current attention across them, and
`task list/show` retains exact historical lookup. Project IDs, chapter history,
KRs and Workflow storage survive. A combined cross-Wave backlog view is optional,
not a requirement merely because roadmap exists today.

Jack questions `run NAME`: remove that duplicate dispatch spelling after its
Flow drivers, skills, examples and tests use explicit invocation. `task run`
continues to advance the captured Workflow; direct invocation does not. Removing
a CLI spelling does not remove the shared execution function or driver.

Jack accepted combining activity and history (October 8): `lf history` is the
chronological feed, with execution details and replay underneath. Remove the
separate activity command; preserve event coverage, exact capture IDs, inputs/results,
filters and paging. Recorded `monitor list/show` and top-level `replay` move here.
Replay starts a new execution. Session-native history stays under Session.
Jack accepted `history usage` for aggregate views of tokens, cost and time.
Reuse existing records; no second store.

Jack questioned both `open` and `desktop`, and dislikes `inspect` alongside
`--explain`. Proposed: `desktop open` owns opening, `desktop list` reads actual
windows/panes. They keep short forms through normal resolution, not duplicate
implementations. `desktop list --json` preserves reading freshness, exact targets,
capability support and legal actions. Composed Session+diff opening must return
opening/usable/failed outcomes; neither command success nor layout mutation proves
a rendered endpoint. Remote viewing stays separate from execution placement.

Jack deferred Discord changes; leave it unchanged.

### Flags: three independent concerns

| Scope | Flags | Contract |
|---|---|---|
| General command behavior | `--explain` (new), `--json`, `--verbose`, `--help` | Explain the selected invocation without performing its effects; structured output where defined; diagnostics; static usage. Current JSON/flag placement is not uniformly global. |
| Repository/execution selection | `--repo`, `--machine` | Select a repository on the execution Machine; never rewrite saved defaults. |
| Work/checkout selection | `--task`, `--wave`, `--wt` | Apply only to operations that consume that scope. Explicit subjects and inferred checkout use one resolver; contradictory selections produce an explanation, never silent retargeting. |
| Agent launch | `--context` (new), `--agent`, `--docs`, `--clipboard`, `--diff`, `--interactive`/`--batch`, `--account`/`--only-account`, `--isolate`/`--shared`, `--chrome`, `--yolo`, `--steers-after` | Applies to agent launches, inline requests and appropriate Flow steps; account/permission behavior stays unchanged. |

Within launch flags, context inputs are `--docs`/`--clipboard`/`--diff`;
agent/account selection is separate from execution settings `--chrome`/`--yolo`.
Jack requests removing `--max-turns` and probably `--no-loopflow`; the draft
removes both public switches while retaining normal operating guidance.
Keep `--steers-after`: Jack recalled context-budget blowups and requested checking
its continued use. Source tracing confirms `flow.rs` records a cursor per node,
passes it through `Cli::step_args`, and `run.rs` applies it in `task_input::read_seed`.
The existing `a_repeated_node_receives_only_task_direction_newer_than_its_last_run`
fixture checks first-visit inclusion, repeated-node omission and later new direction.
It was inspected, not rerun. The map resets with a fresh Flow driver; this is not
cross-restart deduplication. Removing `--no-loopflow` must migrate that fixture's
setup without dropping its context-growth assertions. Retain the advanced launch flag.

Jack explicitly proposes `--context` as “show me the context that would be used
here, dont actually launche”; `--explain` applies to more commands. Neither claim
is an account of existing usage. Examples of intended behavior:

```sh
lf task run LOO-427 --explain          # resolve Work, Machine, action, impediments
lf implement --context                # print assembled agent input, no launch
lf implement --context --explain      # input plus why each source was selected
lf --repo loopflow --machine mini ps  # observe that repository on Mini
```

Explain may read local facts and explicitly addressed peers using existing
transport, but never creates Tasks/checkouts, claims clients, syncs plans, logs in,
publishes or launches agents. Missing evidence stays unavailable. The result is
an observation, not a reservation: execution re-resolves changing facts. Help
explains syntax; explain describes this invocation. Unsupported combinations
fail before effects. `--context` on a non-agent command is invalid; Flow context
preview must define whether it shows the first step or a labeled set before it
is exposed. It cannot execute prior steps to fabricate future input.

`--docs` keeps its existing meaning: add contents from paths, globs or directories
to agent input; it does not generate documentation. Explain can show why those
sources were included. Preserve context budgets, provenance and truncation
reporting while retiring `context --explain` as a separate identity front door.
The standalone budget report's final home remains open.

### Repository lookup

**Implemented locally October 9.** `--repo NAME` resolves `<repo_root>/NAME`;
Machine-local `~/.lf/config.yaml` owns `repo_root` (default `~/src`). Explicit
absolute, `./`, `../` and quoted `~/` paths retain the selected checkout; existing
common-directory identity recognizes symlinks/worktrees. No remembered-name
registry, recursive scan, auto-registration or checkout creation is added.
Explicit `--machine` forwards the selector unchanged and bypasses its saved
repository; names/home expansion happen there. Automatic Task routing replaces
the caller's path with the resolved repository plan ID. Neither changes saved
defaults or joins/publishes plans. Existing opaque `--repository ID` remains the
Work transport's identity selector, distinct from filesystem lookup.

Known Task selectors are checked against canonical repository scope before
routing/preparation; retained branch lookup is also checked before execution.
Exact and abbreviated Task IDs now resolve within the selected path or repository
plan once, before dispatch; primary and stack-parent selectors carry full IDs.
Dispatch no longer reparses the original argv and loses that selection. Task Flow
launches retain the full ID rather than returning to a display alias. Remote
execution remains unproved; argument tests cover transport only.

`lf list` now handles an absent registry without registering; unreadable stores
remain errors. Focused CLI fixtures cover default/overridden roots, explicit paths,
no registration, missing/non-Git paths and another Task's ID refusing edit,
checkout and launch.
### Cutover and acceptance

Local `--context` reuses launch prompt assembly and bound Work enrichment for skills and inline requests;
`--explain` uses WorkSelection/ContextFact before admission or preparation. JSON
combines both on request. Primary scoped identity survives dispatch. Broader action
explanation, remote preview and complete-path composition remain unfinished.

Headless gate separates command discovery (`documented_commands`), wire shape
(`dto_fixtures`) and behavioral routing/preview tests. The latter must establish
no provider or Work effects, selected-Machine repository lookup, explicit unknowns
and preview/launch input parity for the same snapshot. Existing exact-target
fixtures prove separate resolver/model boundaries, not the composed usable endpoint.

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

## Delete — do not maintain

Removed: launch-only opening, automatic execution routing/preparation, split Session/Changes
intent and URL-based completion. One generation-scoped `LinkedSession` observes existing
Session/Files/pane owners. Palette links reuse `TaskLink.url`; direct/chooser failures
share receipts. Compression also removes `multiplexerStoreDidChange`, `_notify`, both
view revision counters and `openingSessionObservation`. Session and multiplexer stores
now use Observation, like Files; no replacement readiness or layout store is added.
MultiplexerStore now owns exact-pane visibility for reads and readiness; ordered
request-ID snapshots are sets, with no copied receipt dictionary for validation.

Repository resolution and restored-scene validation now share the router;
pre-registration failures are inspectable. Mounted Task content, not selection,
settles the request generation.

Removed in the invocation cut: Workflow choice embedded in `traverse_workflow`
and fallback Process observation after parsed previews. Shared selection and ordinary
command observation survive; no second dry-run planner exists.

Remaining removals are `Commands::Project` and `Commands::Roadmap`, with consumer
cutover to Wave planning. `Context { explain }`, validation-only scope checks,
`execute_command`'s raw-argv reparse and unused `resolve_checkout_binding` are removed.
Checkout/retirement fixtures now exercise the surviving execution-binding reader. The standalone budget reader remains;
its final home is open. Prompt excerpts now defer persistence until launch. Their underlying
planning/history/prompt owners survive. LOO-412 owns exchange replacement;
no second planning writer or callback planner is introduced.

Removed: copied inheritance; duplicate window/opening paths; unqualified terminal
keys; duplicate visibility/pruning/insertion paths; root `run` and `open`;
`MonitorCommand::{List,Show,Usage,Activity}` and root Replay; `DesktopCommand::Inspect`;
`run::split_skill_args`; unused fresh `ProcessPromptInput.cwd`/`max_turns` overrides.
History owns recorded reads/replay/usage, Desktop owns open/list, and Task edges
retain explicit skill/Flow dispatch. Historical captures retain their data and
normalization. `output::print_process` remains shared with live Monitor.
Earlier Monitor failure: `e844d1458:scratch/compare-cmux-s-command-line.md`.

Destination lookup and scope checks use the existing read-only SQLite owner.
Missing registries stay absent; unreadable registries remain errors. Explicit
`repo identity` owns registration, not peer admission. Earlier removed wrappers
and initialization paths: `f5770094d:scratch/compare-cmux-s-command-line.md`, this heading.

Input reuses `insertTerminalText` and the pane event. Registry, multiplexer and
surface owners remain authoritative: Close/Undo renews content tokens; filtered
absence never releases a surface; passive reads allocate/clean up nothing and
never fall back to unbounded copies. [Findings](findings.md) retain contrary
native/artifact evidence and the plan below retains incomplete composition.

## Implementation sequence — one PR

**CLI cutover, partially implemented (October 8–9).** Root `run`, public
`--max-turns`/`--no-loopflow`, root `open`, recorded Monitor routes and raw-argv
reparsing are removed. Task edges use explicit skill/Flow dispatch; history and
Desktop retain the owners in the accepted map. Captured requests and command
normalization survive. The repeated-node steer filter still requires operating
guidance in its regression fixture. Detailed fixtures and the original missing-Workflow-node repair:
`1f42a98e4:scratch/compare-cmux-s-command-line.md`, Implementation sequence.

### Local invocation previews and recorded location — implemented locally

Local skill/inline previews share launch Work assembly without storage or excerpt
writes. `task run --explain` now reports invocation action and impediments; other
action explanation and remote/Flow previews remain.
`tasks.checkout_machine_id` separates recorded execution from delegation; missing
Machine evidence stays unknown. The one migration draft changes no installed store.
Prior focused evidence: `0cb4ff2c5:scratch/compare-cmux-s-command-line.md`, Local
invocation previews / Separate recorded checkout location. Gate owns full lifecycle
and installed migration checks.

### Task run invocation explanation — October 9

Implemented locally: text/JSON carries identity resolution, the selected edge or
ad-hoc Flow, impediments and unavailable evidence. `select_task_run` replaces the
choice embedded in `traverse_workflow`; launch and preview share it and the existing
launch-admission reader. Only launch takes up/moves Workflow or prepares Work.
Omitted Task selection uses the existing checkout/declaration resolver in both
paths. Recorded execution wins for started Tasks; effective delegation is a proposed
destination for locally unstarted Tasks, never first-start permission.

Remote Workflow state is not read from a caller's copy. Missing/corrupt registries,
unknown Machine, absent checkout, future placement, completion reconciliation and
provider/account checks remain explicit. Parsed previews suppress the post-command
Process observation that otherwise writes after their read-only code returns.
The strengthened state fixture checkpoints WAL before comparing the whole database;
its initial failure and repaired owner are in [findings](findings.md).

Remaining: general action explanation, remote/Flow input, Wave planning, composed
exchange/admission and verified bounded native extraction. No separate delivery or native acceptance follows.

### Local composed opening — October 9

`1dd8230d0` and `e3ca861b1` satisfy the preceding repository-failure/Task-page
readiness direction. Delayed lookup, registration, failure and cancellation are
fenced by request. Remaining work is remote/native composition, not another
readiness owner.

`desktop open` now reads the shared Work resolver and sends a repository-qualified
Task link to the existing identity-keyed window router. `--session` selects the
exact Task-associated conversation; `--diff` reveals retained Changes beside it.
Without a Session selector, the existing primary-Session owner prepares/chooses it;
subsequent reads retain the originating Task and complete Session inventory.
No CLI checkout preparation or automatic routing to the execution Machine remains.
Explicit `--machine` still selects the command's machine, not a viewing relay.

Links retain literal paths, full IDs and `diff` on relocation; lookup accepts planning
and durable Task IDs. Changes reuse the Files document cache and drafts. Generation
checks fence late navigation/insertion, and unchanged Task reopening avoids needless
writes. Chooser and direct links share failure receipts.

LaunchServices returns `opening`; `.prepared` and layout insertion are not usable.
The destination generation follows Session connection/surface readiness, Files comparison
and the original visible pane occurrences. Repeated URLs are new requests. Closing,
hiding, replacing or zooming away a requested pane fails the pending request without
waiting for another Session event. Navigation cancels the request without stopping shared
preparation; late callbacks/comparisons cannot settle replacements. Session, Files and
multiplexer all use Observation, replacing the Combine/notification bridges and refresh
counters. Native creation failures propagate, including builds without Ghostty.

Composed `--diff` clears zoom to reveal both panes; ordinary companion commands retain
zoom. Pending exact links suppress automatic primary entry. These are model receipts,
not rendering or provider acceptance.

Pre-registration failures are inspectable through the router. Registration retains
validated request IDs; a later request confirms the retained shell through its own
view callback. Plain Task links wait for mounted page content or fail on its
unavailable surface. Pane visibility is observed even before a Session reading
arrives, so absence cannot mask closure. These callbacks prove model/view readiness,
not compositor or provider usability. Receipts retain only the latest outcome.

**Remaining opening implementation/proof:** remote composition and native endpoints.
Headless fixtures cover supersession, cancellation, delayed registration, failures,
closed panes and retained file selection, but not LaunchServices → real Session
surface + usable Changes. Native endpoints remain in this one PR's demo; Linux
refusal remains with gate/CI.

### 2. Shared Work identity, delegation and routing

**Partially implemented, not composed.** The branch includes LOO-406's
common writer and planning-sync receipts. `repository_plans` supplies local plan
identity even without Tasks; explicit binding can associate a peer path with that
ID. Known started Tasks route via recorded checkout Machine; locally unstarted
Tasks use effective delegation. Neither rewrites Machine defaults. LOO-412's
portable exchange still needs composition with this identity/delegation schema.
The two-store routing fixture seeds identical Work IDs directly: it proves routing
is independent of path/default, not exchange convergence or peer start admission.

First-start observation/admission remains unresolved: local absence cannot prove
a peer never started. Even a fresh negative peer observation is not a reservation.
A local SQLite transaction cannot exclude a competing peer allocation. Dependent
cross-machine launch work must not treat the prototype as complete admission.
Delegation inheritance remains proposed, not accepted from source code alone.

Remaining identity work composes the existing repository root with LOO-412's
user-keyed default and explicit shared selection. At `ce740b028`, one repository
can select different destinations for different Waves; a destination/ref is not
itself repository identity. Window convergence must preserve that distinction,
not open a window per destination or implicitly merge existing plans.
Code remotes grant neither identity nor publication permission. `bind_repository` currently refuses a different
existing ID; preserving existing plans while selecting/joining belongs in this
composition, not a second registry. Connected-plan ownership and historical IDs
remain intact.

Resolve Work → effective delegation → execution checkout. Define explicit versus
inherited provenance, assignment changes and pending/conflicted synchronization. A
Machine can execute work from several repositories without rewriting its saved
default per request. Consume LOO-412's transport and LOO-406's common local writer; do not add a
callback planner or another synchronization engine. Work is one logical planning
model with synchronized local records, not one physical planning-machine store.
Retain historical execution and explicit legacy-ID mappings.

**Required proof:** two machine fixtures converge on the same planning identities and
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
portable planning exchange separately from code branches. Its committed prototype
uses `refs/loopflow/planning/users/<uuid>` by default and explicit
`refs/loopflow/planning/shared/<name>` destinations. Connection selects no existing
Work and publishes nothing; explicit Wave selection includes descendants/history.
The selected remote, user key, scope and merge protocol remain with that owner.
This is dependency source evidence, not an integrated CLI promise or authorization
to publish real plan data to the public code remote.

Planning changes (identity, brief, membership, comments, completion) are locally
committed with stable mutation identity and pending synchronization. Execution
facts (Workflow position, checkout paths, Sessions, Processes, claims, signal
and cleanup authority) are excluded from the ref. Delegation metadata's exchange
scope must agree with the common schema/protocol; assignment is not process state.

Git unavailability does not make ordinary planning writes depend on a laptop:
local saves succeed with pending sync, rather than unconfirmed host callbacks.
Pending data is not remotely durable until published. Existing execution keeps
its own lifecycle; no automatic turn/Flow retry is introduced. Online comments
and completion propagate semi-live through the active sync owner. Main's LOO-406
`d20c56daf` is integrated, retaining repository-scoped foreground Linear sync.
Readback cannot exclude unseen Linear reopening. LOO-412 through `cdec83fe8` is
inspected, not integrated. Creation discovery uses one pending-export view;
deletion receipts preserve attempts and settlement through the common writer.
Rejection of an object's provider frontier can still roll back imported uncertain
receipts while retaining its journal/conflict; ordinary delivery ignores that
conflict. Retention alone therefore cannot prevent another effect. This source
gap is not an observed duplicate write. Mixed Linear/Git stays disabled until
retained effects constrain delivery without blocking independent acquisition.
Ordering receipts (not scalar ranks), alternate/relationship acquisition, legacy
association and unplaced-Wave/Desktop remain. Deletion fixtures compare seven
populated execution tables including Processes; the earlier creation fixture
compares six unrun tables and its revised status assertions await gate. Neither
proves live controls, mixed-provider lifetime behavior or peer-exclusive admission.
[Findings](findings.md#committed-exchange-update--october-9) retains exact revisions,
the superseded discovery failure and dependency proof limits.
LOO-427 consumes the repaired common path rather than bypassing its
limitation or narrowing acceptance to disconnected repositories. Import uses the
common local writer, preserves causal reopening and conflicts, and avoids echoes
with optional Linear sync. Receiving a comment is not a command to start a Flow.

**Required mixed-operation proofs:** local `task run` writes its own Workflow and
runs locally while Task comments/follow-ups/completion synchronize to a second
Machine. Repeat exchange creates no duplicate mutations. Disconnect permits local
planning and execution with pending sync; reconnect converges supported planning
changes. Incoming completion never advances the other Machine's captured Workflow,
signals a Process or removes a checkout; delayed completion cannot overwrite newer
reopening. Rejected provider facts must preserve the displayed local projection,
not just its provider cache; unresolved membership remains visible while unrelated
imports proceed. Existing divergent execution IDs/history remain intact. No central
Workflow write, remote arrival acknowledgement or host-failure gate remains.

### 3. One repository window and command explanation

**Local mechanics implemented; broader explanation and peer composition remain.**
`desktop open/list` own launch/inspection; `--explain` reads local Work selection plus Task-run actions/impediments. Startup,
restoration, menu opening and Task links use repository-keyed windows. Per-repository
queues and delivery tokens preserve cold-open destinations and fence canceled
completions. Utility windows and retained surfaces keep their own lifetimes.
Inspection forwards Task and Session actions independently, retaining Task dates;
Session observation time stays unavailable. Saved/failed readings grant no actions.
Shared peer identity, remote observations/opening and configured proof remain.

**Required proof:** cold opens for two repositories arriving before registration and
registering in reverse order both reach their own windows; same-repo opens reuse
one. Plain-repo and remote-unavailable cases remain useful. JSON/Swift agree.

### 4. Arrange and interact with exact retained panes

**Retained-pane arrangement and exact input implemented locally; native proof remains.**
`DesktopPaneTarget` names repository, window incarnation, Machine/checkout, pane
and content incarnation. One typed Apple event and synchronous MainActor validation
reach existing owners, with JSON as data, not script interpolation. Restore removes
collapse without navigation, focus or zoom changes. Move relocates the retained leaf
within one Machine/checkout; resize gives the target side its divider share. Split
preserves selection/zoom; move/resize preserve visibility too. Zoom grants no focus;
explicit focus reveals/selects only the addressed pane in its retained workspace.
Q2 remains open.

Lost replies leave uncertain outcomes. Hide/restore are idempotent; split and Shell
can allocate again, so inspection must precede any deliberate retry. Close/Undo
renews content identity; the surface lifetime is separate. Passive reads allocate
or clean up nothing, and neither token alone authorizes input. Earlier detail:
`0dba18b79:scratch/compare-cmux-s-command-line.md`, section 4.

Companions are implemented locally: `lf desktop shell/files/flow-log` dispatches
through the same exact-target command. Shell opening requires the local Machine;
Files/Flow-log require the named Task's recorded/prepared checkout to match.
They fill an empty target or split beside occupied contents, preserving selection,
zoom and Work navigation. Files/Flow-log reuse retained panes and drafts; Shell
allocates a new pane, so a lost reply cannot authorize replay. Toolbar and CLI
share insertion rather than separate layout mutations. Review caught a notification
preceding shell command publication; the common insertion now publishes both
together. Native surface/draft retention across this full path is still unproved.

Machine-qualified surface/callback/membership and Session-cache identity is local;
remote composition remains. Detailed evidence: [findings](findings.md).

Passive inspection returns a native `surface` incarnation only for an existing
surface, including a retained exited surface until its lifecycle owner releases it.
Missing surfaces remain unavailable without allocating. The token comes from
`ProgramStatusSurface`, not pane occurrence or a parallel generation store. A
released view cannot create a second native surface with its old lifetime.

**Bounded extraction patch implemented locally; artifact and composition remain.**
`0004-bounded-text.patch` extends the embedded reader with caller-owned byte
storage. `ScreenFormatter` writes directly into a fixed destination, stopping on
capacity and checking only the final UTF-8 scalar for truncation; no full string or pin map
is allocated. Explicit selections cover viewport/scrollback; a null selection
reads the current text or command block, with empty distinguished from failure.
Review removed stale command-block pin cleanup from its getter; selection changes,
reset and teardown retain that ownership. Clipboard/Quick Look keep their existing
unbounded APIs and viewport metadata, not a fallback for this read.

The next artifact remains `a60e9e2-lf3`; `Package.swift` still pins verified lf2.
Linux extraction tests/libraries pass but supply no native framework. Native helpers
stalled before main; no framework was built or published. [Findings](findings.md)
and `0cb4ff2c5:scratch/compare-cmux-s-command-line.md` retain failed attempts.
Jack Heart's October 6 authorization covers future artifacts after relevant checks
and download checksum verification, not real planning-data publication.

**Exact-target read boundary implemented locally.** `lf desktop read` sends the
pane target, required surface incarnation, region and byte bound through the
existing Apple event transport. Window and pane validation share arrangement's
owners; passive pool lookup validates the surface lifetime without `view(for:)`
or `hasSurface`. The reply echoes the request with observation time, pane-local
collapse/zoom visibility and a tagged available/unavailable result. Empty text
cannot stand for unavailable. Stale windows/content/surfaces fail without fallback;
focus does not select the read target. Limits are 1–1048576 bytes (default 65536).

The pinned lf2 reader returns `bounded_reader_unavailable` for an existing exact
surface, `missing_surface` for absence and `not_terminal` for companion panes.
This is not native extraction or a complete read capability; inspection does not
advertise it as supported. No new symbol, dynamic lookup or unbounded fallback was
added. Remaining native composition is a verified lf3 build/publication via
[the patch workflow](../swift/GhosttyKitPatches/README.md), manifest/checksum
selection, then fixed-buffer C extraction and bounded Swift copying at this
existing boundary. A capable build executor is required; repeating unchanged
pre-main stalls, importing the unavailable symbol or using unbounded reads
does not complete it.

Fixtures cover the wire contract, missing/nonterminal surfaces, limits, stale
window/content and focus-independent targeting. A display-gated fixture covers
real exited-surface retention and replacement rejection; it has not run. Empty
native selections, extraction/copy limits and retained terminal drafts still need
the bounded C reader composed and executed, not inference from DTO examples.

**Exact input implemented locally (October 8).** `desktop text/key` extend the
existing pane action/event; no input router, socket, layout store or clipboard
path is added. Window/content validation and Machine-qualified surface lookup
stay synchronous. Missing, exited or replaced surfaces fail without allocation,
cleanup, client acquisition or focus fallback. Text uses ordinary insertion;
keys use paired Ghostty press/release. Focus, layout and existing drafts stay put.
Read tokens alone never cause input; each input command is deliberate.

Jack selected combining with the existing draft (Q3): insert at the current cursor,
with separate Enter submitting the combined contents. Text rejects C0/C1 controls,
including newlines and Tab, rather than risking submission when bracketed paste
is disabled. No escape decoding, silent clear or occupied-draft refusal. Active
IME composition must finish first. These reversible input restrictions are in
questions.md. Lost replies remain uncertain and cannot authorize automatic retry.

Wire fixtures cover every key and required surface identity; headless fixtures
cover invalid text, missing/nonterminal/stale content and retained state. The
owned two-shell native fixture checks cursor insertion, explicit submission and
focus retention; exit/replacement coverage also rejects input. These native
fixtures require a display and remain unexecuted. Bounded observation still
establishes output, never Task/turn completion.

**Required proof:** stale targets, focus changes, surface replacement, retained drafts,
literal text versus Enter, passive reads and limits. Linux errors precede any
app/provider/Work effect. Use owned fixture terminals and fake providers only.

### 5. Complete-path verification and demo

**Depends on 1–4.** No partial slice ships separately. Gate owns affected Rust
behavior/migration suites and
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

Demo: one repository window shows two Tasks on different Machines. Explain their
identity/delegation, open them, add shell/Files panes, retain an unfinished draft,
change focus, target a harmless command, read output, hide/restore, and verify the
original input target and draft survive. Demo owns native usability; preserve comparison evidence and use no real accounts or live user terminals.

Earlier failed attempts and check archives: [findings](findings.md) and
`f48d84511:scratch/compare-cmux-s-command-line.md`.

Check (October 9 sync): network-isolated `cli_discovery` filters `desktop_input_keeps_text_separate_from_keys_and_requires_surface_identity` and `portable_help_describes_exact_kind_selection` PASS (2); broader gate/CI and native demo remain. Prior checks: `b7f7a234c:scratch/compare-cmux-s-command-line.md`.
