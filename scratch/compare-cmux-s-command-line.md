# Desktop control on shared Work — LOO-427

**Status (reconciled October 9, 2026):** Jack Heart authorized implementation through demo
review, in one PR. Recorded checkout location, prototype delegation/routing,
repository-keyed windows, exact retained-pane arrangement and text/key input are
implemented locally. Passive reads use the published, checksum-verified lf3
fixed-buffer reader with exact pane/surface validation; native proof remains.
Focused local Rust and model evidence establish neither native acceptance nor delivery.

LOO-418/#1499 is integrated; completion/checkout previews and creation receipts are reconciled.
LOO-406/#1503 and LOO-412 through `96f714bd6` are composed locally;
`1ece5cc52` reconciles transitive ancestry. Section 2 retains remaining composition.
Public completion/reopening exchange supersedes old intent. Delegation exchange,
first-start admission, mixed-provider composition, remote opening and broader
explanation remain. Mixed Linear/Git
stays disabled; native proof remains, including request-scoped Task/Session-plus-Changes opening.
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

Jack selected Wave-owned planning. Locally, `wave show [WAVE]` reads current and
historical chapter plans; `wave workflow` selects future take-up, and `wave edit-plan`
edits chapter name/summary. Project IDs, KRs, history and stored Workflow definitions
survive. Root `project`/`roadmap` and duplicate `run` dispatch are removed. Task run
still advances the captured Workflow; explicit skill/Flow invocation does not.
A separate cross-Wave backlog surface is optional, not implied by the old command.

Jack accepted combining activity and history (October 8): `lf history` is the
chronological feed, with execution details and replay underneath. Remove the
separate activity command; preserve event coverage, exact capture IDs, inputs/results,
filters and paging. Recorded `monitor list/show` and top-level `replay` move here.
Replay starts a new execution. Session-native history stays under Session.
Jack accepted `history usage` for aggregate views of tokens, cost and time.
Reuse existing records; no second store.

`desktop open/list` own opening and actual-window inspection, with short forms
through normal resolution. JSON retains freshness, exact targets, capabilities and
legal actions. Opening/usable/failed are distinct: layout mutation is not a rendered
endpoint. Remote viewing stays separate from execution placement.

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
The accepted cut removes `--max-turns` and `--no-loopflow`; both public switches
are removed locally while normal operating guidance remains.
Keep `--steers-after`: Jack recalled context-budget blowups. Flow records a cursor
per node, passed through `Cli::step_args` to `task_input::read_seed`. Preserve
`a_repeated_node_receives_only_task_direction_newer_than_its_last_run`'s inclusion,
omission and later-direction assertions when removing `--no-loopflow`. The fixture
was inspected, not rerun; fresh drivers reset cursors, so this is not cross-restart
deduplication. Prior trace: `859c414d1`, this section.

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
fail before effects. `--context` on a non-agent command is invalid; Flow preview shows the compiled graph and labeled node inputs: only the initial
skill/router has current-snapshot input. Future and repeat input stays unavailable;
commands have no agent prompt. It never executes predecessors.

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

Local skill/inline `--context` shares launch assembly and Work enrichment; Flow
previews share compilation and initial input, including native arguments. Read-only
`--explain` precedes admission/preparation and supports JSON and explicit Machines.
Broader action explanation and complete-path composition remain unfinished.

Headless gate separates discovery (`documented_commands`), wire shape
(`dto_fixtures`) and routing/preview behavior, including
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

LOO-406 owns the local planning lifecycle; LOO-412 supplies custom Git-ref Task
synchronization. Its started-Task transfer is excluded; remote launch composition
remains in LOO-427, using existing Task preparation. Ordinary operations remain local.
LOO-426 owns Work opening; LOO-416 saved panes; LOO-387 draft preparation; LOO-415/424 relay/resume;
LOO-422 host status; LOO-402/403 Waiting/shortcuts; LOO-397 command discovery.
Reuse these owners.

## Delete — do not maintain

Dependency composition excludes `ops/task/remote.rs`, `TaskSource`,
`LF_TASK_SOURCE`, its launch-time Work-binding preparation and transfer-only
`task_remote_tests`. Started Tasks keep their recorded Machine; shared planning
acquisition, pushed-branch preparation in the existing Task owner, preview parity
and account handling survive. Copied child placements stay deleted. Dependency
scratch stays at `19e31f64d`, not beside this Task's plan. Desktop replaces the
path-only peer reading key with the existing Machine/repository reading key.


LOO-418 integration removes preview's EndOptions/force and PR-required checkout
lookup. Task placement owns branch/base; completion requests replace the older
terminal-planning conflict. Keep scoped dispatch and read-only previews, deleting
duplicate preparation before routing. Task run and checkout dispatch before ordinary
Work binding; remove run's binding exception and copies used only for Wave validation.
Retain repository checks for historical branch selectors resolved after entry.
Historical filing uses `planning_creations`,
not removed export columns on `task_creation_intents`. Original command, reader
and callback deletions: `96b3217fe`, this section; surviving owners remain below.

Window retention/fence deletion details: `6ce198b75`, this section. Scene equality
cannot fence A→B→A; receiver, delivery and native lifetimes stay separate.

Repository association replaces the blanket different-ID refusal with one selected
identity plus retained local IDs in `repository_plans`; no second alias registry.
The single Task draft changes in place. Entry dispatch and routing share scoped Task
resolution: repository IDs (including historical locators) resolve to a path before
Task aliases/prefixes. The post-routing identity comparison is deleted; scope does
not depend on globally unique aliases. Unknown Tasks retain ordinary acquisition.

Receipt import/lookup, accepted-frontier and per-origin deletion details:
`a44ddcb5f`, this section. Import creates no local requests or execution authority.
Git disposition projection keeps request supersession and state delivery together,
removing the separate completion-update tail and disposition branch in scalar
delivery. Accepted mutations supersede intent; value changes drive delivery.
Both stay in the existing projection savepoint, without a new lifecycle owner.

Preview dispatch shares Task/location resolution, agent-input classification and
writer validation without preparation, locks or recovery. Compression details and
failed assertions remain at `a44ddcb5f`, this section; [findings](findings.md)
retain evidence. Identity alone still cannot validate an action.

Input reuses `insertTerminalText` and the pane event. Registry, multiplexer and
surface owners remain authoritative: Close/Undo renews content tokens; filtered
absence never releases a surface; passive reads allocate/clean up nothing and
never fall back to unbounded copies. [Findings](findings.md) retain contrary
native/artifact evidence and the plan below retains incomplete composition.

## Implementation sequence — one PR

### 2. Shared Work identity, delegation and routing

**Locally composed, incomplete.** LOO-406's common writer and LOO-412 through
`96f714bd6` supply exchange, foreground sync and repository Work-stream receipts.
Peers use Machine/repository keys. `repo identity --bind ID` associates established
roots, preserving prior local locators, Work, mappings, effects and execution;
it selects/publishes nothing. Imported Waves remain unplaced. Independent-root
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

Repository association retains locators; explicit Task/Project correspondence preserves
physical IDs. `66dd3c44f`/`2afcfba1a` compose lookup in one snapshot, not a reservation.
Neither association supplies remote location or first-start authority.

**Remaining identity/routing composition (October 9).**
`PlanningKind::fields()` carries neither RepositoryId nor authored delegation.
Unbound acquisition still allocates local identity; explicit association is required,
not inferred from a ref, remote or clone name. Historical repository IDs remain
Machine-local locators, not exported aliases. One selected ID serves every destination.

- Live-window association is implemented on opening/restoration. The common store
  reads selected ID and retained local locators in one statement; `repo identity --json`
  returns both. WorkLinkRouter keeps the existing scene/queue/receiver, updates its
  selected identity and renews its targeting token. Delayed lookups cannot roll back
  a newer open; restored aliases converge before mounting. Drafts, selection, native
  view owners and in-flight links survive headless fixtures. No native surface or
  automatic background identity-refresh proof follows.
- Exchange authored delegation through the common journal/Placement owners, retaining
  explicit/legacy provenance, conflicts and pending state. Apply accepted nearest-
  ancestor inheritance and narrower overrides (Q2); started Tasks stay put.
- Observe recorded execution location separately, with freshness/unavailability,
  before interpreting an imported Task's missing local checkout as unstarted.
  Couple first-start allocation to exclusive admission, not a negative peer read.
  The present refusal is a safe incomplete path, not successful delegated execution.
- Compose remote opening after identity/location resolution. Preserve execution on
  its recorded Machine, Mac presentation and saved Machine defaults; no transfer
  envelope, callback planner or second synchronization engine.

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

LOO-406 owns local writes/Linear; LOO-412 exchanges planning separately from code,
using `refs/loopflow/planning/users/<uuid>` by default and explicit
`refs/loopflow/planning/shared/<name>` destinations. Connection selects no existing
Work and publishes nothing; explicit Wave selection includes descendants/history.
LOO-412 owns destination, scope and merge policy; no real plan publication is authorized.

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
Readback cannot exclude unseen Linear reopening. LOO-412 through `19e31f64d` is
integrated locally, excluding the incompatible started-Task transfer path. Its receipt ordering, independent evidence rollback and
causal invalidation replace no local owner; prior mechanisms and contrary evidence:
`2c23acbb8:scratch/compare-cmux-s-command-line.md`, **Mixed command**.

Imported Waves stay unplaced; the existing repository Work stream carries scoped
last-good destination receipts/holds. Per-Work sharing/authorship/assignee and
losing-edit recovery presentation remain. No second poller or destination-derived
repository identity. Associated origins preserve original journals, creation inputs,
receipts and private selection; only accepted joint frontiers parent saves.
Earlier composition and failures: `a44ddcb5f`, this section, and
[findings](findings.md#shared-planning-composition).

**Ancestry and foreground composition, October 9:** `1ece5cc52` retires transitive
ancestors while retaining original heads, mapping validation, accepted-only save
parents and private holds. `b9e7013ad` replaces direct transport proof with public
association, Task saves and repository `monitor work --watch`: offline/reconnect,
causal reopening and lost-publication-receipt recovery preserve captured comment
input and seeded execution. Git confirmation never acknowledges Linear delivery.
[Findings](findings.md#shared-planning-composition) retain failed attempts and exact
limits; no live Linear, running provider or mounted Desktop proof follows.

**Post-LOO-418 completion boundary (reconciled October 9):** `e79451d05`/`2a2379913`
complete `public_completion_exchange_supersedes_only_old_completion_requests`,
the requested public-command fixture. It uses two isolated Homes, file-only Git and repository work-watch. Public complete,
reopen and end retry cover retained failed requests, same-value offline reopening,
stale completion and replay. Seeded Workflow/PR/placement, a disk draft and one owned
fake provider survive. This does not exercise a running `task run` Flow, configured
providers, native drafts or remote routing.

Accepted Git mutation identity supersedes requests; changed values alone trigger
state delivery. Both consequences share the projection savepoint, and the rollback
fixture preserves intent on receipt failure. Linear retains its status-change owner:
a title-only revision must not cancel intent. Explicit complete authors new intent;
retrying retained end cannot revive superseded intent. [Findings](findings.md#task-completion-integration--october-9)
retain the failed counterexamples and focused checks.

Next is authored delegation, still absent from the portable schema. Assignment,
execution-location observation and exclusive admission remain separate. Mixed-provider
exchange and shared first start remain refused.

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
`desktop open/list` own launch/inspection; `--explain` reads local Work selection,
Task-run actions/impediments and Session connection intent. Startup,
restoration, menu opening and Task links use repository-keyed windows. Per-repository
queues and delivery tokens preserve cold-open destinations and fence canceled
completions. Utility windows and retained surfaces keep their own lifetimes.
Inspection forwards Task and Session actions independently, retaining Task dates;
Session observation time stays unavailable. Saved/failed readings grant no actions.
Shared peer identity, remote observations/opening and configured proof remain.
**Explanation coverage, October 9:** run/move/Workflow restart, Desktop open,
Session connect and interactive message-less resume have operation-specific readings.
Create/edit/comment/refile/save share read-only validation with their writers; writes
revalidate. Refile resolves IDs before initialization; Save uses pinned path/revision/
draft checks without recovery writes. New identity/location, initialization and
comment provenance remain unallocated/unavailable. Detailed covered cases and
commit evidence: `a44ddcb5f`, this section, and [findings](findings.md).

Checkout reads Task-owned placement without requiring a PR; restoration and
preparation share validators without fetches or leases. LOO-418 removes move/run
`--force`; arrival records a completion request before settlement checks PR gates
and eligible cleanup. Preview performs neither.

**Remaining explanation work:** abandon/delete and interrupt still omit cleanup or
control impediments; complete/reopen/follow-up and design-handoff explanation remain. `history show` and agent invocations
remain identity-only; `--context` previews input. §2 composition and native proof remain.

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

**Bounded extraction composed October 9; native acceptance remains.**
`Package.swift` selects published `a60e9e2-lf3`. Public download and SwiftPM match
`dfa0e65b216cccbee132b1d579bc2bd646fda34b2822fe8e00b0306b70bada7f`.
Jack Heart's October 6 authorization covers this artifact workflow, not planning-data
publication. The prior ReleaseFast build exported both architectures and passed
270 primitive tests (four skipped), after repairing reset-pin cleanup. Earlier
build failures and native limits: `aade49b61:scratch/findings.md`, **Terminal identity
and extraction boundary**.

`desktop read` uses the existing Apple event, registry and passive surface lookup.
Synchronous window/pane/surface validation never allocates a terminal, cleans up
an exited child, acquires a client or follows focus. The renderer-locked formatter
writes directly into fixed caller storage; Swift copies only the written UTF-8
prefix. `screen` selects viewport, `scrollback` full retained screen, and `selection`
current text/command block. Empty succeeds; invalid regions fail without an
unbounded fallback. Selection/reset/teardown retain command-block pin ownership.
Clipboard/Quick Look keep their independent APIs.

Replies echo exact target, time and pane-local collapse/zoom visibility, not
compositor visibility. Bounds remain 1–1048576 bytes (default 65536). Inspection
advertises `read`; missing surfaces and companions remain explicitly unavailable.
The lf2 placeholder result and wire variant are removed, not maintained beside it.

Headless fixtures cover stale targets, missing/nonterminal surfaces, limits and
focus-independent routing; patch checks cover bounded formatting. Native fixtures
now observe input through the router and cover empty selection, each UTF-8 prefix,
viewport versus scrollback, retained selection and exited/replaced surfaces. They
compile but remain display-gated and unexecuted. No installed app or user terminal
was changed. Remaining proof: mounted extraction, input/draft retention, complete
CLI-to-pane path and remote composition in this one PR's demo.

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

Demo: one repository window shows two Tasks on different Machines. Explain their
identity/delegation, open them, add shell/Files panes, retain an unfinished draft,
change focus, target a harmless command, read output, hide/restore, and verify the
original input target and draft survive. Demo owns native usability; preserve comparison evidence and use no real accounts or live user terminals.

Check: `git diff --check` PASS; reuse `2a2379913` build, five focused isolated tests, fmt/Clippy (no code changes); broader suites/migrations gate/CI, native demo.
