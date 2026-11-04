# Open and restore a Loopflow workspace in Rex

**LOO-498 · accepted build scope · reconciled November 3, 2026.**

Jack Heart approved implementation after readiness review in comment
`7088ad9c-c923-4fff-acd7-0a261921b0f0`, superseding the mistaken-launch stop.
The LF Task/workflow terminal view belongs in this Task. Review publication and
demo are authorized; Auto-Ship, merge and Task completion are not.

The implementation at `cc607dc3e1` provides opening, tools and passive arrivals.
Full restoration, lifecycle coordination and the complete configured experience
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
- Reopening preserves an existing occurrence unchanged. New occurrences resolve
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

## LOO-717 lifetime coordination

Jack's November 3 accepted contract treats Desktop and Rex equally: LF honors
host Session retention; each host owns release policy. Desktop must not prefer
completed Sessions. Eligible unviewed idle agents end after two minutes; completed
and away shells/checkout clean at fifteen minutes total. Returning cancels the
countdown; a later departure starts a fresh interval. Requested active turns and
Graphs finish first. Explicit closure can end Sessions after that work. Unfinished
Task membership alone does not retain idle execution, but closing an unfinished
Task view must not delete its checkout or branches.

Jack accepts dirty/untracked files being discarded during eligible completed-Task
cleanup. That does not settle moved/unmerged committed refs. Task/conversation
history survives. Automatic deleted-checkout reconstruction was an agent-introduced
hypothetical and is excluded. Ordinary shell output alone is not retention policy.

LOO-717's coordination comment `67cd130c-9104-48ef-b019-615e3e41e01e`
(November 3 local date) now names a concrete integration boundary:

`lf task presence TASK --occurrence ID --state retained|away|closed|unknown --revision N`

The reported receipt pins Task/completion/checkout/branch and includes due_at,
phase (waiting/closing/retired) and reason. Each occurrence has a stable identity
and increasing persisted revision. Retained observations renew within 30 seconds;
expiry becomes unknown and blocks checkout cleanup, while only a live hold retains
an eligible idle agent. Away starts the workspace deadline; last explicit closed
permits retirement after work settles. Closing requests native closure of only
that Task occurrence followed by closed acknowledgement. Another retained copy
cancels queued cleanup. This is implementation coordination, not an added Jack
product decision or proof of delivery.

The latest shared status read shows LOO-717 in sync, loop 2 pass 2, with no PR.
Inspection of its checkout found the coordination specification but no implemented
Task presence command.
The API is not integrated in this checkout. Here `observation="retained"` records
successful materialization; it is not a renewed lifetime hold. LOO-498 still owes
occurrence reporting and scoped native cleanup to LOO-717's owner. No second
timer, shell registry or checkout deletion owner belongs in this Task. Unknown
host observations cannot authorize retirement; a Wave container with other
Tasks must never be closed as a Task cleanup shortcut.

## Delete — do not maintain

Removed: the Herdr keystroke-resume adapter and its exclusive test; native-window
and container early returns that bypass activation or follower recovery;
`run_host`'s wait-before-draining pipe capture; and the workspace reader's
ID-list/per-ID query round trip. No remaining predecessor path needs maintaining.
Preserve native blocks, uncertain-outcome receipts and explicit-close behavior.
No replacement ordinary-shell relay or cleanup authority is introduced.

## Recovery implementation, November 3

Explicit opening activates a passive arrival once by adding the selected tools
beside its existing native panes. The pending activation receipt prevents command
replay after a lost split reply. Subsequent opens preserve the resulting layout.
A follower holds an OS lock for its lifetime; reopening can add a fresh follower
tab after that process exits, without replacing the old native tab. Launch-pending
receipts prevent blind retries. Repository-created Wave containers now also follow
independently. These locks describe follower liveness, never Task retention. Host command output
is spooled to temporary files while running, so native inventories larger than a
pipe buffer cannot deadlock the timed wait. The initial isolated bare-open timeout
exposed that wait-before-drain failure; the host-inventory regression test passed.

Container reuse and creation converge on one follower check. Activation uses the
container already resolved under the occurrence lock. Workspace lookup reads
complete rows in one SQL statement; no cross-statement snapshot or second lookup
is needed. Layout traversal handles hidden panes and collapsed splits separately
from tool selection, with one split-axis translation for the Rex renderer.

## Remaining work and decisions

1. **Safe restoration.** Retained tool windows have no command-continuity signal.
   Stable native block IDs cannot distinguish restored login shells from
   fresh native work. A supported signal identifying untouched restored blocks is
   still needed for automatic recovery. If unavailable, demo/review must choose
   an explicit replacement/new-occurrence interaction or a bounded restoration
   contract; neither choice is accepted yet. Arbitrary source commands cannot be
   replayed and native edits cannot be overwritten. Full restoration remains an
   acceptance requirement until Jack accepts a change.
2. **Recovery evidence.** Activation and follower recovery now have implementation;
   the isolated native check passes for one-time activation, preserved block identity,
   repeated no-focus open and follower recovery. A lost follower-launch reply before
   startup remains explicitly uncertain, rather than authorizing duplicate launch.
3. **Lifecycle integration.** Consume LOO-717's actual delivered presence contract,
   persist observation revisions, aggregate copies and implement native scoped
   closure. Reliable Rex use/away evidence remains unresolved; attachment/window
   inventory alone cannot supply it. A host release policy without such evidence
   needs explicit review rather than a guessed timer. Disconnect and unknown
   inventory must preserve safety without claiming indefinite retention is accepted.
4. **Bare-open and remote coverage.** Resolve non-default Rex caller endpoints
   without required flags. Demonstrate actual LLM tool invocation after viewer
   resume/move, explicit overrides and no local fallback. Existing tests cover
   source placement routing and direct native shell invocation, not that full
   agent path. The local follower skips remote Tasks; newly relevant Machines
   need following/reconciliation, not merely a new explicit open. Real peer/app
   visibility and provider usability remain unproved.
5. **Move/copy and secondary lifecycle.** Named observing copies and explicit
   `lf resume` exist. Coordinated destination preparation/handoff receipts and
   scoped lifecycle cleanup are unfinished. Native reorder and source-state
   preservation need end-to-end proof; cross-host ordinary-process migration is
   excluded. Herdr's bounded scope needs the complete configured return/reopen and
   cleanup demonstration, without claiming unsupported Wave/remote parity.
6. **Gate and demo evidence.** The complete select/open → work → return →
   close/reopen path must preserve unfinished input and provider history, with
   no duplicate provider or replay. Concurrent/partial/uncertain outcomes must
   preserve unrelated panes. Gate owns affected suites and migration/DTO coverage;
   demo owns configured interaction and the exact unresolved product choices above.

## Verification

Earlier focused passes recorded in `cc607dc3e1` remain evidence for unchanged
code: 15 work-open tests, 2 workspace unit tests, observer/editor/Task-view/template/
committed-diff checks, 22 Swift WorkSelection/RegistryQuery tests, format/clippy and
both opt-in native host tests. Rex passed outside the network sandbox; inside it,
its caller produced no receipt or exit/error file before the deadline. Herdr passed
inside. That sandbox failure remains unresolved and the real-agent/remote path
has not been demonstrated. No live resource cleanup occurred during reconciliation.

Check: `cargo test -p loopflow --lib wave_task_defaults_only_materialize_unsaved_task_layouts` passed (1); `cargo test -p loopflow --test integration work_open_tests:: -- --include-ignored` passed (17, including isolated Rex/Herdr, with inherited LF variables unset and installed host paths supplied); `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and `git diff --check` passed. The library test linker warned about the existing large unwind section. Gate owns broader affected suites; demo owns configured interaction and the unresolved restoration/lifecycle choices.
