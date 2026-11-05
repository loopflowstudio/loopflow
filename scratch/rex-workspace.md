# Open and restore a Loopflow workspace in Rex

**LOO-498 · concept review revised November 5, 2026. Implemented in the fresh-launch pass below; earlier plan and evidence: `9187804aee:scratch/rex-workspace.md`.**

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
There is no additional loop, Graph-tab or conversation-tab concept. This supersedes
the earlier reopening/named-copy-reuse and durable creation-recovery contracts.

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
ownership; each open launches fresh. The `task_layout` setting uses the existing
config resolver and shared pane intent; the winning scope replaces the whole
layout. No other configuration file or host-specific layout language.

### Actions → concepts → APIs

| Action | Concept / owner | Operation |
| --- | --- | --- |
| Set new Task defaults | Authored template / config | Existing config read/write path, `task_layout` setting |
| Arrange this Task in Desktop | Current Task layout / LF DB | Existing Task `desktop layout` read/save path |
| Open it in Rex | One launch / native host | `lf open`; read source, compile in memory, native create, report result |
| Return, resize, rename or restore | Native workspace / Rex | Rex's own UI/APIs; no LF writeback |
| Open again | Another explicit launch | Same `lf open`; creates another Task tab and split set, potentially in the same native session |

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
hosts; the earlier cmux-workspace exclusion is superseded for this direction.
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

### Fresh-launch pass (November 5) — implemented

`lf open` with Rex, Herdr or cmux as the selected host goes to
`open/workspace.rs`. It resolves the host from explicit `--host`, else the agent's
viewer placement, else the caller's terminal (Rex, Herdr, tmux, cmux in that
order). Any detected host is the only route: no Desktop/terminal fallback.
`Native::{Rex,Herdr,Cmux}` hold this command's handles only:

| Step | Rex | Herdr | cmux |
| --- | --- | --- | --- |
| Task inside a container | caller's session (`REX_SESSION`/viewer) | `pane.current` workspace | caller's window |
| Otherwise / repo / Wave | `session.create` | `workspace.create` (its tab is reused for the first Task) | `workspace-group create` |
| Task tab | `session.new_window` + split tree | `tab.create` + `layout.apply` (reply names the replacement tab) | `new-workspace --layout --group` |
| Follow | Workspace tab running `open <url> --follow <session> --seen …` | — | — |

Layout source: saved Desktop `work_pane_layouts`, else `task_layout` from
`load_wave_config(repo, wave slug)`, else the builtin. `task_layout` is a General,
Replace setting holding `TaskLayoutTemplate {tree, hidden}`. Readiness keys are a
fresh UUID per tab, cleared after `settle`; receipts offer `lf open … --session`
to view again. Remote Machines get `open --host <h> --local-workspace` over the
existing SSH request; Herdr/cmux run locally only.

**Deleted:** `store/sqlite/host_workspaces.rs` (`HostWorkspace`, `HostOccurrence`,
`HostCreation`), the unmerged `rex_workspaces.sql` (three tables, Wave-template
triggers), `wave_task_layout`/`save_wave_task_layout` and the template fallback in
`layout_in`, `desktop layout wave-default:`, `workspace/hold.rs` with `ViewHold`/
`rex_hold`, `find_native` marker recovery, `--copy`, `--new-shells`,
`--watch-workspace`, `--arrive-task`, `--retry-observers`, the follower lock/pending
file, conversation-only `open_cmux`, and registry-reuse tests (remote arrival/
follower, marker recovery, passive activation, restart reuse). Store code under
`store/sqlite/` now equals origin/main. No live store was migrated; the private demo
snapshot keeps whatever draft tables it has.

**Counterexamples covered:** a renamed native tab is untouched by a later open
(native Rex/Herdr); a closed arrival stays closed while the follower runs (native
Rex); arrivals do not change the active window (native Rex); a Task opened from a
Rex shell joins that session; source drafts never reach argv (layout unit test).

### Remaining (reconciled November 5, realign)

Jack Heart's two unresolved requirements stay requirements; neither is waived here.

1. **Retained Rex Task Session terminals must protect checkout cleanup.** Nothing
   enforces it. The consumer is LOO-717 (PR #1811 at `b81df60579`, conflicting with
   main): `lf task presence <issue> --occurrence <id> --state retained|away|closed|unknown
   --revision <n> --native-settled <bool> [--owner '{"machine_id","pid","started_at",
   "settles_on_exit"}']`, a 30 s lease (`HOST_LEASE`), a 15 min grace, stored in
   `task_host_occurrences` (`task_workspace_lifetimes.sql`); lease expiry is unknown,
   never release. **Conflict:** that table persists host occurrences in LF, which
   Jack's November 5 decision rejects anywhere. Open decision for Jack: is 717's
   lease the one exception (an observation, not a workspace recipe), or does 717's
   API need a stateless shape?
   Native inventory without an LF registry (isolated Rex probe, November 5):
   `session.list_blocks` returns block_id, label, flavor, window_id, owner and
   host_id; block methods `pwd` (last OSC 7 report, owner pid) and `process` (child
   pid/path/home). No argv, cwd option or env is readable. So:
   - Live LF tools in a block (`session view`, `task view`, `task tool`) see their
     own `REX_BLOCK`/`REX_SESSION`; each can lease occurrence
     `rex:<endpoint>#<block>` as retained with `--owner` its own pid, renewing
     under 30 s, and report `closed` on SIGHUP. Concrete LF work once 717 lands.
   - An exited program in a retained block (observer `exit.on_completion=false`)
     renews nothing, so the lease goes unknown and blocks cleanup until a
     cleanup-time host query: `session.list_blocks` by block id says present
     (still retained, regardless of focus) or absent (closed). Keyed by id, so
     renamed tabs keep their association. Needs 717's executor to call the host,
     which it does not today (it observes `--owner` exit through `lf machine process`).
   - Matching window labels to Task titles is rename-fragile; not the primary path.
2. **Automatic arrivals on a Machine with no initial workspace wait for another
   open.** `open_machines` launches remotely only on peers holding a started,
   unfinished Task now, and the follower skips remote routes. At `9187804aee` the
   follower forwarded `--arrive-task` through `remote_open` into the registry's
   remote container; the registry left, so that path left. Options for Jack:
   (a) a repository/Wave launch opens on every routed Machine (`store.machines()`),
   each with its own session and follower, at the cost of an SSH request and a
   failure receipt per unreachable Machine; (b) the follower forwards
   `open --task <id> --host rex --local-workspace --no-focus` on arrival, creating a
   fresh session there each time (fresh-launch semantics, no container reuse).
3. **cmux container.** Rex and Herdr Tasks join the caller's container; cmux Tasks
   join the caller's window (`Container { id: None }`), outside any group the
   caller's workspace belongs to. `cmux identify` and `workspace-group list --json`
   can resolve the caller's group from `CMUX_WORKSPACE_ID`; `new-workspace --group
   <id> --group-placement afterCurrent --group-reference <ws>` places it. Reversible,
   unverified against a live app; it would match the Rex/Herdr hierarchy.
4. **Template authoring validation.** `task_layout` parses as `TaskLayoutTemplate`
   at config read; `instantiate` rejects captured Session/location/shell execution/
   files and runs `WorkPaneLayout::validate` only when a launch reads it. `lf set
   task_layout` accepts an invalid tree, and the first `lf open` fails with
   "task_layout setting". Validating in the setter with a placeholder key is small
   and reversible; not done.
5. Demo: app Split from each pane (Rex, Herdr, Desktop), cmux end to end, agent-tool
   routing after viewer move, two-Machine open, real-provider input transfer through
   "view again". `workspace-demo.md` holds the dated failed demos.

### Carried evidence

- Rex `0.0.1793377904-ge6592f`: invalid native cwd falls back home, so LF checks the
  checkout first. Native children carry REX_SESSION/REX_BLOCK but not REX_SERVER;
  LF reads the ancestor server's `--listen`, and tool argv carries `REX_SERVER`.
  Restart kept native IDs but replaced programs with fresh shells: IDs never prove
  continuity or authorize replay. `client.perform(session.select)` is display only.
- Split directories: tool panes print OSC 7 for their checkout; shells report
  nothing so `cd` governs; a leaving viewer clears its report; bound-later
  conversations report the selected directory. Rex's app action is the only Split
  that inherits, and which source it prefers is unobserved.
- LOO-717 (PR #1811, `b81df60579`) keeps occurrence leases and `--native-settled`;
  Jack's revised retained-regardless-of-focus contract has not reached its API.
- Herdr 0.9.3/protocol 22 and cmux 0.65.0 (CLI at
  `/Applications/cmux.app/Contents/Resources/bin/cmux`) capabilities as above.

## Verification

Compress (November 5): `cargo clippy -p loopflow --all-targets -- -D warnings` clean
after merging template validation into `instantiate`, sharing Task-tab creation
between launch and follower, and deleting unreachable Herdr/cmux focus arms; lib
`open::` 16/16 and `configured_task_layout…` passed; gate owns the rest.

Realign (November 5): no code change; compress results stand. Isolated Rex
inventory probe `/tmp/loo498-rex-inventory-probe.py` (throwaway server): block
list has no argv/env; `pwd`/`process` are per-block methods.

Check (November 5 fresh-launch pass): lib `open::` 16/16, `config::` 76/76,
`work_open_tests::` 21/21 (+2 ignored native), native `rex_native_fresh…` and
`herdr_native…` and `transport_tests rex_unconfirmed_observer…` passed on isolated
servers, `documented_commands::` 4/4, fmt + clippy clean. Gate owns the affected
suites; demo owns cmux and app Split.
