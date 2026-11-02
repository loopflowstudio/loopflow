# LOO-711 · Task Sessions tell you what their background work is doing

Status: accepted product direction; production implementation and focused checks complete;
authored demo and gate remain. Reconciled 2026-11-02. Requested by Jack Heart.

## Accepted outcome

Jack selected worker posts into the existing Task Session **plus** a separate,
minimal run page with expandable detail. His closest prototype is screen 2 in
`scratch/prototype-screens.html`: “2 is probably the closest.” Combine position
and readiness, avoid repeated facts, and let Files own the diff. Missing prose
summaries, costs and file counts stay absent; the HTML uses simulated data.

Jack's November 2 steer authorized implementation under the existing feature
Graph, retaining demo review and excluding publication or merge. His request to
restart this Task worker on Codex did not restrict the product's providers.
Temporal was a presentation reference, not a request for durable replay.

Source: the continued Claude conversation at
`/Users/jack/.lf/runs/f1/f15d2a56a6b84df2b6427996a923fad0/conversation.md`.
Original exploration and superseded wait-only design:
`5f658180d9:scratch/loo-711-task-sessions-tell.md`. The prototype HTML remains here.

## Delivery design and implementation · 2026-11-02

`harness/task_updates.rs` reads existing Task/Graph/Process facts. Only interactive,
unfinished Sessions receive reports; worker Runs are excluded. The first reading
establishes a baseline. Later changes settle for one second; the separate exit
and next-start commits coalesce. Busy Sessions keep the latest reading. The
cadence is a reversible implementation assumption for Jack's demo review, not
an attributed product choice.

The native owner delivers into its selected provider conversation. There is no
terminal input injection, new conversation, worker launch, Graph cursor, authored
steer or notification table. A progress post asks for a brief report and explicitly
grants no review, launch, publication or merge authority. The task-session skill
recognizes this observation separately from user direction.

Existing Session history holds baseline/dispatched/accepted/rejected receipts
keyed by the observed facts. One atomic insert claims each reading before provider
bytes; the existing unique receipt index prevents overlapping relays from both
sending. Native fences still validate authority. A latest-key query excludes late
acknowledgements without loading the receipt history. Reconnect never replays an
uncertain dispatch. A changed reading can produce a new report. No schema migration.

- **Codex:** the existing `CodexConnection` relay sends `turn/start` on the native
  client's subscribed connection and selected thread, with native approval routing
  intact. It waits for turn completion, retains replies, and fences overlapping
  reconnects/takeover with the existing attachment. Disconnected clients receive
  the latest changed facts when their native relay reconnects.
- **Claude:** asynchronous SessionStart/Stop hooks observe through the launch's
  provider capability. The hook is a provider descendant and uses the inbox and
  ephemeral child token Claude supplies in its environment. Native inbox messages
  name the selected thread and use next priority. Tokens never leave process memory.
  The socket supplies no consumption acknowledgement: a write remains uncertain.
  Observer locks include native thread identity so clear/resume can replace an old
  observer without transferring its messages. Availability of this provider inbox
  in the configured Claude build remains a demo prerequisite to verify; absence
  produces no terminal-input substitute or invented delivery receipt.
- **OpenCode:** the existing native server owner observes on its event loop and
  posts through the existing authenticated prompt endpoint and request/stream
  owner. Native turn events hold delivery while busy; HTTP success records native
  acceptance, not a claim that a person read the report.
- **Pi:** the existing launch-local extension owns an observer child and receives
  bounded newline frames. Its native `sendMessage` uses `triggerTurn` and follow-up
  delivery, preserving the editor. Session changes reject delayed foreign frames;
  shutdown ends the observer. Pipe delivery has no consumption acknowledgement,
  so it also retains uncertainty instead of replaying.

Native identity and provider authority remain with their existing owners. Receipt
history is delivery evidence, never permission to operate the Task. The configured
provider/draft experience is not established by the simulated transports below.

## Run page

`GraphProcessLog` replaces the old default diagram/flat list. Its compact header
combines current Task position and readiness, then shows the latest run and Files.
Earlier runs, contiguous whole nested-loop positions, and individual steps expand.
All historical steps retain IDs, timestamps and actual exits; repeated nodes and
returned loop positions are distinct. Process history reads the exact step ID on
its content Machine. No prose summary is synthesized from success/failure.

The Task's workflow position owns review readiness. A completed historical Graph
never means the Task completed or authorizes advancing review. Definition diagrams
stay in the graph-definition surface. Local pages reuse WorkObservation; remote
pages use the same `desktop task --watch` reading, with recorded-owner validation
on every changed frame. Failed reads retain visible history and name the error.
Files and Session panes retain their existing owners and drafts.

### Delete — do not maintain

- Removed `GraphProcessView.detail`'s default graph diagram and flat step list;
  grouped history preserves every recorded step and exit. `GraphDiagram` remains
  for definition inspection.
- Removed `RegistryQuery.taskGraphProcesses`'s inventory-plus-detail fanout and
  exclusive fixture. One store-driven Task reading supplies readiness and history.
- Replaced the task-session skill's “nothing schedules your next one” contract
  with native progress observations and their explicit authority limits.
- Removed the four provider-local receipt scans and `task_updates::{receipt,
  retain}` wrappers. `SqliteStore::{record_task_update,latest_task_update}` owns
  receipt admission and reading; provider tests retain transport coverage.
- Removed stored `GraphStepGroup.iterations`; the first step supplies the group's
  position. The run page selects one content source before rendering, including
  read failures alongside retained history.
- `task_wait` remains unchanged; the superseded wait-only proposal is not revived.

## Remaining acceptance

- Gate owns affected suites and static checks: `harness::{task_updates,
  claude_updates,codex_connection,opencode}`, `context_block`,
  `lf::commands::context`, Session-event reads, and direct native launch callers.
  Boundary suites include `agent_startup_tests`, `session_cli_tests`,
  `session_steers`, `session_lifecycle_tests`, and
  `explicit_launch_policy_survives_task_workflow_nested_graphs_and_provider_children`.
  Keep the Rust filter explicit; architecture checks, clippy and the broader Swift
  suite also belong to gate. No configured-provider usability claim follows.
- Preserve the authored demo: leave a Task Session idle in Desktop or terminal
  with an unsent draft, run its Graph, and observe a report without typing. Show
  failure/process-history navigation and an interactive stop without advancing it;
  inspect the same run in the page. Verify draft and Session identity survive,
  reconnect does not replay uncertain delivery, and judge reporting cadence.
- Demo must identify a build containing this diff. The branch contains upstream
  `4e9ade2d04` (merge `fb382a7d45`), whose installed-app routing means ordinary
  `lf open` and the prototype HTML prove neither this UI nor native delivery.
- Publication, merge and live-store migration remain outside this request.

Check: `cargo test -p loopflow --lib task_update` (7 passed), `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'DesktopHeadlessTests/nestedRunHistory|DesktopHeadlessTests/runPositionReadiness|DesktopHeadlessTests/taskWorkFollowsTheStream'` (app built, 3 passed), `cargo fmt`, `git diff --check`, `lf context` (pass, under both budgets); existing linker/macOS-target and Swift nil-coalescing warnings remain, broader suites/static checks belong to gate, configured native delivery/drafts to demo.
