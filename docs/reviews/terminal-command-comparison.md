# cmux, lf / Loopflow for Mac, and herdr

Inline `# lf-doc: ambiguous` comments annotate retired spellings for today's
command checker; they were not part of the recorded trial commands.

Behavioral comparison for [LOO-427](https://linear.app/loopflow/issue/LOO-427),
2026-10-07 Pacific. Jack Heart asked for command-by-command judgments after an
agent arranged his work in cmux. Jack selected **at most one repository window
across machines**, with identity inspection, arrangement and targeted terminal
I/O in one LOO-427 PR. Command dispositions remain proposed product judgments.

The missing loop is **identify → open → arrange → observe → target input → verify**.
Loopflow already owns the work being arranged. It should not require an agent
to reconstruct that work from tab names, or reconstruct Desktop from screenshots.

**Have** = a sufficient counterpart for the stated job; **worse** = have it,
but with a material gap; **want** = no adequate counterpart and worth building;
**no** = do not add this capability to Loopflow. **★** marks the operations an
agent needs to arrange Jack's work. “No” commonly means run lf inside cmux/herdr.
Related variants share a row only when the judgment is the same.

Commands in cmux/herdr columns omit the executable prefix. These are observed
command names, not proposed lf spellings. `W`, `T`, `P`, `S` mean window,
workspace, pane, surface IDs in cmux; herdr IDs belong to its own objects.
Use returned IDs, not these placeholders. `—` means no counterpart found in the
installed help or reviewed Desktop code, not proof that no external tool exists.

October 9 implementation cutover: `lf wave show [WAVE]` owns the plan reading;
`wave workflow` and `wave edit-plan` replace public Project commands. Internal
Project identity and historical planning remain. `575bde5bc`/`367ce31a9` implement
this through the existing readers/writers; current/history text/JSON and
checkpointed-store fixtures cover effect-free reads. Earlier delta entries saying
Wave planning is incomplete describe their revisions, not the current checkout.
Explicit-Machine previews now reuse the transport without identity-probe writes,
credential preparation or Process recording (`31dbb3a68`/`942dab5d8`). Fixtures
running two real CLIs cover remote repository/Work reads through simulated SSH.
Addressed identity is validated once before early reads or dispatch; rejected
commands suppress fallback Process writes. `49effe9bf`/`82f9ed087` add effect-free
Flow context through shared definition, graph and prompt owners: only the initial
skill/router has current-snapshot input; future and repeat input is unavailable.
Commands never run to manufacture input. Resolved skill identity handles aliases
and captured operator steps without preparing checkouts. Local and two-CLI fixtures
cover branches/loops, command-first Flows, native argument preservation and unchanged
checkpointed stores. Configured SSH/native invocation remain unproved. Broader action
explanation, peer composition/admission and native acceptance remain; bounded
extraction is composed with lf3 as recorded below.
The later `4688a4d85`/`e3d5d46df` cut explains Task movement/restart through
captured Workflow validation, sharing Task selection/location with run explanation.
Session connect and interactive message-less resume have operation-specific
explanation without endpoint probes. End intent does not perform completion reconciliation or cleanup; JSON
preparation grants no takeover. Planning create/edit/comment now share input,
current-Project, scope and saved-record validation, reporting intended effects
without acquisition or delivery. Unknown initialization/provenance stays unavailable.
`42a6cb7f7`/`8157583a9` add refile/save validation: destination identity/current
Project and recorded-work restrictions, plus pinned path/revision/draft checks,
without planning locks or file recovery writes. Unknown destination initialization
remains unavailable. Checkout now shares preparation status/planning, local placement
proposal and restoration checks without fetch, lease or allocation. LOO-418 integration
reads Task-owned placement without requiring a PR and removes move/run `--force`. Explicit and
inferred targets retain the same intent; stack mutation and remote recovery remain
unperformed. Design-handoff validation is explicitly unavailable. Complete/reopen,
follow-up, abandon/delete, interrupt, `history show` and
skill/Flow/inline invocations still have identity-only `--explain`, not mutation
validation or a complete action plan. Agent `--context` is a separate input preview. These source
boundaries do not add native, configured-peer or installed acceptance.
`9cc0707b0`/`92d95edfe` add recorded execution-location observation through
`lf task location TASK --peers --json`. Shared Task routing consults owning Machines,
not imported checkout state or later delegation. Nonces reject stale replies;
missing/conflicting owners remain unavailable and cannot admit first start.
Task run/move/checkout and Desktop opening share location provenance, including
Started without a checkout path. Two-CLI/simulated-SSH evidence covers retained
checkout routing and effect-free previews, not running Flows or configured SSH.
At that revision, Desktop still refused remote owners and Swift only decoded location.
`faebb042e`–`5d55957c6` now compose remote Task opening through TaskLink,
RegistryQuery and the retained Session/Files owners. The repository scene keeps
local planning and drafts; only the addressed peer Session joins its inventory.
Fresh nonce-bound owner observations precede opening and Session connection;
stale, unavailable or changed-checkout readings fail without transfer. Session-only
links prepare the same Files owner as Changes links, using local cache identity
and the peer's command subject. Recorded query copies retain remote classification:
SSH closures alone cannot prevent accidental local watchers or recovery-folder opens.
SSH carries its script as an argument, preserving stdin and remote native argv/Home.
Headless fixtures and simulated SSH cover these boundaries, not mounted terminals
or configured peers. `7aa8a5eb5` adds owner-pinned remote shells and filesystem
invalidations through the existing transport; stale/disconnected readings retain drafts.
`2c5610ca4` replaces public JSON targets and lifetime tokens with `--repo`/`--task`
and optional `--pane`. One eligible pane is implicit; ambiguity lists choices without
mutation. Move/resize peers stay in that inspected workspace. Native/configured-peer
acceptance, exclusive first start, mixed-provider exchange, running-Flow composition
and broader explanation remain. The original host judgments remain unchanged.
Earlier command names below are dated evidence, not compatibility aliases.

## Evidence and version boundary

- **P**: exercised locally. Pane operations used only the disposable cmux window;
  help, discovery and herdr status reads are noted separately below. cmux
  **0.65.0 (108), dda24fbd2**. All nine `cmux help GROUP` screens, `--help`,
  `guide` and individual command help were inspected. No host source, tests,
  configuration or skill text was incorporated.
- **H**: installed help only; advertised support, not runtime proof. herdr
  **0.9.3** groups and relevant leaf help were inspected. `herdr status --json`
  reported a compatible running server (protocol 22); no live herdr panes were
  inspected or changed. Browser, remote, provider, account and restore commands
  were not exercised.
- **L**: installed lf **0.13.9** help/catalog and relevant reads; Desktop claims
  are source inspection at **35e759aaf**, not a native Desktop trial. This
  checkout already uses Machine, Process and global `--machine`, ahead of the
  installed binary (`home`, `ssh`, `flow show --sessions`). Source also exposes
  `flow show --processes`. Exact older commands below are explicitly the installed
  baseline; they are not current design recommendations.
- Source rechecked at **7e852defe** on October 7: root `lf open # lf-doc: ambiguous` now launches
  the app and rejects non-macOS, but lacks a terminal alternative and Work target.
  HTML `lf screenshot` is removed. Repository-window addressing, remote Desktop
  opening and programmatic pane control remain absent. Global `--machine` uses
  one saved repository; it does not establish cross-machine correspondence or
  route arbitrary repository locations. Earlier installed spellings below remain
  baseline evidence.
- **October 8 source delta (through `a342d6870`):** context explanation shares
  recorded-checkout resolution; Desktop inspection reads plan-keyed windows,
  cancellation-safe queues and retained layouts. Task control and Session actions
  keep independent availability and Task dates; Session observation time stays
  unavailable. Exact-target focus, split, move, resize, zoom, hide/restore and local
  Shell/Files/Flow-log companions reuse the retained owners. Move preserves the
  leaf; zoom grants no focus; Undo renews closed content tokens. Pane insertion
  publishes command/focus/Undo together. Machine-qualified terminal and Session
  reading keys keep same-ID peers separate; membership pruning releases no surface.
  `desktop read` validates pane and native surface incarnations without allocation
  or exit cleanup. Missing differs from empty; existing surfaces explicitly report
  `bounded_reader_unavailable` under pinned lf2. The bounded Ghostty patch passed
  Linux formatter checks and embedded C compilation/export, not native extraction.
  The framework remains unbuilt/unpublished; Swift truncation is no fallback.
  Direct model smoke/typechecking passed; packaged attempts timed out without
  results. The later exact-input cut adds `desktop text/key` through the same pane
  event, validating the existing live surface and preserving focus/drafts. Text
  rejects control bytes; explicit keys remain separate. The native draft/cursor
  fixture is unexecuted. Native extraction, peer composition, complete opening and
  native acceptance remain unfinished. Detailed implementation and contrary check evidence:
  `a342d6870:docs/reviews/terminal-command-comparison.md`, this section.
  **October 9 bounded-reader update:** lf3 is published with a verified public
  download and selected in SwiftPM. Exact-target reads now use its fixed buffer;
  inspection advertises `read`. Removed the temporary unavailable-reader result.
  Headless composition checks pass; native extraction/draft fixtures compile but
  remain unexecuted. The earlier stalls and native acceptance gap still stand.
  The later CLI cut moves the app launcher to `desktop open` and inspection to
  `desktop list`; bare `open` is ambiguous with PR opening (Sessions use
  `connect`). This does not establish composed Work opening or native readiness.
  The tables retain October 7's baseline judgments; no cmux/herdr behavior was
  re-exercised for these deltas.
- **October 9 source delta (`87180d962`):** Machine-local repository root/path
  lookup and scoped Task IDs now feed downstream dispatch. Local skill/inline
  `--context` shares launch assembly, including bound Work and unwritten excerpt
  sources; `--explain` reports local identity before admission. Focused fake-provider
  input parity and absent-storage checks passed. Remote/Flow-wide preview, broader
  action explanation, Wave planning and composed opening remain incomplete.
  This updates source capability, not the installed baseline or host judgments.
- **October 9 opening delta (`ef179762d`, `c633589e0`, `e49e4ccf3`):**
  `desktop open --session ID --diff` resolves Task-associated Work without CLI
  checkout preparation or automatic execution-Machine routing. One Task link carries
  Session and Changes intent to the repository-keyed window, retaining panes and
  complete Session inventories. Launch acceptance returns `opening`; inspection's
  latest outcome now follows Session/surface readiness, comparison loading and the
  original visible panes. Navigation cancels pending requests; generation checks
  fence superseded requests, including repeated URLs. Session, Files and multiplexer
  observations replace notification/revision bridges. The recorded 48 headless tests
  include 15 opening cases with simulated native callbacks, not rendered endpoints.
  `1dd8230d0`/`e3ca861b1` then add inspectable pre-registration repository failures
  and mounted Task-page readiness. Registration retains validated request IDs;
  superseded success/failure/cancellation cannot settle newer requests. Pane
  closure remains observable before Session readings arrive. The later direct
  headless run records 60 passing tests, not native or remote acceptance.
  Remote composition and native endpoints remain. Historical tables and
  cmux/herdr judgments are unchanged.
- **October 9 preview delta (`b7f7a234c`–`af34dd20c`):** Task-run explanation
  shares Workflow selection/admission with launch; Desktop opening explanation
  shares URL validation and reports platform impediments. Proposed URLs are not
  readiness receipts. Parsed previews suppress fallback Process writes; a
  checkpointed-WAL regression exposed writes missed by main-file comparisons.
  Launch/preview share definition syntax, and builtin dispatch retains parsed
  scoped IDs. The recorded build/Clippy and 26 focused tests passed; broader action,
  remote/Flow input and Wave planning remain incomplete. No installed or native
  acceptance follows, and the original host judgments remain unchanged.
- Public [cmux API](https://cmux.com/docs/api) and
  [concepts](https://cmux.com/docs/concepts) provide context, but the tables below
  use the installed command discovery and our local observations. The public API
  page's default socket differs from the installed CLI's state-directory socket;
  neither path is an API requirement for Loopflow.

## Current decisions that govern this comparison

The LOO-387–430 briefs and their available non-progress comments were read on
October 7; later comments override earlier brief wording. LOO-389's comment read
failed because its Initiative was not bound to a local Wave; its brief was read.
LOO-170's older planning read also failed on conflicting Project facts; its
retained execution is abandoned. Neither failure was repaired as a prerequisite.

- **Machine, not Home:** LOO-394 changed the concept throughout the model;
  LOO-411 replaced `lf ssh` with `lf --machine LABEL COMMAND`. A machine is one
  OS user and data directory; filesystem `LF_HOME` and historical opaque IDs keep
  their meanings. No stable cross-version API or capability negotiation is wanted.
- **One repository window across machines:** Jack's direction in this conversation.
  Reopen/focus by repository; worktrees stay inside it. Closed repositories need
  no window. Keep every machine's work identity, authority and observation age
  explicit. This changes presentation, not LOO-394/417's separate execution state.
  Jack clarified that the window reads one shared Work model, whose subtrees
  can be delegated to Machines. Work identity supplies the relationship; a
  separate repository-pairing registry is withdrawn. Execution stays machine-local.
- **macOS-only Desktop operations:** Jack requested useful Linux errors for
  Desktop launching. Opening/control must report the unsupported platform before
  launching anything or changing Work, and offer the ordinary terminal path.
  Linux still supports lf's Tasks, Sessions and machine routing. The Mac displaying
  remote work is not the machine executing that work; preserve that distinction.
- **No new host:** LOO-415 owns a transparent per-Session relay, no emulation;
  Codex keeps its engine socket. Desktop screen reads are of its actual terminal,
  not a requirement to turn the relay into a terminal emulator. LOO-398 is the
  status read half; emission is deferred to LOO-415, with LOO-422 interoperability.
- **Current command/product boundaries:** LOO-401 selects `self`, `config user`,
  app opening, and outright HTML screenshot removal. LOO-418 selects zero or one
  PR per Task and follow-through as another Task. LOO-406 owns the accepted local
  planning writer and optional Linear sync. Jack selected execution-machine local
  operations and custom Git-ref Task synchronization on October 8; LOO-412 owns
  exchange. Workflow, checkout and process authority remain local. The shared
  model is selected; writer/exchange integration is tracked in the working
  design. Selecting the prototype authorizes no publication of real planning data.
  Later LOO-406 memory (`f027890ab`) records Jack's user-keyed plan default,
  explicit shared-plan opt-in and Linear conflict precedence. A shared code
  remote never implicitly merges plans; LOO-412 owns binding and ordering.

The [owner map](#implementation-scope-and-existing-owners) below retains the
related Tasks' boundaries; this Task exposes the existing workspace owners.

## Open and arrange

| Job | cmux command (evidence) | lf / Desktop today (L) | herdr command (H) | Judgment and owner |
|---|---|---|---|---|
| Open on a supported platform | Mac app control; no Linux Desktop trial | Source `lf open # lf-doc: ambiguous` rejects non-macOS; no terminal alternative yet | Terminal host, not a Mac app launcher | **worse** — explicit, actionable Linux errors for Desktop operations; LOO-426/427. |
| ★ Open a working directory | `<path>`, `open`, `new-workspace --cwd PATH` (P for new-workspace) | Installed `lf desktop`; source `lf open # lf-doc: ambiguous` launches the app only; `open 'loopflow://open?repo=PATH'`; Task links below | `workspace create --cwd PATH`; `worktree open` | **worse** — app launch and Work opening are separate; LOO-426. |
| ★ Open the actual Task/conversation | `new-workspace --command CMD`; `new-surface --command CMD` (P) | `open 'loopflow://task/ISSUE?repo=PATH&session=ID'`; no installed `lf open ISSUE # lf-doc: ambiguous` | `tab create`, then `pane run P CMD` | **worse** — Task links already preserve Work identity, but lack the one-command entry; LOO-426. |
| ★ Open everything waiting | `jump-to-unread` (H); notifications are not Work Waiting | `lf session list --waiting --json`; no batch Desktop open | `agent list`; no matching batch open found | **want** — open shared Waiting conversations once, not every completed Run; LOO-426. |
| ★ Open/identify the repository window | `new-window`, `list-windows`, `current-window` (P except current-window) | Native windows; no addressable window inventory | `session list`; terminal-host windows are outside herdr | **want** — at most one window per repository across machines; repeated opens reuse it and commands address the repo; LOO-427 arrangement slice. |
| ★ Focus an existing destination | `focus-window`, `select-workspace`, `focus-pane`, `focus-panel` (P for select-workspace) | Task link; sidebar, palette and pane click; no exact-window/pane CLI | `workspace focus`, `tab focus`, `pane focus --direction`, `agent focus` | **worse** — opening is addressable, existing pane focus is not; LOO-426 plus LOO-427 arrangement. |
| ★ Add a companion pane/tab | `new-pane`, `new-surface`, `new-split` (P for last two) | **+** shell / Files / Flow log; split controls; no CLI | `tab create`, `pane split` | **want** — expose the existing Task workspace, not a parallel terminal hierarchy; LOO-427 arrangement. |
| ★ Move/reorder within the workspace | `move-surface`, `split-off`, `drag-surface-to-split`, `reorder-surface` (H) | Retained multiplexer; no public programmatic placement | `pane move`, `pane swap` | **want** — rearrange existing views without restarting them; LOO-427 arrangement. |
| ★ Promote a tab to a sidebar entry | `move-tab-to-new-workspace` (P) | Tasks already own sidebar placement; a Session belongs to its checkout's Task | `pane move P --new-workspace` | **no** — do not create Tasks by moving tabs; open/select the existing Task through LOO-426. |
| Move entire workspaces between windows | `move-workspace-to-window` (H) | Window-local surface pools; another window may see a client elsewhere | `pane move P --workspace ID` is within a server, not an app-window transfer | **no** — no arbitrary multi-window workspace system; one repo window spans machines. LOO-424 owns client transfer. |
| Arbitrarily reorder/pin/color sidebar entries | `reorder-workspace`, `reorder-workspaces`, `workspace-action` (H) | Work outline derives from Waves and started Tasks | Workspace focus/rename; no equivalent ordering CLI found | **no** — keep the Work hierarchy; run lf inside cmux for host organization. |
| ★ Resize/zoom an existing layout | `resize-pane`, `resize-window`; `tab-action` (H; action-dependent) | Native resize and pane zoom; no CLI | `pane resize`, `pane zoom` | **want** — expose pane sizing/zoom; OS window geometry need not be cloned; LOO-427 arrangement. |
| ★ Hide/close a view, then restore it | `close-surface`, `close-workspace`, `close-window` (P for window) | Close view / Undo; shell closure and app quit can end PTYs; no CLI | `pane close`, `tab close`, `workspace close` | **want** — explicitly separate reversible visibility from ending a shell/client; LOO-427 arrangement. |
| Host navigation shortcuts | `next-window`, `previous-window`, `last-window`, `last-pane`, `find-window` (H) | Sidebar and ⌘K use Work names | `pane neighbor`, `pane edges`, directional focus | **no** — exact target focus is useful; tmux-shaped aliases are not a new Work API. |
| Terminal maintenance/compatibility | `refresh-surfaces`, `swap-pane`, `break-pane`, `join-pane`, `respawn-pane`, `clear-history`, `copy-mode` (H) | Split/close/Undo and native selection; no matching CLI | `pane swap`, `pane move`, `pane close` | **no** — retain narrow arrange operations above; leave terminal administration to the host. |

## Inspect identity, relationships and output

| Job | cmux command (evidence) | lf / Desktop today (L) | herdr command (H) | Judgment and owner |
|---|---|---|---|---|
| ★ Explain the command's target | `identify --json` (P) | Source: `lf machine id`; installed: `lf home id`; `lf task status --json`; `lf context` lacks full selection provenance | `pane current`, `agent explain` | **want** — one shared Work identity explanation; LOO-427 identity slice. |
| ★ Inspect visual placement and selection | `tree --json`, `list-workspaces`, `list-panes`, `list-pane-surfaces`, `list-panels`, `current-workspace` (P for tree/workspaces/panels) | No Desktop tree read; Work membership is available via `lf task status` | `api snapshot`, `workspace get`, `tab get`, `pane list`, `pane get`, `pane layout` | **want** — join window/pane placement to Work IDs without replacing Work membership; LOO-427 arrangement. |
| Inspect actual work/process ancestry | `top --processes`, `current`, `memory`, `surface-health` (H) | `lf ps --json`, `lf top`, `lf task status --json`; Task Flow log | `pane process-info`, `agent list`, `agent get` | **have** — Work-associated processes are already stronger than a terminal process tree; resource profiling stays with host tools. |
| ★ Read the displayed terminal | `read-screen --surface S --lines N`, `capture-pane` (P for read-screen) | Native terminal display only; `lf monitor show ID --final` is a recorded conclusion, **not** the screen | `pane read P --source visible --lines N`; `agent read` | **want** — bounded passive reads of exact rendered surfaces; keep the relay byte-transparent under LOO-415. LOO-427 terminal I/O slice. |
| ★ Read selection or scrollback | `read-selection`, `read-screen --selection`, `read-screen --scrollback` (H) | Copy/selection and scrolling in UI; no targeted read API | `pane read --source recent`; no selection command found | **want** — useful for discussing what Jack is looking at; LOO-427 terminal I/O slice. |
| Read durable conversation history | `sessions`, `vault sessions`, `vault search`, `vault checkpoints`, `recover` (H) | `lf session list --search TEXT`, `lf session history ID`, `lf monitor show ID --events` | `session list` is host lifetime, not provider history; `agent read` is terminal output | **worse** — shared history exists; outside-lf adoption/name-based continuation belongs to LOO-424. |
| Capture a native window | `shot` (P); `record start/stop/status/note/list` (H) | AppleScript `capture screenshot` captures the key window, not an explicit target; installed `lf screenshot` captured HTML/URLs and is now removed | — | **worse** — current capture cannot establish which addressed window rendered; support explicit capture with Desktop inspection, not a video editor. |
| Inspect patch/files beside work | `diff`, `markdown`, `open` (H) | `lf task diff ISSUE`, `lf task files ISSUE`, `lf task file ISSUE PATH`; Files/Diff pane | Shell/editor; no equivalent viewer command found | **have** — Task-backed file and diff identity is already useful; formatted Markdown is optional, not a new comparison Task. |
| Review comments/findings | `comments list`, `review list`, `review show`, `review findings` (H) | `lf task comment ISSUE`, Task Description/comments, PR links, skills | Shell tools; no parallel review commands found | **have** — preserve authored Task/PR review; no host-specific review database. |
| ★ Discover version support vs legal action | `capabilities`, `ping`, `version`, `socket-status` (P except socket-status) | `lf --version`, `lf list --json`; shared Session/Task action projections; no app capability read | `api schema`, `status --json` (P for status only) | **want** — local operation discovery and legal actions, without cross-version negotiation (LOO-411); Desktop arrangement and LOO-397. |

## Send input and wait

| Job | cmux command (evidence) | lf / Desktop today (L) | herdr command (H) | Judgment and owner |
|---|---|---|---|---|
| ★ Insert text into an exact terminal | `send --surface S TEXT`, `paste`, `send-panel` (P for send) | Native typing/paste; no Desktop CLI | `pane send-text P TEXT` | **want** — explicit surface/content target; share LOO-387's draft preparation boundary. LOO-427 terminal I/O slice. |
| ★ Submit/interrupt with a key | `send-key --surface S KEY`, `send-key-panel`, `paste --submit` (P for send-key) | UI keys; `lf task interrupt ISSUE` is a provider action, not a general key API | `pane send-keys`, `pane run` | **want** — separate insertion from execution; no implicit Enter or transfer of client authority; LOO-427 terminal I/O slice. |
| Continue a conversation semantically | `agent message`, `agent inbox`, `agent messages` (H; host messaging, not provider continuation) | `lf -b session resume ID MESSAGE`; ongoing interactive Session; Task steers via `lf task comment --steer` | `agent prompt TARGET TEXT` | **have** — durable conversation/steer path already exists; do not add another agent inbox. Live terminal input is the separate row above. |
| Wait for a known outcome | `wait-for`; `pipe-pane --command CMD` (H) | `lf task wait ISSUE --until terminal --timeout N --json`; `lf monitor work --json --watch` for state changes | `pane wait-output --match TEXT --timeout MS`; `agent wait`; `agent prompt --wait` | **worse** — Task completion waits exist; terminal read/input needs bounded observation, not LM polling or a new completion authority. LOO-427 terminal I/O slice. |
| Host clipboard buffers and popups | `set-buffer`, `list-buffers`, `paste-buffer`, `popup`, `display-message` (H) | System clipboard and terminal paste; no buffer manager API | Native terminal clipboard | **no** — run lf inside cmux; do not add another clipboard store. |

## Name things, status and attention

| Job | cmux command (evidence) | lf / Desktop today (L) | herdr command (H) | Judgment and owner |
|---|---|---|---|---|
| ★ Name the conversation | `rename-tab` (P), `rename-workspace`, `rename-window`, `tab-action --action rename` (H) | `lf session rename ID NAME`; Desktop inline Session rename | `agent rename`, `pane rename`, `tab rename`, `workspace rename` | **have** — durable Session name is the useful identity; avoid a competing pane-title database. Host propagation belongs to LOO-422. |
| Rename the work itself | `rename-workspace` (H; a visual label) | `lf task edit ISSUE --title TITLE`, `lf wave rename`; different semantic effects | `workspace rename` (a visual label) | **have** — use existing Work mutations deliberately, never turn pane rename into Task rename. |
| ★ Show current step/status | `set-status`, `clear-status`, `list-status`, `sidebar-state` (P for set/sidebar) | Task Workflow and Flow log; `lf monitor work --json --watch`; incomplete external-host propagation | `pane report-agent`, `report-agent-session`, `release-agent`, `report-metadata`; `workspace report-metadata` | **worse** — consume shared execution and Waiting; LOO-422 owns external-host fidelity, not arbitrary host-specific state writers. |
| Set a workspace lane manually | `workspace status [set …]` (H) | `lf task move ISSUE NODE` changes Workflow position; not a visual label | Display metadata and lifecycle reports are distinct | **no** — never let a terminal lane overwrite Work lifecycle or Waiting. |
| Show arbitrary percentage/logs | `set-progress`, `clear-progress`, `log`, `clear-log`, `list-log` (P for set-progress/log) | Recorded Process results and Session history; no arbitrary progress setter | Display metadata; no matching percentage/log CLI found | **no** — preserve observed step/status; do not invent percent-complete truth. Host-native decoration can remain in cmux. |
| ★ Signal an actual request for input | `notify --desktop false`, `list-notifications`, `open-notification`, `jump-to-unread` (P for notify only) | Shared Waiting and `lf session list --waiting`; Desktop Waiting presentation | `notification show TITLE --sound request`; agent state | **worse** — LOO-402 owns sidebar/jump including blocked shell panes, LOO-426 opens waiting Sessions, LOO-422 external status. No new Task. |
| Dismiss a host notification | `dismiss-notification`, `mark-notification-read`, `clear-notifications` (P for scoped clear) | No equivalent notification queue; Session/Task completion are not dismissals | No notification-history controls found | **no** — no second attention queue or “read means done” contract; host owns its notifications. |
| Associate a pull request | `pr URL`, `pr clear` (H) | Recorded PR, `lf pr checks`, Task PR link; LOO-418 selects zero or one PR per Task | Display metadata / shell tools | **have** — keep typed PR ownership, not a second editable host PR authority. |

## Agents, Sessions and resume

| Job | cmux command (evidence) | lf / Desktop today (L) | herdr command (H) | Judgment and owner |
|---|---|---|---|---|
| Start agent work | `new-surface --type agent-session`; `claude-teams`, `codex-teams`, `omo`, `omx`, `omc` (H) | `lf skill NAME`, `lf run FLOW # lf-doc: ambiguous`, `lf --task ISSUE : MESSAGE`; Desktop New Session | `agent start NAME --kind KIND --pane P` | **have** for lf-owned skills/Flows; LOO-420 owns native/cross-harness skill fidelity. LOO-428 owns known host launch failures; no team-runner clone. |
| Detect/report the running agent | `hooks setup/uninstall`, `hooks AGENT install/uninstall/event`, `hooks feed` (H) | Owned provider streams plus terminal reports; host interoperability incomplete | `integration install/uninstall/status`, `agent explain`, lifecycle reporting | **worse** — LOO-422; do not install host hooks or create duplicate status authority here. |
| Hibernate the agent/terminal | `agent-hibernation on/off/hibernate/wake` (H) | No equivalent transparent suspension; provider continuation is distinct | Persistent server and `agent attach`; not evidence of hibernation | **no** — leave host lifecycle in cmux/herdr; continuation work belongs to LOO-424. |
| Resume saved provider work | `restore`, `surface resume set/show/get/clear`, `session restore`, `recover` (H) | `lf session resume ID`, `lf session connect ID`; Desktop Open / Move here | `agent attach`, `session attach NAME` retain a host terminal, not necessarily provider history | **worse** — LOO-424 owns adoption, names, directories and host portability. |
| Fork/checkpoint a conversation | `fork`, `vault checkpoint`, `vault fork` (H) | `lf replay ID` repeats a captured request; not a provider-history fork | — | **no** — do not equate replay with fork; no demonstrated Work requirement for a checkpoint product here. |
| Restore/export an entire host session | `restore-session --from/--export`; `local-tmux`, `tmux attach`, `local-zellij` (H) | Retained layout and durable Session identity; LOO-416 owns saved per-Task pane restoration and reattachment | `herdr --session NAME`, `session attach/stop/delete/list` | **no** to host export/tmux management — run lf inside the host; LOO-416 owns app restoration, LOO-415/424 continuity. |
| Route models/accounts through host | `coderouter` / `cr`, `ai-accounts` (H) | `lf account` has its own provider-account model | Agent launch arguments/integrations | **no** — no CodeRouter/account upload integration from this comparison; never exercise real accounts in tests. |

## Browser and remote

Every entry in this section is **H**, not an exercised browser/remote workflow.
These commands are inventoried because they are discoverable, not because their
implementation or runtime quality was examined.

| Job | cmux commands | lf / Desktop today (L) | herdr commands (H) | Judgment and owner |
|---|---|---|---|---|
| Own an embedded browser | `enable-browser`, `disable-browser`, `browser-status`; `browser enable/disable/status`, `open`, `open-split`, `tab new/list/switch/close` | No embedded browser; ordinary external links | — | **no** — run lf inside cmux or use the browser tool. |
| Navigate and inspect a page | `browser goto/navigate/back/forward/reload`, `url/get-url`, `identify`, `snapshot`, `get`, `is`, `find`, `frame` | Installed `lf screenshot` is capture only; LOO-401 removes it without replacement | — | **no** — no second browser automation product in Desktop. |
| Interact with page controls | `browser click/dblclick/hover/focus/check/uncheck/scroll-into-view`, `type/fill/press/keydown/keyup/select/scroll`, `wait`, `dialog` | External browser tooling | — | **no** — use the host's browser. |
| Debug/customize web UI | `browser eval/repl`, `devtools`, `react-grab`, `focus-mode`, `design-mode`, `zoom`, `highlight`, `addinitscript/addscript/addstyle`, `console/errors` | External devtools | — | **no** — use the host's browser. |
| Capture/download web results | `browser screenshot`, `download list/wait` | Installed `lf screenshot`; removal decided in LOO-401 | — | **no** — LOO-401 removes HTML capture without replacement; use available external tools. |
| Own browser profiles/data | `browser profiles list/add/rename/clear/delete`, `import`, `cookies get/set/clear`, `storage`, `state save/load`, `history clear` | No Desktop browser account store | — | **no** — host owns it; no credentials/history read in this study. |
| Run work on another machine | `ssh`, `mosh`, `mosh-tmux`, `ssh-tmux`, `remotes` / `remote` (H) | Source: `lf --machine LABEL COMMAND`, `lf machine add/list/status/rename/remove`; installed baseline still has `lf home ssh/observe` | `--remote TARGET`, `--machine LABEL`; `machine list/status/add/rename/remove/enable/disable/reconnect` | **have** for remote lf execution; host shells/tunnels remain with cmux/herdr. |
| Continue a Session elsewhere | `session move`, `ssh-session-list`, `ssh-session-attach`, `ssh-session-cleanup` | Machine routing and provider resume pieces, not universal live transfer | `--remote`, `agent attach`, `session attach` | **worse** — LOO-424 owns experience; LOO-412–417 own machine/relay foundations. |
| Own cloud machines | `auth status/login/logout/team`, `login/logout`; `vm` / `cloud` (all subcommands below) | Machine operations; no cmux Cloud product | Saved SSH machines; no cloud lifecycle group | **no** — no cloud-vendor platform in this Task; run lf on the provided machine. |
| Drive mobile/simulator UI | `simulator`, `ios` | No equivalent Desktop terminal-host API | — | **no** to simulator control — LOO-396 separately owns phone access to waiting work. |
| Diagnose remote transport | `remote-daemon-status`, `iroh-diag` | `lf doctor`, SSH errors | `machine status`, `status server/client` | **no** for another transport diagnostic stack; keep diagnostics with the transport owner. |

The `vm`/`cloud` disposition covers every leaf advertised in top-level help:
`base`, `new`, `ls`, `domains`, `tree`, `self`, `status`, `stats`, `resize`,
`network`, `agent-updates`, `rename`, `pause`, `resume`, `snapshot`, `fork`,
`restore`, `rm`, `run`, `route`, `agent`, `dev`, `prompt`, `exec`, `push`, `pull`,
`wait`, `shell`, `tui`, `desktop`, `open`, `workspace`, `terminal`, `tab`,
`layout`, `env`, `ports`, `tools`, `handoff`, `promote-template`, `attach`, `ssh`,
`ssh-info`. Listing is not verification of these commands or their subtrees.

## Automation, discovery and host administration

| Job | cmux commands (H unless marked) | lf / Desktop today (L) | herdr commands (H) | Judgment and owner |
|---|---|---|---|---|
| Learn commands from the tool | `help`, `help GROUP`, `guide` / `--skill`, `docs` (P for help/guide) | `lf help --all`, `lf help PATH`, `lf list --json`; ambiguity/typo rough edges | `--help`, leaf help, `--skill`, `completion` | **worse** — LOO-397 owns learnability; do not clone another tool's help or skill text. |
| Observe changing Work | `events` | `lf monitor work --json --watch`; `lf activity --json` | `api snapshot`; waits for pane/agent conditions | **have** — Work events already feed Desktop; a visual layout snapshot belongs in LOO-427 arrangement. |
| Schedule/run automation | `automation list/show/test/enable/disable/logs/reload`, `set-hook` | `lf wave cron`, `lf run FLOW # lf-doc: ambiguous`, `lf task run` | Shell tooling; integration lifecycle hooks | **have** — existing Work automation suffices; no second host automation engine. |
| Use a host-specific automation service | `glaeda request/observe` | No Glaeda-specific equivalent | — | **no** — run lf inside the host; no integration need was demonstrated. |
| Keep an ad hoc host todo list | `todo add/list/check/uncheck/start/rm/clear` | `lf task create`, Task Workflow and Project plan | No todo group found | **no** — preserve Work planning; LOO-406 owns the local planning lifecycle, not a terminal-local backlog. |
| Customize terminal appearance | `themes list/set/clear`, `import`, `reload-config`, `shortcuts`, `bind-key`, `unbind-key` | Desktop Settings and fixed workspace composition; no general terminal configuration CLI | `config`, `server reload-config`, `channel`, `completion` | **no** to a host config clone — LOO-403 already owns discoverable Desktop shortcuts and palette actions. |
| Replace host sidebars | `right-sidebar`, `sidebar templates/try/new/validate/reload/select/open` | Work outline, Files and Flow log panes | Workspaces/tabs/panes | **no** — expose existing Work views rather than build a sidebar extension platform. |
| Host settings/onboarding/feedback | `welcome`, `settings`, `config doctor/check/validate/path/paths/docs/documentation/reload`, `feedback`, `feed tui/clear` | Settings, `lf doctor`, repository onboarding | `config`, `update`, `channel`, server lifecycle | **no** to host administration parity; Loopflow keeps its own installation/diagnostics. |
| Raw RPC/debug controls | `rpc`, `debug-terminals`, `trigger-flash`, `set-app-focus`, `simulate-app-active`, `simulate-sidebar-drag` | Headless app tests and native fixtures; no general Desktop transport | `api schema`, `api snapshot` | **no** to a public debug-control clone; ship the explicit supported operations above. |
| Privileged execution | `sudo run/pending/setup-touch-id` | Ordinary process/OS permissions | Ordinary shell | **no** — not needed to arrange Work; never build a privilege broker for this comparison. |

## What the disposable exercise established

The exercised command sequence used explicit window/workspace/surface IDs after
creation: `new-window`; `rpc window.create`; `new-workspace`; `new-surface`;
`select-workspace`; `send`; `send-key`; `read-screen`; `rename-tab`; `new-split`;
`move-tab-to-new-workspace`; `set-status`; `set-progress`; `log`; `sidebar-state`;
`identify`; `tree`; `notify --desktop false`; `shot`; `clear-notifications`;
`close-window`. No command targeted Jack's existing terminal surfaces.

- The first window command returned an ID which later returned “Window not
  found.” Cause unknown; it was absent from the final inventory. A second
  `rpc window.create` produced an inspectable separate window.
- A harmless shell command emitted `LOO427_PROBE`. After moving its tab into
  another sidebar workspace, the same surface retained the output. Sending
  literal `LOO427_DRAFT`, switching away and reading that explicit surface
  retained the unfinished draft. No provider was launched.
- `identify` distinguished the explicitly addressed surface from the selected
  workspace. `tree` reported three workspaces and the two-pane split. A hidden
  surface could report `not_started` render health while its text remained
  readable: a screen buffer is not proof of currently rendered content.
- [The captured test window](terminal-command-comparison/cmux-disposable.png)
  visibly shows the split, named sidebar entries, notice, status, log and progress.
  It proves this rendered arrangement, not Desktop parity or provider readiness.
- The text command's escaped newline was interpreted unexpectedly and produced
  a continuation prompt before the marker. Future send contracts must distinguish
  literal text from key/escape interpretation; do not infer byte fidelity from
  an `OK` response.
- Ordinary window close refused because our nested shells were running.
  `close-window --window TEST_ID --force` removed only the test window after its
  scoped notification was cleared. Final window inventory contained the original
  window only, still with seven workspaces. Its selected workspace changed during
  the study; no original selection was restored over Jack's concurrent activity.

No comparable Desktop run, herdr pane exercise, provider continuation, browser,
SSH/cloud/account operation or latency benchmark was performed. A test-window
screen and command receipts do not meet the chapter's sustained-use KRs.

## Implementation scope and existing owners

LOO-427 delivers the identity → arrange → observe/input path in one PR;
premature LOO-430/431/432 are folded back. Jack Heart authorized advancement
through the demo review boundary. Jack subsequently selected cursor insertion into
existing drafts, with Enter separate. On October 9 Jack accepted nearest-explicit-
ancestor delegation for unstarted Tasks, with narrower Task/sub-Wave overrides;
started Tasks stay on their recorded Machine. Unknown historical assignment
provenance stays unknown. Current transport, command spelling and implementation
limits are in the working design at
`scratch/compare-cmux-s-command-line.md`. Browser, cloud, checkpoint vault,
custom sidebars and host terminal administration remain outside this diff.

Reuse these owners without duplicating their outcomes:

| Task | Boundary |
|---|---|
| [LOO-397](https://linear.app/loopflow/issue/LOO-397) | Command map and discovery. |
| [LOO-426](https://linear.app/loopflow/issue/LOO-426), [LOO-401](https://linear.app/loopflow/issue/LOO-401) | Task/Session and Waiting opening through existing links; launch-command move. |
| [LOO-416](https://linear.app/loopflow/issue/LOO-416) | Saved panes and restored attachments; reuse the layout store. |
| [LOO-402](https://linear.app/loopflow/issue/LOO-402) | Waiting navigation, including blocked shells: Rust judges Sessions, shared Swift rolls up shell reports. |
| [LOO-403](https://linear.app/loopflow/issue/LOO-403), [LOO-387](https://linear.app/loopflow/issue/LOO-387) | Shortcuts/palette and native draft preparation. |
| [LOO-422](https://linear.app/loopflow/issue/LOO-422) | Host titles/status through standard reports. No host-specific integration when the host consumes none of the existing mechanisms, or arbitrary cmux progress setters. |
| [LOO-424](https://linear.app/loopflow/issue/LOO-424), [LOO-415](https://linear.app/loopflow/issue/LOO-415) | History adoption/resume across hosts and machines; relay and machine Tasks provide transport. |
| [LOO-421](https://linear.app/loopflow/issue/LOO-421), [LOO-423](https://linear.app/loopflow/issue/LOO-423) | Actual lf-in-host trials and small repairs; this host-only exercise does not replace them. |
| [LOO-428](https://linear.app/loopflow/issue/LOO-428) | Duplicate launch flags/noisy output, retaining argument delivery. |
| [LOO-429](https://linear.app/loopflow/issue/LOO-429) | All assembled context in the system file; no split, relaunch fallback or lf-side interactive error wording. |
| [LOO-420](https://linear.app/loopflow/issue/LOO-420) | Native invocation for native skills, translated ports across harnesses, builtins inlined. No new wrapper/prompt workaround here. |

## Loopflow source anchors

- [Task links and native controls](../../swift/README.md),
  [link dispatch](../../swift/LoopflowMac/LoopflowApp.swift),
  [destination handling](../../swift/LoopflowMac/WorkDestination.swift).
- [Retained workspaces](../../swift/LoopflowMac/Views/SessionsView.swift),
  [multiplexer](../../swift/Loopflow/Models/MultiplexerStore.swift),
  [terminal input/read primitives](../../swift/LoopflowMac/Services/Ghostty/GhosttyTerminalView.swift).
- [AppleScript dictionary](../../swift/LoopflowMac/Loopflow.sdef) and
  [handler](../../swift/LoopflowMac/ScriptCommands.swift): passive retained-window
  inspection plus key-window capture.
- [Shared bindings](../../rust/loopflow/src/ops/run.rs),
  [context command](../../rust/loopflow/src/lf/commands/context.rs),
  [RegistryQuery](../../swift/Loopflow/Services/RegistryQuery.swift).

Review finding: transcript reads, Task membership and successful dispatch were
initially tempting substitutes for screen reads, visual placement and usability.
The comparison keeps each pair separate; no new runtime owner follows from it.
