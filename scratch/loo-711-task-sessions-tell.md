# LOO-711 · Task Sessions tell you what their background work is doing

Status: accepted product direction; quiet native reporting is committed at
`b7815609fb`. All four configured native demos are recorded; Jack Heart authorized
shipping after the remaining limits were disclosed. Headless demo decision pass 1
advances to the remaining gate and delivery on November 3. See
[the current demo decision](loo-711-native-update-demo.md#headless-decision-pass-1--2026-11-03).
Earlier acceptance and authorization boundaries below are historical; the
November 3 shipping approval supersedes them.
Requested by Jack Heart.

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

Checkpoint `dd2bb7bc21` introduced native delivery, receipt admission and the run
page. The supplied iteration feedback identifies a defect in that checkpoint;
`db5cdbbd14` contains the changed-run repair and focused regressions. Source
inspection during realign confirms that repair, including observation retention
at all four provider dispatch sites. It does not establish the authored demo's
idle wake, draft or usability outcome. The prior Swift checks remain applicable
to the unchanged run page. The recorded upstream tip remains `4e9ade2d04`, already
integrated here; its installed-app routing still constrains demo evidence.

`harness/task_updates.rs` reads existing Task/Graph/Process facts. Only interactive,
unfinished Sessions receive reports; worker Runs are excluded. The first reading
establishes a baseline. Compact observations retain Task position and each run's
identity, outcome and latest step; receipts do not copy compiled graphs or full
history. Runs settle independently for one second. A run between step exit and
next start keeps its previous observation, without suppressing another run's
settled change. Posts name only changed runs, with exact step Process IDs and loop
positions. Codex/OpenCode keep the latest reading while their native stream is busy; Claude/Pi use their provider queues (see the November 3 correction below). The
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
sending. Native fences still validate authority. A latest-observation query excludes late
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
  observer without transferring its messages. The configured Claude inbox was verified in the November 3 native demo; absence
  produces no terminal-input substitute or invented delivery receipt.
- **OpenCode:** the existing native server owner observes on its event loop and
  posts through the existing authenticated synthetic-input endpoint and request/stream
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

- Removed `TaskUpdates::pending`'s full-history hash, latest-run report selector,
  and global exit/start-gap return. Compact observations in existing receipts
  compare and settle by durable identity. Native fencing, atomic admission, busy
  coalescing and uncertain-dispatch suppression remain on the same delivery path.
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
- Removed the duplicate `task_updates::tests::running_graph` fixture; busy
  coalescing uses `SqliteStore::test_graph` and exact repeated-step identities.
  `RunObservation::report` keeps wording outside change detection; baselines
  skip report formatting and silent readings skip receipt serialization/hashing.
- `task_wait` remains unchanged; the superseded wait-only proposal is not revived.

## Earlier acceptance plan

The November 3 demo decision and shipping approval below supersede this plan's
pending demo and publication boundary. Current gate results follow at the end.

- Gate owns affected suites and static checks: `harness::{task_updates,
  claude_updates,codex_connection,opencode}`, `context_block`,
  `lf::commands::context`, Session-event reads, and direct native launch callers.
  Boundary suites include `agent_startup_tests`, `session_cli_tests`,
  `session_steers`, `session_lifecycle_tests`, and
  `explicit_launch_policy_survives_task_workflow_nested_graphs_and_provider_children`.
  Keep the Rust filter explicit; architecture checks, clippy and the broader Swift
  suite also belong to gate. The Pi extension's
  `scripts/benchmarks/skill-invocation/test_pi_context.mjs` and the remote watch /
  exact-process cases in `RegistryQueryTests` need recorded results too; the
  focused commands below do not cover those cases.
  No configured-provider usability claim follows.
- Preserve the authored demo: leave a Task Session idle in Desktop or terminal
  with an unsent draft, run its Graph, and observe a report without typing. Show
  failure/process-history navigation and an interactive stop without advancing it;
  inspect the same run in the page. Verify draft and Session identity survive,
  reconnect does not replay uncertain delivery, and judge reporting cadence.
- Demo must identify a build containing this diff. The branch contains upstream
  `4e9ade2d04` (merge `fb382a7d45`), whose installed-app routing means ordinary
  `lf open` and the prototype HTML prove neither this UI nor native delivery.
- Publication, merge and live-store migration remain outside this request.

Check: realign `git diff --check` and `lf context` pass, under both budgets; reused unchanged-code `cargo test -p loopflow --lib task_update` with inherited `LF_*` cleared and stdin closed (10 passed after compression), `cargo fmt`, and unchanged-page `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'DesktopHeadlessTests/nestedRunHistory|DesktopHeadlessTests/runPositionReadiness|DesktopHeadlessTests/taskWorkFollowsTheStream'` (app built, 3 passed). Existing linker unwind warning remains; broader suites/static checks belong to gate, configured delivery/drafts to demo.

## Concept review · 2026-11-02

**Current outcome: nothing to change; no open product choice.** Re-review of
`db5cdbbd14` confirms the earlier changed-run correction is implemented. The
accepted concepts remain Task position, recorded runs/steps, and the existing
Session; delivery receipts add no workflow or operating authority. The review
page now distinguishes the historical mismatch from the current implementation.

Normal progress, an older worker failing beside a newer helper, and reconnect
after uncertain dispatch all fit that contract. Exact Process identity survives
repeated labels; late acknowledgements cannot replace the dispatched observation.
The existing README and task-session instructions express the accepted experience
and need no further conceptual rewrite. Source and regression inspection support
this finding; no new configured-provider execution or test pass is claimed.

Next proof remains gate and Jack Heart's authored demo: native idle delivery with
an unsent draft, failure inspection, an interactive stop that stays at review,
and reconnect without duplicate delivery. A provider unable to deliver without
altering the draft would invalidate the implementation's acceptance, not authorize
a substitute surface or narrower provider scope. No new implement direction.

### Earlier finding, now repaired

**Prior design change: report the work that changed.** Jack Heart's selected experience
remains Session posts plus screen 2's minimal, expandable run page. The repository
contract includes every associated Graph in a Task's work; the newest Graph has
no special authority. Using it as the sole subject of progress reports leaves
Jack asking what happened to the worker again.

Concrete counterexample in checkpoint `dd2bb7bc21`: `TaskUpdates::pending` hashes
all `graphs` but builds its report only from `graphs.last()`. Start worker A, then
helper B; after the Session has observed both, A fails while B stays unchanged.
The changed key schedules a post about B, omitting A's failure. Its global early
return for the latest Graph's exit/start gap can also withhold another Graph's
settled failure. This is a source-derived counterexample, not a newly executed
provider test. A and B remain equally Task work even when B is taskless execution
in the same checkout.

### Applied correction and evidence limits

Checkpoint `db5cdbbd14` replaces the latest-run selector with compact observations
keyed by durable Task/run identities, retained in existing Session receipts.
Each run settles independently; a gap retains only that run's previous facts.
The report names changed runs and keeps Task position separate. The README and
task-session skill describe this behavior. Latest-first page ordering, Files
ownership and all native delivery/authority boundaries remain unchanged.

Regressions cover an older run failing while newer work is unchanged, between
steps or newly changing. A repeated step label retains its exact Process ID;
reconnect after uncertain dispatch stays silent, then reports the older run's
subsequent failure. The newer run's deferred gap survives those receipts and is
reported when its next step starts. Busy coalescing and position-only review/end
changes stay distinct from historical run results. Atomic observer admission,
late acknowledgement and provider fencing checks remain on the existing path.

No unresolved product choice was found. One-second settling remains the recorded
reversible assumption, subject to Jack Heart's authored demo; configured provider
delivery and draft preservation remain proof obligations. Concept review's
**design changed** finding is historical; its implementation repair is present.
Remaining acceptance is gate and the authored demo listed above, with no
publication or merge authorization. Product memory was read in full; this
checkout has no child Product memory directories to incorporate.

## Native message attribution · 2026-11-03

Jack Heart requested less noisy or hidden automatic updates and prefers them to appear from an agent rather than from the user. The installed Codex protocol exposes `turn/start.toolOutput` with a name and retained output, plus background additional context and raw history injection. The ordinary input path represents user input and is the deletion target for Codex task updates. Probe the named tool-output path with an empty user-input array; preserve the same Session, native owner, receipts and draft. These facts originate in Loopflow's process observations, so name the source `loopflow_task_update`, not an invented worker quotation. Native Codex wake/rendering and draft retention were verified, followed by equivalent quiet delivery through every supported harness; evidence is in `loo-711-native-update-demo.md`.

Jack Heart additionally requires the quiet, non-user-attributed delivery behavior across **all supported harnesses** (Codex, Claude, OpenCode, Pi). Codex-only success does not satisfy this direction. Preserve each native owner; verify idle wake, busy delivery, native rendering, draft retention and retained context individually. Record unsupported or unavailable native capability explicitly rather than claiming parity from adapter tests.

Cross-harness implementation uses Codex retained named tool output, OpenCode synthetic input with the concise notice “Loopflow worker progress”, and Pi hidden custom follow-up messages. Claude uses its existing authenticated native inbox, whose installed receiver marks origin as peer and the input as metadata. Native Claude testing exposed a stale unavailable projection despite its idle terminal: the observer must enqueue through the provider-owned next/follow-up queue rather than wait forever on a missing availability observation. Codex/OpenCode retain their owned stream busy gates. Claude/Pi delivery timing now follows their native queues; per-run settling remains, but multiple settled busy-period observations can be queued and must be checked for noise. No user-origin message fallback is allowed.

Current proof: all four native clients delivered quiet automatic reports; Codex/Pi hide the incoming payload, Claude collapses a named peer message, and OpenCode shows one concise synthetic notice. Each preserved an unsent draft through the checked update. OpenCode’s redundant display-availability gate was also removed in favor of its owned stream. Native burst/coalescing and comprehensive reconnect acceptance remain distinct from this proof. The implementation deletes ordinary user-input transport for these posts; no agent quotation is invented. See `loo-711-native-update-demo.md` and `demo.html`.

## Shipping approval · 2026-11-03

After positive demo feedback and a review of outstanding issues, Jack Heart explicitly requested “lets ship it”. Continue the existing Task’s remaining feature graph on Codex: gate, waited landing, verify and completion. Include the local quiet-message changes in the current PR. No provider restriction. Preserve the native evidence and known limitations in the final PR body before scratch cleanup: all four configured clients demonstrated automatic quiet delivery and draft retention; native busy bursts and comprehensive reconnect/takeover remain unmeasured; Claude/Pi uncertain dispatches are not replayed and can lose an update at disconnect; the initial Codex 267,664 ms model turn remains unexplained. The latest 11 focused Rust tests, Pi extension check, CLI build, formatting and all-target clippy passed. Run the remaining required gate checks and fix actual failures. Do not claim the demo proves the default operator skill or all run-page navigation. Jack accepted shipping after these limitations were disclosed; do not turn them into a new approval request.

## Gate plan · 2026-11-03

Compare with the recorded checkout base `4e9ade2d04`; include the existing
uncommitted demo evidence. Changed Rust paths reach Task observations and receipt
storage, all four native adapters, context hooks and native launch callers.
Run the explicit modules `harness::{task_updates,claude_updates,codex_connection,
opencode,opencode_decode}`, `context_block`, `lf::commands::{context,util}`,
`store::sqlite::session_events`, `builtins::tests`, plus `agent_startup_tests`,
`session_cli_tests`, `session_steers`, `session_lifecycle_tests`, and the nested
Task/Graph provider launch policy regression (now named `explicit_launch_policy_survives_task_runs_nested_graphs_and_provider_children`).
Swift model, transport and view changes reach the headless app build and Swift
suite, including RegistryQuery, WorkObservationTransport and DesktopHeadless.
The changed-aware runner also owns architecture, inventory, rustfmt, all-target
clippy and Swift boundaries. Run Pi and OpenCode extension checks separately.
Use rustup stable (1.99.1), with inherited LF variables cleared and stdin closed.
No Xcode project or FFI Rust source changed; hosted UI and unrelated integration
binaries stay with CI. Existing configured-provider demos remain applicable.

Production performance signal proposed for Product: elapsed time from a recorded
step transition to native report arrival, separated from model response time;
record post count during busy bursts. The native demo's unexplained 267,664 ms
Codex model turn remains a disclosed limitation, not a gate-check timeout.

## Gate results · 2026-11-03

Review found one fixture defect: the new OpenCode receipt test stopped reading
HTTP at a closing-brace byte. It now waits for a complete JSON body. This also
removes an unmatched literal brace that fooled the architecture scanner into
classifying the test's `/bin/sleep` as a production subprocess. No production
behavior changed during gate. The repaired fixture was included in the 144-test
run and passed; subsequent rustfmt changed only whitespace.

Check: `uv run python scripts/test.py --reuse-passing --rust-filter 'test(harness::task_updates::) | test(harness::claude_updates::) | test(harness::codex_connection::) | test(harness::opencode::) | test(harness::opencode_decode::) | test(context_block::) | test(lf::commands::context::) | test(lf::commands::util::) | test(store::sqlite::session_events::) | test(builtins::tests::) | test(agent_startup_tests::) | test(session_cli_tests::) | test(session_steers::) | binary(session_lifecycle_tests) | test(explicit_launch_policy_survives_task_runs_nested_graphs_and_provider_children)'` — 144 Rust tests passed; inventory, CLI builds, all-target clippy (Rust 1.99.1), headless Desktop build, 667 Swift tests (45 optional display diagnostics skipped), and Swift boundary checks passed; initial architecture failure repaired and rechecked separately. Original log: `.lf/tmp/gate/run-80273/`; total 520 s, no timeout.
Check: `uv run python scripts/check_architecture.py` — passed after fixture repair, zero unexplained boundaries.
Check: `node scripts/benchmarks/skill-invocation/test_pi_context.mjs` and `node scripts/benchmarks/skill-invocation/test_opencode_context.mjs` — passed.
Check: `cargo fmt --all -- --check` and `git diff --check` — passed after formatting the fixture repair.

The Swift pass covers nested and repeated run history, independent review
readiness, retained history after read failures, remote Task watch routing,
exact Process history routing, and the real asynchronous FFI write regression.
README behavior matches the implementation. The existing macOS test-linker
unwind-table warning remains; no new compiler/static-analysis warning was found.
Unrelated integration binaries and Xcode/hosted diagnostics remain with their
CI or display owners; none is claimed passed here.

Remaining delivery: the existing Graph owns landing and verification. Preserve
the shipping approval's native burst/reconnect, uncertain-delivery, initial
Codex latency and mounted-navigation limits in PR copy before scratch cleanup.
No new approval or design decision is required.
