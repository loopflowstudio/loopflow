# Open and restore a Loopflow workspace in Rex

**LOO-498 · revised build authorized · reconciled November 4, 2026.**

## Accepted direction

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

## Delete — do not maintain

- Rex away timers, native closure and native-settlement ownership are already absent
  from this branch. Do not reintroduce them to satisfy LOO-717's older API. Preserve
  creation receipts, named copies and unknown-inventory safety on the surviving path.
- Remove reliance on the configured default server for commands launched by LF into
  a known Rex destination. The launch argv carries that endpoint through native
  tools and shells; current-viewer placement still outranks provider startup state.
  Arbitrary pre-existing native terminals remain a separate discovery gap.
- Removed `Rex::launch_layout` and `herdr::herdr_layout`: shared tool rendering now
  emits the selected host's format directly. Rex adds its endpoint while constructing
  each command; Herdr no longer parses Rex JSON. Task/checkout context stays in one
  `Tools` value throughout traversal. Neither renderer reads active native layouts.
- No second retention store, shell runtime or mirrored Rex layout replaces these
  deletions. Shared lifetime integration requires the LOO-717 owner contract.

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

Source inspected at `bacac38d44`, including the November 4 remote-arrival changes:

- `open/workspace.rs` consumes shared selection/execution routing, serializes
  creation, reuses native IDs across renames and retains uncertain-create receipts.
  A found activated tab is unchanged; a confirmed missing tab can be explicitly
  recreated. Marker recovery recovers identity without granting replay permission.
- `HostWorkspace` holds destination/selection/copy identity and a pending create
  receipt. `HostOccurrence` is Task-specific, with a native destination and four
  creation states (`pending`, `passive`, `tools_pending`, `complete`). Neither
  stores a Rex split tree, retention policy or native liveness. Removed fields
  include duplicated Work identity, occurrence timestamps and persisted host
  observations. Herdr retains only its workspace locator and pending layout receipt.
- Host receipt tables no longer advance Work revisions. Wave-template writes still
  advance planning because their content affects shared layouts. The Task's one
  draft migration remains `rex_workspaces.sql`.
- Explicit passive activation adds selected tools once; a pending split reply
  prevents retry replay. The follower's OS lock proves only follower liveness.
  Reopening can create a new follower tab after exit while retaining the old tab.
  A subscription remembers arrived Tasks so membership changes do not copy them.
  Repository-created Wave containers follow independently. The follower now routes
  remote arrival requests through the existing SSH envelope, using shared scope
  filters to include Tasks whose remote checkout is absent from the local plan.
  The destination decides whether the Task started and validates its Machine/cwd.
  Each request is passive and fenced by existing creation receipts; remote
  followers stay local to avoid forwarding loops. A fifteen-second reconciliation
  request covers remote starts/reconnections without local Work changes. Configured
  two-Machine acceptance remains outstanding.
- Native host output is spooled during commands, avoiding wait-before-drain pipe
  deadlocks on large inventories. Shared workspace lookup reads rows in one query.
  Passive creation validates the recorded checkout before opening its Task viewer.
- The obsolete Herdr keystroke-resume path is deleted. Workspace painting never
  executes `ViewContent.argv`; conversation tools use passive `session view`.

These source facts and earlier isolated checks establish components, not complete
configured acceptance. No Rex retention, away, closure or settlement implementation
exists here; removing the old plan is not delivery of the new retention hold.

### Shared tools

| Intent | Current implementation |
| --- | --- |
| Conversation | `session view` observes live output or history; explicit `lf resume` takes input |
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

Native children supply REX_SESSION/REX_BLOCK without REX_SERVER, including on the
isolated non-default server. Current code resolves Rex's configured endpoint and
validates the caller block; recorded current-viewer placement can supply an endpoint.
LF-created native tools now carry the resolved endpoint in their launch argv,
including the Workspace follower and Wave conversation. A generated-shell fixture
executes the actual tool launcher and reopens with bare `lf open`, retaining the
same destination. This does not discover an arbitrary pre-existing terminal's
endpoint. A first direct call in an arbitrary custom server can still require `--server`,
which falls short of accepted bare-open behavior. A fresh isolated November 4
probe preserved Rex's native caller environment (`REX_SESSION`, `REX_BLOCK`,
`REX_BIN_DIR`) and ran unqualified `rex whoami`: it succeeded but identified a
different endpoint. Letting Rex's CLI keep those variables does not solve discovery;
caller-block validation remains necessary to avoid opening the wrong server. Existing source-placement and
direct-shell tests do not prove the actual LLM tool path after resume/move.

### Native split defaults

Jack's November 3 comment `691eb6f2-0bcc-4054-8625-536f1de23feb` requires native
shortcut/menu/API splits to start at the source Task's worktree root on its recorded
Machine, beside Conversation, Shell, Files, Changes and Task/workflow. Two Tasks in
one Wave must produce their own directories, including after reopening; explicit
host directory choices remain explicit. This covers supported Desktop/Herdr paths
too. Initial-pane cwd alone is insufficient.

`scratch/rex-host-probe.py` anchors native `session.new_split` calls to two different
Task directories with cwd omitted. Both real shells wrote `pwd -P` in the isolated
HOME. Inspected public creation schemas offer per-block cwd but no per-window/Task
split-default setter. Account-wide Lua changes are outside the accepted design.
This disproves the raw API case. It does **not** prove that the app's normal Split
action omits cwd: that path remains unverified. A supported scoped host capability
or an explicit product revision is needed for the failing API case. No alternative
LF-only split command or narrower acceptance has been approved.

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

## Shared lifetime dependency

November 4 status confirms LOO-717 remains open at PR #1811, at demo. Its clean
checkout at `b81df60579` still requires `--native-settled` on every `task presence` write and native closure
before retirement. It also offers exact retaining-process exit observation through
optional owner identity. Those are implementation evidence for the older contract,
not permission to reinstate Rex closure. Client disappearance, lease expiry and
network failure do not establish that Rex's retaining server/terminals ended.

This checkout has no Task presence API. Retention integration needs the shared
owner to support the accepted smaller contract: a reliable terminal-to-Session
association, a hold independent of focus, confirmed final release and unknown
inventory preserving uncertainty. IDs and creation receipts cannot stand in for
live holds, including after restart. A parallel adapter cleanup loop, layout mirror
or shell runtime would conflict with the accepted ownership boundary.

Desktop's accepted two-minute eligible-idle/fifteen-minute completed-and-away
policy is unchanged. Final requested work, drafts and pending requests retain their
existing protections. Eligible cleanup may discard uncommitted files; moved/unmerged
committed refs retain the shared owner's safeguards. The Rex adapter has no separate
authority to settle or delete those resources.

## Remaining work and decisions

### Outstanding acceptance

1. **Retention integration:** shared lifetime owner plus Rex observation must
   enforce the revised hold/release contract, including unfocused terminals,
   unavailable inventory and restart identity. No LF closure of Rex shells.
2. **Native split capability:** raw API cwd inheritance fails; scoped host support
   remains unresolved. App-menu/shortcut, Desktop, Herdr and remote proof are still
   owed. A requirement change needs Jack Heart's decision, not an inference from
   loose layout fidelity.
3. **Bare-open and remote routing:** arbitrary non-default server discovery, actual
   agent-tool invocation after viewer resume/move, explicit overrides and no local
   fallback. Remote arrival routing/reconciliation and known-server launch propagation are
   implemented. Connected source/destination fixtures cover delivery and retries
   without local Work changes; configured two-Machine interaction still needs proof.
4. **Configured reuse and lifecycle proof:** real open → work → return → close/reopen,
   renamed destinations, native arrangement and restart/relaunch without replay,
   unfinished-input/provider-history preservation, app selection and remote usability.
   Herdr needs its bounded configured return/reopen demonstration.
5. **Move/copy:** named observing copies and explicit resume exist; coordinated
   destination preparation/handoff receipts remain substantial implementation work.
   Destination preparation must complete before an explicitly requested input
   handoff; failure must preserve source input and retries must reuse the destination.
   Native reorder/source-state preservation need end-to-end proof. Dropping that
   coordination or named-copy behavior remains an unaccepted optional proposal.
6. **Validation:** gate owns affected suites, migration/DTO coverage and the two
   host-fixture deadline failures below. Authored demo owns configured interaction
   and remaining product judgments; source/fixture passes cannot substitute for it.

## Verification

Gate still owns the preceding compress pass's two 2s host-fixture deadline failures: Launch Services in
`available_conversations_need_no_question_and_are_never_taken_over` and the cmux
resume test. Its earlier isolated native Rex/Herdr passes are supporting evidence,
not validation of the new remote path. Affected gate suites include work-open,
the open/workspace library tests and CLI parsing; configured interaction stays
with the authored demo.

Code review found that the shared Working selection hides Tasks whose checkout
exists only remotely. Workspace inventory now uses the existing scope filters;
only the execution Machine decides whether to materialize a started Task. Routed
followers cannot forward again. Native creation receipts remain the retry fence,
and no retention, input-taking or Rex-closure authority was added.

Native cwd proof uses launcher-written evidence before exec: Rex telemetry can
still report a root-owned login wrapper while Task-view output is already visible.
Neither a displayed checkout nor stable pane IDs prove process cwd or continuity.
Earlier complete plans/results remain at `cc607dc3e1:scratch/rex-workspace.md` and
`df3b5e810d:scratch/rex-workspace.md`; historical demo details stay in their own file.

Check (compress, November 4): `cargo test -p loopflow --lib lf::commands::open::workspace::` — 3 passed; `cargo test -p loopflow --test integration work_open_tests::rex_ -- --test-threads=1` — 11 passed, 1 native-host test ignored; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `git diff --check` pass. Gate owns broader suites and earlier host deadlines; demo owns configured interaction. Authored context remains within both limits.
