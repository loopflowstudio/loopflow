# Desktop control on shared Work — LOO-427

**Status (reconciled October 9, 2026):** Jack Heart authorized implementation through demo
review, in one PR. Recorded checkout location, prototype delegation/routing,
repository-keyed windows, exact retained-pane arrangement and text/key input are
implemented locally. `9cc0707b0`/`92d95edfe` add owner observation and shared
Task/Desktop location explanations; `faebb042e`–`5d55957c6` compose remote Task
links through the existing Session/Files owners. Passive reads use the published, checksum-verified lf3
fixed-buffer reader with exact pane/surface validation; native proof remains.
Focused local Rust and model evidence establish neither native acceptance nor delivery.

LOO-418/#1499 is integrated; completion/checkout previews and creation receipts are reconciled.
LOO-406/#1503 and LOO-412 through `96f714bd6` are composed locally;
`1ece5cc52` reconciles transitive ancestry. Section 2 retains remaining composition.
Completion/reopening, delegation exchange and remote companions are composed locally.
Exclusive admission, mixed-provider exchange, broader explanation and configured/native proof remain. Mixed Linear/Git
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

Scoped full Task/parent IDs survive dispatch and Task Flow launch; canonical
repository checks reject foreign Tasks before preparation. Read-only registry
lookup distinguishes absent from unreadable. Root/path, no-registration,
foreign-repository and transport fixtures are recorded at
`debc66a7c:scratch/compare-cmux-s-command-line.md`, **Repository lookup**;
configured remote execution remains unproved.

### Cutover and acceptance

Local skill/inline `--context` shares launch assembly and Work enrichment; Flow
previews share compilation and initial input, including native arguments. Read-only
`--explain` precedes admission/preparation and supports JSON and explicit Machines.
Broader action explanation and complete-path composition remain unfinished.

Gate separates discovery, wire shape and routing/preview behavior (§5), including
effect-free reads, selected-Machine repository lookup and snapshot input parity.
Exact-target fixtures do not prove the composed usable endpoint.

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

Completed removal details and preserved behaviors: `5825f95d9:scratch/compare-cmux-s-command-line.md`,
this heading; pre-edit reconciled notes: `/tmp/loo427-shell-files-before/`.
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
- Jack Heart's latest selector direction deletes public `--target` and caller-supplied
  window/content/surface tokens, with no compatibility alias. Exact tokens stay internal.
  Replace public destination/toward JSON with pane selectors on the same retained owners.

## Implementation sequence — one PR

### Latest direction — October 9

Jack Heart's comments `0631ef7e`, `dfe4bc38`, `95ebc58c` and `22d93d69`:
finish/checkpoint the active remote companion edit, then simplify Desktop selectors,
then finish exclusive first-start admission. Existing delegation exchange and recorded-owner
observation must not be redone. Stacking on LOO-412 is authorized if necessary: inspect
its committed API/integration state and use supported lf operations, never dirty peer code.

Remote companions are checkpointed at `7aa8a5eb5`; selectors are implemented locally.
Existing Work resolution plus one Desktop inspection captures internal lifetime tokens.
LOO-412 frontier `a60d5594a` has identical `planning_git.rs`: publication/readback,
not exclusive admission. No stack/sync or dirty-checkout edit occurred.

Admission must replace the temporary refusal with an exclusive path through existing
planning/execution owners. Two independent Machines must not allocate/start one Task;
the winner survives retries, disconnects and delegation edits. No runtime replication,
Task transfer, duplicate Flow, publication or merge is authorized.

Remote companions now use `task shell --checkout` and `task watch-files --checkout` on
recorded execution. Shell admission rechecks Task/Machine/path before running the remote
login shell. The retained Files store consumes request-bound filesystem invalidations and
heartbeats over the existing SSH/line transport; retained documents reconcile disjoint edits.
Disconnected/stale streams cancel pending reads and retain drafts. Read replies recheck
location before/after transport; these observations are not filesystem compare-and-swap.
The cross-platform notifier adds no planning or execution store. Native/configured SSH remains.

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

Repository association retains locators; explicit Task/Project correspondence preserves
physical IDs. `66dd3c44f`/`2afcfba1a` compose lookup in one snapshot, not a reservation.
Neither association supplies remote location or first-start authority.

**Remaining identity/routing composition (October 9).**
`PlanningKind::fields()` carries grouped authored delegation, not RepositoryId.
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
- Recorded-location observation is implemented through `task location`, shared
  routing and opening/run/move/checkout previews. An owning Machine answers from one
  snapshot; request/repository/Task/Machine identity rejects stale replies. Added
  peers are read sequentially, with a 20-second deadline each, not per invocation.
  One positive owner overrides delegation even if another peer is unavailable;
  conflicting positives refuse. Missing/negative evidence never admits first start.
  This candidate set is not a global inventory or an exclusive reservation.
- Remote Task links now retain the local repository scene and carry Machine/repository
  identity. RegistryQuery re-observes the exact owner with a fresh nonce; unrecorded,
  stale, mismatched and unavailable replies prepare nothing. Only the addressed
  Session joins the retained inventory, never a second planning tree. Session and
  Files use the owner's Task correspondence and checkout, retaining local drafts.
  The SSH owner returns a terminal command preserving remote argv/Home and separate
  takeover. File input uses stdin, not the transport script. Headless fixtures cover
  these owners; remote shells/file observation now use that transport. Configured
  SSH and mounted native opening remain unproved.

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

Import never assigns the receiving Machine implicitly; the repository Work stream carries scoped
last-good destination receipts/holds. Per-Work sharing/authorship/assignee and
losing-edit recovery presentation remain. No second poller or destination-derived
repository identity. Associated origins preserve original journals, creation inputs,
receipts and private selection; only accepted joint frontiers parent saves.
Earlier composition and failures: `a44ddcb5f`, this section, and
[findings](findings.md#shared-planning-composition).

Foreground reconnect, lost-publication receipts and completion counterexamples:
`b2add0cf8:scratch/compare-cmux-s-command-line.md`, **Ancestry and foreground
composition** / **Post-LOO-418 completion boundary**. Fresh stream confirmation,
accepted-only frontiers and private holds remain required. Accepted Git mutations
supersede completion intent atomically with delivery; Linear retains its status-change
owner. Git confirmation never acknowledges provider delivery. Seeded execution,
disk drafts and one fake provider prove neither a running Flow nor native acceptance.

`787f440ea`/`50bf78f1e` exchange nullable Machine ID/time/provenance through
Placement, retaining conflicts, narrower/future inheritance, clearing, no-echo
and seeded started-checkout retention. Reaffirmation preserves explicit intent;
selecting a legacy assignment authors a save. Public Wave/file-Git and Store-level
Task/Project evidence, failed attempts and limits remain in
[findings](findings.md#delegation-exchange--october-9). No running Flow, exclusive
admission or public Task/Project-clearing proof follows.

**Required mixed-operation proofs:** execution-local `task run` and its Workflow
must coexist with repeat Git exchange, disconnect/reconnect, comments/follow-ups,
completion and causal reopening. Seeded state is insufficient: preserve running
controls, divergent execution history and pending inputs. Rejected provider facts
must preserve displayed projection, not just provider cache; unresolved membership
must remain visible without blocking unrelated imports. No central Workflow write
or remote arrival acknowledgement.

### 3. One repository window and command explanation

**Local mechanics and remote Task opening composed; native proof remains.**
Missing recorded remote checkouts still refuse. A remote link without a Session
selects the existing primary-Session owner; it never prepares first start.
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
`--pane` select retained Work. One eligible pane in the selected Task is implicit; multiple matches
return choices without effects. Move/resize use plain peer-pane selectors.
`DesktopPaneTarget` and surface tokens stay internal. Resolve through existing
Work/pane owners; validate the captured window/content/surface before acting,
never retarget by later focus. Unique/ambiguous/explicit selectors and retained lifetime
refusals have headless proof. Typed Apple events retain synchronous MainActor validation.
Restore, split, move, resize and zoom preserve selection; focus is explicit.
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

**Bounded extraction composed October 9; native acceptance remains.**
`Package.swift` pins checksum-verified `a60e9e2-lf3`; Jack Heart's artifact
authorization grants no planning publication. The locked formatter writes complete
UTF-8 prefixes into fixed caller storage, never an unbounded fallback. Screen,
scrollback and selection include command-block selection. Exact target validation
allocates/cleans up nothing and never follows focus. Empty succeeds; limits remain
1–1048576 bytes, default 65536. Visibility is pane-local, not compositor evidence.
Selection/reset/teardown preserve pins; clipboard and Quick Look stay independent.

Earlier bounded-reader checks, failed builds and native limits:
`50bf78f1e:scratch/compare-cmux-s-command-line.md`, **Bounded extraction**, and
`aade49b61:scratch/findings.md`, **Terminal identity and extraction boundary**.
No installed app or user terminal changed; native acceptance remains.

**Exact input implemented locally (October 8).** `desktop text/key` reuse the pane
event and synchronous Machine-qualified surface owner; no clipboard, router,
allocation, cleanup, acquisition or focus fallback. Missing/exited/replaced surfaces
refuse. Focus, layout and drafts stay put; reads never authorize input.

Jack selected cursor insertion into the existing draft (Q3), with Enter separate.
Text rejects C0/C1 controls, including newline/Tab, to prevent accidental submission
without bracketed paste. No escape decoding, silent clear or occupied-draft refusal.
Active IME must finish first. Restrictions remain reversible in questions.md;
uncertain replies cannot authorize replay.

Wire/headless checks cover keys, identity, invalid text and stale/missing/nonterminal
contents. The owned two-shell native fixture covers insertion, submission, focus,
exit and replacement, but remains unexecuted without a display. Full mechanism and
evidence: `9cc0707b0:scratch/compare-cmux-s-command-line.md`, this heading.

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

Check: `cargo test` pane-selector unit/CLI tests (4), focused Desktop lifetime tests (4), `cargo clippy --all-targets -- -D warnings` — PASS; `7aa8a5eb5` retains companion checks; gate/CI broader/Linux checks, demo configured SSH/native usability.
