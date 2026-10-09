# Desktop control on shared Work — LOO-427

**Status (October 8, 2026):** Jack Heart authorized implementation through demo
review, in one PR. Recorded checkout location, prototype delegation/routing,
repository-keyed windows, exact retained-pane arrangement and text/key input are
implemented locally. Passive reads validate exact pane/surface identity but pinned
lf2 cannot extract bounded text; the verified native artifact remains required.
Source and focused compile/model evidence establish neither native acceptance nor delivery.

LOO-406 through `b6f34a6f8` is integrated; LOO-412 through `a388ed425` was inspected,
not integrated. Mixed Linear/Git remains disabled pending acquisition/receipt
composition, not by product policy. Shared exchange, peer first-start admission,
remote Desktop opening and complete-path proof remain. Implementation details and
contrary evidence follow below and in [findings](findings.md); [questions](questions.md)
retains unresolved choices. [Comparison](../docs/reviews/terminal-command-comparison.md)
retains every host disposition and behavioral observation.

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
Exact Task IDs now obey the store's repository filter. Ambiguous abbreviated
Task IDs can still fail in downstream global lookup despite a scoped match;
carrying the resolved identity through those consumers remains. Remote execution remains unproved; argument tests cover transport only.

`lf list` now handles an absent registry without registering; unreadable stores
remain errors. Focused CLI fixtures cover default/overridden roots, explicit paths,
no registration, missing/non-Git paths and another Task's ID refusing edit,
checkout and launch.
### Cutover and acceptance

Proposed structures: `RepositorySelection` (cwd/name/path), one resolved invocation
carrying Work/Machine and intended effect, and `InvocationExplanation` with sources,
unavailable facts and effects. Reuse `WorkSelection`, `ContextFact`, existing
prompt assembly and Clap navigation; no parallel resolver or execution planner.
Proposed functions `resolve_repository(selection, machine)` and
`explain_invocation(resolved)` are read paths; context preview consumes the same
prompt builder as launch. Names are draft, not promises of new public Rust APIs.

Remaining public removals: `Commands::Project`, `Roadmap`,
and `Context { explain }` dispatch in `lf/mod.rs`
and `bin/lf.rs`. History now owns the former recorded Monitor commands and Replay. Preserve implementation owners,
DTO evidence, history, native retention and supported published contracts.
Discord stays unchanged.

Headless gate: `documented_commands`, CLI routing and DTO fixtures must prove
canonical/short forms agree, ambiguity has no effects, previews start no providers
or mutating operations, repository roots resolve on the selected Machine, and
unknown facts remain explicit. Preview input matches launch assembly for the same
snapshot. Desktop list/open retain the existing exact-target and readiness tests.
This API cut is accepted within the one-PR design; native-reader work is independent.
Unresolved choices remain in questions.md.

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

Remaining removals are `Commands::Project`, `Commands::Roadmap` and the
`Context { explain }` dispatch in `lf/mod.rs` and `bin/lf.rs`, together with
consumer cutover to Wave planning and invocation preview. Their underlying
planning/history/prompt owners survive. LOO-412 owns exchange replacement;
no second planning writer or callback planner is introduced.

Removed: copied inheritance; duplicate window/opening paths; unqualified terminal
keys; duplicate visibility/pruning/insertion paths; root `run` and `open`;
`MonitorCommand::{List,Show,Usage,Activity}` and root Replay; `DesktopCommand::Inspect`;
`run::split_skill_args`; unused fresh `ProcessPromptInput.cwd`/`max_turns` overrides.
History owns recorded reads/replay/usage, Desktop owns open/list, and Task edges
retain explicit skill/Flow dispatch. Historical captures retain their data and
normalization. `output::print_process` remains shared with live Monitor.
Detailed cutover evidence and the earlier uncompilable Monitor move remain at
`e844d1458:scratch/compare-cmux-s-command-line.md`, this heading.

Destination lookup now uses the existing read-only SQLite owner, removing its
Tokio runtimes, mutable store initialization, duplicate automatic-route store
open and unused `Store::{repository_path,bind_repository}` forwarding. Scope checks and repository-plan lookup use the same reader. Missing
registries stay absent; unreadable registries remain errors. Explicit
`repo identity` still owns registration. This is observation, not peer admission.

Input reuses `insertTerminalText` and the pane event. Registry, multiplexer and
surface owners remain authoritative: Close/Undo renews content tokens; filtered
absence never releases a surface; passive reads allocate/clean up nothing and
never fall back to unbounded copies. [Findings](findings.md) retain contrary
native/artifact evidence and the plan below retains incomplete composition.

## Implementation sequence — one PR

**CLI cutover, partially implemented (October 8).** Root `run` dispatch and the
public `--max-turns`/`--no-loopflow` switches are removed, including forwarding,
help, current examples and launch fixtures. Task edges resolve in their prepared
checkout and invoke explicit `flow` or `skill`, escaping command-name collisions.
Shared execution, captured requests/replay and standalone native-skill guidance
selection survive. `--steers-after` and the repeated-node context assertions remain;
the fixture now also requires normal operating guidance. Review repaired missing Workflow nodes; fixtures cover
ambiguous root `run`, removed flags, cursor forwarding, reserved Flow names and
single-skill Workflow edges. These Rust fixtures have not executed.

History now defaults to the durable Work feed, with `list`, `show`, `usage` and
`replay` under it. Monitor retains live observations. Desktop's one-shot activity
read, fixtures and current docs use the new owner; wire records are unchanged.
Saved command normalization preserves historical selectors without exposing old
Monitor routes. Review repaired stale examples that used `show` with list filters.

`DesktopCommand::Open` now owns the existing app launcher; `Commands::Open` and
`commands/open.rs` are deleted. Captured root-open commands normalize to the new
owner. Ordinary Work selection/routing and early Linux refusal remain; this does
not yet deliver a selected Task/Session into its repository window or report
rendered readiness. Exact pane controls still bypass Work preparation. Inspection
capabilities now advertise `list`, not the removed `inspect` CLI name.

Remaining API work: Wave-owned planning, composed Work opening, context preview/explain
and carrying repository-scoped Task identity through remaining global lookups.
Q2, shared-plan composition, native extraction and input proof, and complete-path acceptance
remain required. This internal cut neither completes LOO-427 nor splits its PR.

### 1. Separate recorded checkout location from delegation — Implemented locally

`tasks.checkout_machine_id` records location beside `worktree`. The one Task draft,
`task_checkout_machine.sql`, backfills existing placement evidence and leaves
missing Machine evidence unknown. New checkouts record the preparing Machine;
new Work resolves effective placement instead of copying its parent.

`task_checkouts` no longer joins `work_placements`. Status, Task-file access,
comparisons and Session workspace resolution consume recorded location. SQL
Session/Process membership now includes Machine, while explicit bindings survive.
Git read-ahead skips remote/unknown checkouts. Existing DTOs already carry an
optional Machine; wire shapes and Swift decoding need no change.
Comparisons read the validated checkout and its PR through one store, without
hydrating the Task again. Session resolution uses cwd and explicit Work; its
unused recorded-root argument is removed.

Unknown location cannot authorize local file reads or infer membership from an
unavailable path. The later delegation prototype changes no recorded checkout
location or live process. The same migration draft owns subsequent schema changes.

Focused fixtures cover migration/history retention, reassignment A→B with retained
files/status/Session identity on A, creation on the preparing Machine, unknown
history, alias pagination and explicit bindings. No installed store was changed.

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
and completion propagate semi-live through the active sync owner. LOO-406 now keeps
Desktop foreground Linear sync scoped to the repository (`29777b8cb`), independent
of selected Task/Wave; switching selection must not stop propagation. Git exchange
is still unintegrated and cannot claim this coverage. LOO-412's newer source has
taskless foreground/post-save exchange, but refuses Git exchange in connected
Linear repositories. The newer `270019c8d` cut acquires provider evidence before
projecting fields,
rolls rejected objects back to savepoints and retains membership/provider conflicts
outside that rollback. Malformed input aborts import; valid contradictory observations
do not discard independent objects. Those regressions remain unexecuted. `7262b6b20` shares field-winner/baseline
normalization; `a388ed425` routes peer Task completion/reopening/cancellation
through common state-delivery receipts with stable mutation IDs, causal baselines
and no local Workflow move. Reimports retain uncertain attempts. This source
cut has SQL checks, not executed Rust or composed proof. Other grouped receipts,
alternate acquisition and legacy association still need composition.
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

**Local mechanics implemented; public API reconciliation remains.** `desktop open/list`
own app launch and retained-window reading; root `open` and `inspect` are removed. `context --explain`
still awaits the broader invocation API. Cross-machine composition depends on 2. Startup,
restoration, menu opening and Task links use repository-keyed windows. Per-repository
queues and delivery tokens preserve cold-open destinations and fence canceled
completions. Utility windows and retained surfaces keep their own lifetimes.
Inspection forwards Task and Session actions independently, retaining Task dates;
Session observation time stays unavailable. Saved/failed readings grant no actions.
Shared peer identity, remote observations/opening and configured proof remain.

`explain_context(selection)` returns Machine/repo/checkout/Wave/Task/Session/Process
and selection provenance, including absent/unavailable values. `inspect_desktop`
returns selected work, layout, focus, supported operations and existing Rust
legal actions. Explicit targets and checkout inference share the same resolver.

**Required proof:** cold opens for two repositories arriving before registration and
registering in reverse order both reach their own windows; same-repo opens reuse
one. Plain-repo and remote-unavailable cases remain useful. JSON/Swift agree.

### 4. Arrange and interact with exact retained panes

**Retained-pane arrangement and exact input implemented locally; native proof remains.** `DesktopPaneTarget`
contains repository, window incarnation, Machine, checkout, pane and content
incarnation. `lf desktop hide/restore --target JSON` uses one Apple event carrying
JSON as an argument, not script interpolation. Router and workspace validation plus
`setCollapsed` run synchronously on the main actor. No new workspace, navigation,
provider/client acquisition or Work mutation is requested. Restore only removes
collapse; it preserves Work selection, window focus and zoom. A lost reply leaves
outcome uncertain; hide/restore are idempotent.

Close/Undo renews content tokens. Surface incarnation is separate; passive reads
validate it without allocation or exit cleanup. Neither token grants input authority.

Focus, empty split, move, resize and explicit zoom now share hide/restore's
single typed Apple event and synchronous exact-target validation. A failed reply
leaves the outcome uncertain: unlike hide/restore, retrying split can add another
pane. Inspection, not automatic replay, establishes what happened. Move and resize
validate both targets before mutation and stay in one Machine/checkout. Move
relocates the original leaf, not close/load; content occurrences, shell commands
and surface keys survive. Resize gives the target's side the requested divider
share, including when it is the second subtree. Split preserves selection/zoom;
move/resize preserve visibility too. Zoom changes visibility without selecting
another pane; rendering no longer marks an unselected zoomed terminal focused.
Focus selects/reveals the addressed pane in its retained workspace, without Work
navigation or foregrounding another window. These are reversible implementation
choices, not Jack's resolution of Q2.


Companions are implemented locally: `lf desktop shell/files/flow-log` dispatches
through the same exact-target command. Shell opening requires the local Machine;
Files/Flow-log require the named Task's recorded/prepared checkout to match.
They fill an empty target or split beside occupied contents, preserving selection,
zoom and Work navigation. Files/Flow-log reuse retained panes and drafts; Shell
allocates a new pane, so a lost reply cannot authorize replay. Toolbar and CLI
share insertion rather than separate layout mutations. Review caught a notification
preceding shell command publication; the common insertion now publishes both
together. Native surface/draft retention across this full path is still unproved.

Machine-qualified terminal identities now reach surface lookup, title/bell/close
callbacks, shell discovery, status association and retained membership. An unknown
Session Machine cannot select a terminal by ID alone. Session reads/cache markers
use Machine/repository keys; late direct reads and mutation readbacks keep their
originating identity. Remote routing/composition remains separate and unfinished.

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

The patch workflow selects the next `a60e9e2-lf3` artifact and includes focused
bounded-text tests. `Package.swift` still selects published lf2: the new symbol is
not imported by Desktop before a verified artifact exists. Native build helpers
stalled at `_dyld_start`, before testing; the Linux terminal-only build/tests pass. The earlier embedded Linux build lacked cached HarfBuzz. A later application-root
check passes 79 tests, and the complete embedded Linux libraries compile
with Fontconfig, exporting the bounded reader (`d30198ccb`). Neither supplies a
native framework. The independent pre-main capability failure is recorded in
findings; the framework remains unbuilt.
No artifact was published. Jack Heart's October 6 authorization already covers
future patch artifacts after relevant build/behavior checks and download checksum
verification; it does not authorize real planning-data publication.

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

**Depends on 1–4.** No partial slice ships separately. Gate runs affected Rust
behavior/migration suites and
`cargo test -p loopflow --test dto_fixtures` for Rust wire fixtures, plus
`cargo test -p loopflow --test documented_commands` for command ambiguity only.
Use `scripts/test_desktop.sh -Xswiftc -gnone --no-parallel` for headless Desktop
build/tests. Include the full `session_lifecycle_tests` suite and the disposable installation
migration harness for the changed association/schema. Materialized migration and
Desktop checks remain with gate; empty filters are not proof. Checks use the
isolation in TESTING.md. LOO-406's `planning_reconnect_tests` now owns Linux
work-watch/Flow reconnect fixtures; those remain unexecuted evidence, reusable
for the common writer rather than a second reconnect harness. LOO-427 still
needs mixed Git-exchange and Desktop-selection coverage.

Demo: one repository window shows two Tasks on different Machines. Explain their
identity/delegation, open them, add shell/Files panes, retain an unfinished draft,
change focus, target a harmless command, read output, hide/restore, and verify the
original input target and draft survive. Review native usability separately from
headless gate. Preserve comparison dispositions and evidence limits; no real
provider accounts or live user terminals.

Prior failed attempts, Swift compile/model evidence and Linux extraction checks
remain in [findings](findings.md), Git blob `6046145dcdd3d334bb2ad3560130de60dce82ca5`
and `/tmp/loo427-compress-msMFvo/current-scratch/`. No native surface proof or
artifact publication follows from these checks.

Check (October 9 sync): network-isolated `cargo test --offline -p loopflow --test global_commands repo_selection_uses_machine_root_or_explicit_checkout_without_registration -- --exact` PASS (1); merged `cbf0a174a` via owned continuation, retaining `try_exists()` error propagation. Earlier formatting/Clippy/routing checks and repair logs: `2e03f0b76:scratch/compare-cmux-s-command-line.md`. Full gate/CI and native/composed demo remain.
