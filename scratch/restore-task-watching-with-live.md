# Task watcher — LOO-293

Product / Desktop. One PR, including passive native Sessions. The accepted Task
description is the durable seed after scratch is cleared. This document resolves
implementation details without narrowing its end state.

## Experience and acceptance

Add Watch beside Changes and Terminal in TaskWorkspaceView, reachable from Work,
Roadmap, and Wave detail. Watch needs neither a worker nor a surviving worktree;
Changes and Terminal keep their prerequisites.

Show the selected invocation's actual expanded plan as connected, selectable
stages: skills, human checkpoints, operations, ancestry, status, and attempt
counts. An Iterate edge returns to the recorded target. Include text/icons,
keyboard selection, and reduced-motion support. Never reconstruct old plans
from current YAML or join stages by skill name.

All output combines every Task-attributed Run, including concurrent auxiliary
Runs, labeled by stage, Run, and provider. Preserve source order within each Run;
cross-Run order is observation order, not causality. Show prose, commentary,
commands, tool results, and failures; expand long output. Stage and Run selection
filter the feed. Scrolling up suspends following; Follow live clears filters and
follows new Runs and stages. Earlier invocations and older pages remain available.

Watch never launches, resumes, replaces, or interrupts a provider client. A human
checkpoint links to its existing Session and decision controls. Swift owns only
selection, folding, and scroll state; shared Rust readers own the projection.

## Authorities and persistence

- FlowPosition remains execution authority. Its immutable QueuedInvocation
  already includes the expanded plan and ancestry.
- Existing task_events retain committed inspection facts in the same SQLite
  transaction as position mutations: plan selected, stage entered, Run bound,
  ready/blocked, transition, and invocation settled/replaced. These facts never
  create Project/Wave observations. No new table or migration is needed.
- Stage identity is `(invocation_id, step_index)`; attempts include iteration
  and Run identity. Worker bindings come from the claim transaction; human
  bindings come from the Session position transaction. Auxiliary Runs have no
  stage binding and cannot advance the diagram.
- Run journals own autonomous output. Native provider history owns interactive
  conversation output; the watcher reads it passively through Rust. Neither a
  Swift cache nor the Watch query becomes a transcript writer or execution owner.
- Flow completion and Task completion are independent. Old Runs without stage
  receipts remain unassigned history. Missing receipts are never synthesized.

## Native passive read contract

The initial draft's `(run_id, event_seq)` only describes Run journals. Native
transcripts have their own identities and mutable parts, so the output DTO must
also carry a source discriminator, source item identity, and revision. This is
a correction to that proposed wire shape, not a second transcript authority.

Resolve the recorded ProviderSessionRef, harness, and exact provider account.
Read the account's recorded native home via the store; do not invoke account
selection (it can lease, refresh credentials, or change route). Never fall back
from a recorded managed account to the operator's default account. Older Runs
without sufficient source identity report an explicit source gap.

Provider-specific sources (metadata inspected locally 2026-09-23):

| Provider | Passive source | Normalization / continuation |
| --- | --- | --- |
| Claude | Account home `projects/<encoded-cwd>/<session-id>.jsonl` | Validate session identity, read complete JSONL records by byte offset; assistant/user message content includes prose and tool blocks. Preserve UUID/content-block identity. Ignore history snapshots and title/queue metadata. |
| Codex | Account home `sessions/YYYY/MM/DD/rollout-…-<session-id>.jsonl`, also archived sessions | Match session metadata, retain source path in cursor, read complete records by byte offset. Normalize public response messages, tool calls/results, and lifecycle/error events. Do not render response items and their event_msg mirrors twice. Do not expose encrypted reasoning or internal instructions. |
| OpenCode | Recorded native data root's `opencode.db`, opened read-only | Exact session_id joins message/part rows. Parts are mutable: emit stable message/part IDs with content revisions and upsert semantics, never append every sampled full text as a new item. |

JSONL readers distinguish an incomplete trailing line from malformed complete
records, bound bytes and records per page, and report truncation/replacement as
a source reset rather than silently skipping. A native source may persist at
message/tool boundaries, so live means output visible before Session completion,
not an unsupported promise of token-by-token capture. Final-only output fails.

OpenCode continuation carries `(time_updated, part_id)` plus hashes for the
watermark timestamp and unfinished part IDs. Re-read the inclusive timestamp
boundary and unfinished parts; emit only changed revisions. Keep the historical
page cursor separate. A timestamp-only exclusive watermark loses same-millisecond
updates. Test this under concurrent writes and source replacement/compaction
before claiming coverage; report a reset when the source cannot preserve it.

Extend the existing ProviderSessionRef with an optional tagged native source
location (JSONL path or OpenCode database path). The launching process/hook
records the effective location from its exact environment, including nondefault
data roots; Watch only reads it. Existing receipts with no location may resolve
through the exact recorded account and a matching provider Session ID. They must
not guess a nondefault OpenCode root or borrow the current observer's environment.
This keeps source provenance beside Session identity rather than introducing a
second registry. The optional location is now implemented in that receipt.

No native read launches a provider process, attaches a PTY, starts a server,
changes credentials, or writes provider storage. Missing/unreadable/unsupported
source is different from an available quiet live source. Native source evidence
must remain available after reopening; provider-deleted history reports a gap.

Inspected references: `lf/commands/util.rs::session_command_status_with_env`,
`run_record.rs::ProviderSessionRef`, and `provider_account.rs::ensure_account_home_at`.
The managed homes do not share sessions/projects with the operator home.
[Claude persistence documentation](https://code.claude.com/docs/en/agent-sdk/sessions)
describes conversation persistence. [Codex recorder source](https://github.com/openai/codex/blob/f5f08c54cb7a774594d3579c5731ea3e87f01c48/codex-rs/rollout/src/recorder.rs)
flushes JSONL writes; [OpenCode message reader](https://github.com/anomalyco/opencode/blob/82d4c89031ed30da1cf22ba62419fec520df8d27/packages/opencode/src/session/message-v2.ts)
joins message/part rows. Installed record shapes were inspected without printing
conversation content or credentials. These observations establish candidate
sources, not configured live capture proof.

## Shared read API

`lf task watch ISSUE --json` → TaskWatchSnapshot: Task identity, active stage,
invocation plans, stages/attempts/transitions, attributed Runs, and evidence gaps.
This snapshot currently reads all retained flow facts; it has no history cursor.

`lf task output ISSUE --json [--cursor FILE]` (use `-` for stdin) → TaskOutputPage: ordered source
entries, each carrying its Run/provider/stage labels, ordered records, availability,
reset state, and gaps. OutputRecord carries only source item identity, revision,
and the normalized conversation event. Clients retain source order and record
order without joining a separate status list. The page carries Task identity and
opaque continuation. Start live output with `--tail`, then continue without that
flag. History starts without `--tail`; the desktop retains both cursors independently.
Required fields or explicit Option only; shared Rust/Swift fixtures pin the DTOs.

`read_task_watch(store, task_id)` joins the Task ledger and attributed Runs.
`read_task_output(store, task_id, cursor)` discovers new Runs and returns bounded
incremental pages from their exact sources. Discovery must not inherit `lf runs`'
seven-day/fifty-Run limit. Opaque cursors bind Task/source identity and retain
per-Run offsets/revisions plus discovery progress. Reconnect deduplicates stable
identities. Never reread the entire transcript on every poll. Historical paging
and live continuation are separate cursors so loading history cannot lose the
live tail. Native and normalized autonomous copies must not both appear.

RegistryQuery.taskWatch/taskOutput expose typed CLI reads. TaskWatchStore.refresh
retains the snapshot and inspection selection; readOutput merges records using
independent history/live cursors. Both preserve last-good evidence and expose
failed reads. Updates are currently manual. The complete target polls once per
second while visible after discovery and retained state are bounded; hidden
views cancel subprocess reads. Reuse the existing Session navigation for
checkpoints and LOO-291's shared Task identities where available. No dependency
on its landing.

## This slice

Human direction, 2026-09-24: the deliverable is the desktop Task view. Keep the
existing snapshot and output reads as its plumbing; avoid additional commands,
DTOs or service APIs unless a concrete desktop interaction requires them. After
reviewing this continuation change, implement the output feed in the existing
Watch view and resolve reader limits as that integration requires. A successful
CLI read alone is not the product finish line.

Connect the existing output read to the retained desktop Watch store and view.
Seed an independent live cursor before reading the first historical page; Refresh
advances live output, and Load history advances history without moving the live
cursor. Keep the UI explicit that updates are manual while discovery and cursor
state remain inventory-dependent. Automatic polling is still a required later
slice in this PR, not silently enabled by the Follow live control.

Retain output only as window-local presentation. Merge by Run, source and source
record identity; preserve each reader's order, deduplicate overlaps, and prefer
live revisions over overlapping historical revisions. Fold normalized item
snapshots and deltas for display. Show source availability, paging and gaps beside
the attributed output. A reset freezes the last display with a reload notice;
explicit Reload output discards both continuations before reading fresh evidence,
so independent cursors cannot combine different source generations.

Stage selection filters all attempts at the exact invocation/step; Run selection
narrows further. Follow live clears both filters and rejoins the active invocation
and the displayed tail. Scrolling suspends following. Keep the plan and output in
the same workspace, retaining inspection through Task/repository navigation and
cancelling reads when hidden. No new command, DTO, provider control or persistence.

Done when focused Swift tests prove independent live/history progress, overlap
and revision merging, exact filters across transitions and auxiliary Runs, failed
and superseded reads, reset/reload, and visible output alongside the plan. Render
and inspect the integrated view. This is local desktop proof; automatic polling,
bounded discovery/state, complete capture, connected diagram, exact human Session
links and the full configured human demonstration remain required in this PR.

## Remaining slices and full Done When

1. Connect the existing output read to the desktop Watch inspector: feed,
   history/live merging, filters and Follow live. Keep plan and output in the
   existing Task workspace; prove the visible behavior with Swift tests.
2. Resolve bounded discovery/initialization/state for visible polling, complete
   capture and provider-specific configured proof, and exact human Session links.
3. Finish the connected diagram and configured desktop demonstration, including
   transitions, auxiliary Runs, Iterate and completed history; enter the human gate.

The configured demo must show autonomous text/tool output before completion, a
stage transition without reopening, inspection of the earlier stage and Follow
live, a concurrent auxiliary Run, human Session Iterate with a return edge and
separate attempts, and retained diagram/output after completion and reopening.
Include passive native Session output while the original client remains active.
A static diagram, final answers only, autonomous-only coverage, or terminal
attachment does not satisfy the Task. Human demo and final settlement remain
owned by the pinned lifecycle. No multi-Task dashboard, graph editing, or remote
transport expansion is in scope.

## Evidence ledger

- 2026-09-24 desktop feed review fixed native tool-result folding: Codex results
  previously erased command/input, while Claude results in another message left
  calls running. Display folding now correlates the shared native call ID within
  its Run/source, retaining name/input and applying result/status. Both provider
  regression cases failed before and pass after. Fifteen focused tests pass;
  two additional CLI-to-desktop probes prove disposable native arrivals and
  configured historical reading. See `review-watch-feed.md` for scope and receipts.
  These are not an installed-app interaction or the complete configured demo.
- 2026-09-24 desktop output slice: Watch now consumes `taskOutput` beneath
  the retained plan. The existing TaskWatchStore owns independent history/live
  continuations, exact stage/Run filtering, source gaps and last-good evidence.
  Source records merge by Run/source/item identity; live revisions survive
  overlapping history. Tool snapshots replace accumulated output, and adjacent
  text folds without moving prose across a command. A source reset requires an
  explicit reload of both readers. No CLI, DTO, provider or persistence changes.
- Follow live clears filters and tracks active stages on snapshot refresh;
  scrolling pauses following. With this slice's Run grouping, following anchors
  to the most recently changed Run rather than a quiet Run at the overall bottom.
  Automatic polling remains off and is labeled in the view. Cross-Run contiguous
  observation ordering remains part of the complete feed, alongside the existing
  bounded discovery/state and configured capture obligations.
- Focused final proof: 14 tests in TaskWatchTests, TaskWatchFeedTests and
  TaskWatchOutputTests pass with `-Xswiftc -gnone --jobs 4`; receipt
  `/tmp/loo293-feed-verified.log`. Includes independent cursors/revisions,
  filtering/transitions, source reset/reload, gaps, cancellation/supersession,
  source order, tool folding and the Follow action/target. Both standalone and
  integrated plan/output PNGs rendered and were inspected. This is local model,
  query-boundary and rendered-fixture evidence, not configured arrivals or native
  scrolling/human interaction proof. Full Task acceptance remains open.

- 2026-09-24 independent continuation: `task output --tail` now seeds live
  positions separately from ordinary historical pagination. Both CLI dispatch
  paths and RegistryQuery use the existing read operation and cursor transport;
  the shared output DTO and fixtures are unchanged. Newly discovered Runs read
  from the beginning. Initialization seeds every discovered Run before paginating,
  so output written before an unvisited Run's first round-robin turn survives.
- JSONL seeds stop at the last complete line, verify native Session identity and
  retain capture mode. OpenCode seeds its timestamp boundary and exact unfinished
  IDs in one read-only transaction; an older part completing at the same timestamp
  before its first page remains readable. Existing reset and revision handling
  remain in the same source reader. No provider, flow, Session or persistence
  authority was added.
- Focused proof: three reader tests pass in `/tmp/loo293-tail-readers-final.log`;
  the 51-existing-Run/new-auxiliary continuation test passes in
  `/tmp/loo293-tail-discovery-final.log`. These cover unread history alongside new
  output, incomplete lines, native identity/reset, journal mirror suppression
  across retry, and OpenCode unfinished/boundary revisions. Two Swift query/DTO
  tests pass in `/tmp/loo293-tail-swift.log`; the Mac product links. `cargo fmt`,
  `cargo clippy --all-targets -- -D warnings`, the CLI build and whitespace checks
  pass. No affected-suite or full repository gate ran.
- Review/configured correction: the first real read hit `tail_unavailable` on a
  15,481,798-byte normalized journal because its attempt start lay outside the
  8 MiB window. The strengthened journal regression reproduced that failure
  (`/tmp/loo293-tail-large-before.log`). The bounded reader now also accepts
  positive canonical conversation evidence after the last unreadable record;
  it never guesses mode from a summary mirror. It reduces records sequentially,
  without retaining a parsed copy of the entire tail window.
- Fresh configured CLI proof from `/tmp`, with the published Home selected
  explicitly, now reads all three LOO-293 sources successfully, with zero records
  and zero gaps on both live initialization and file-cursor continuation. Recorded
  durations are 2.426 s and 0.327 s, not performance budgets. Binary identity and
  scope are in `watch-tail-proof.json`. This proves a quiet passive start and
  reconnect against real data; it supplies no new live-arrival/provider or UI demo.
- Remaining: discovery/initialization and retained per-Run/unfinished-part state
  still scale with inventory; a source whose tail cannot establish identity or
  journal capture mode reports `tail_unavailable` and reads retained history.
  History can overlap live output, so the feed must merge identities/revisions
  and avoid replacing a newer live revision with an older historical one. Bounded
  discovery/state, complete capture, connected diagram/feed/filtering/Follow live,
  exact human Session navigation and the configured human demo remain in this
  same Task/PR. No publication, landing or Task completion occurred in this pass.

- 2026-09-23 inline layout follow-through: extended the existing window-backed
  Watch rendering proof to mount the full SessionsView with the Work navigator
  visible. It checks the selected Task header, Work details action and retained
  Watch store, then renders the real native controls. Both standalone and
  integrated cases pass (`TaskWatchTests/renderSnapshot`); the integrated
  1200×720 image was inspected for readable navigation, stage/attempt placement,
  and clipping. Receipt: `/tmp/loo293-watch-inline-render.log`; image:
  `/tmp/loo293-watch-inline.workspace.png`. This is fixture rendering, not a
  configured provider or keyboard-interaction demo. The preceding 18-test
  navigation/Watch receipt remains unchanged; no broader gate was repeated.
  Review found no new owner, reader, persistence or provider action in this
  integration. Full capture, bounded discovery, independent history/live
  continuation, the feed and configured human demonstration remain required.

- 2026-09-23 unified navigation integration: Watch is now selected Task content
  in the main-view-task workspace. Removed WorkSurfaceView's separate sheet and
  terminal-store reference. The existing navigation owner retains Watch selection
  by Task within a window/repository; the reader is mounted only for visible
  Watch content. The existing terminal workspace remains unchanged underneath.
- The shared WorkspaceProjection retains completed Tasks. Show completed Tasks,
  search, current selection and open Sessions govern navigator visibility without
  creating another inventory. This only reaches Tasks in the roadmap snapshot;
  it does not add archived PM discovery.
- `swift test --package-path swift --filter
  'WorkspaceNavigationTests|TaskWatchTests'` passed 18 tests (including a two-case
  partial-planning test) and built/linked the Mac target. New tests exercise
  completed Tasks without Sessions, primary Watch navigation, hidden reader
  absence, per-Task/repository selection, and retained terminal layout. Existing
  cancellation, stale-state, navigation and Session association tests still pass.
  Source inspection confirms repository changes replace SessionsView by repo
  identity and Watch has no new polling or Session inventory.
- A concurrent run checkpointed production changes at `01148246b` while these
  focused tests were being added. This implementation's final proof includes the
  subsequent test changes. No publication or landing was performed by this run.
  The full feed, bounded discovery/independent cursors, exact human Session links,
  configured live proof and human gate remain required.

- 2026-09-23 Mac snapshot inspection: Watch now opens from Work, Roadmap, and
  Wave Task detail without runtime/workspace prerequisites. It renders retained
  invocations, selectable stages, ancestry/kinds, attempts, readiness/failures,
  exact transition links, and all attributed Runs. Selection survives refresh and
  completion; unavailable reads preserve the last snapshot with a stale notice.
- Watch reads on opening/manual Refresh only. RegistryQueryLocal now propagates
  cancellation through the existing launcher runner to its exact query subprocess;
  no provider client or Session operation is involved. A newer refresh supersedes
  a still-finishing query so reopening cannot strand an empty Watch or overwrite
  fresh evidence with the older error.
- `LF_WATCH_RENDER_PATH=/tmp/loo293-watch-snapshot.png swift test --package-path
  swift --filter TaskWatchTests` passed all 5 focused tests and built/linked the
  Mac target. Tests cover selection across transitions/completion/repeated names,
  stale evidence/recovery, Iterate navigation/separate attempts, no-workspace
  access, cancellation before/after subprocess spawn, and overlapping refreshes.
  The real Watch view was rendered from the shared DTO fixture in an offscreen
  AppKit window and visually inspected. This is fixture UI proof, not a configured
  live demo. Swift boundary and whitespace checks passed; existing Ghostty symbol
  warnings remain. No Rust, broad-suite, Xcode hosted, publication, or human gate
  was run for this slice. See `watch-plan-inspection.md` for its review limits.

- 2026-09-23 slice review: repaired shared Run-directory discovery after a real
  unreadable-prefix CLI regression failed with exit 1. Individual enumeration
  and metadata errors now become Task discovery gaps or existing reader warnings,
  preserving healthy Runs. The rebuilt synthetic proof retained eight output
  records and both Watch/shared-reader Runs beside the bad prefix; restoration
  cleared gaps. Fresh output discovery and two shared scanner/Session tests passed.
  Fresh configured reads from `/tmp` returned 3 Watch Runs with the truthful
  older-Home position gap and 197 historical output records from 3 sources without
  gaps. See `review-slice.md`; the full UI/capture/polling/demo requirements remain.

- 2026-09-23 reader repairs and snapshot integration: Claude/Codex calls and
  results now share their native conversation item ID while source record IDs
  remain distinct. Missing call IDs report an explicit identity gap. The
  cross-page reversed-results regression passed for both providers; the two
  existing JSONL identity/paging regressions also passed.
- Task manifest discovery now returns readable attributed Runs plus explicit
  discovery gaps. TaskOutputPage has required page-level `gaps`, mirrored in
  Swift and the fixture; TaskWatchSnapshot consumes the same discovery result.
  The 51-old/new-auxiliary-Run test passed with corrupt, missing, and recovered
  unrelated manifests. The rebuilt synthetic CLI proof retained both providers'
  call IDs and returned eight healthy records plus `discovery_incomplete` for a
  corrupt unrelated manifest, with exit 0 and no stderr.
- The snapshot CLI and Swift query now project retained plans, ancestry, attempts,
  transitions, settlement, active stage, and attributed Runs. Read position and
  events in one SQLite transaction; missing evidence remains explicit. Reuse
  Swift's existing step-kind type. Nine focused durable-store tests passed,
  including snapshot checks for repeated skills, Iterate/retry, separate attempts,
  completion/reopen, replacement, missing plans/Run receipts, and auxiliary Runs.
  Rust output/watch fixture tests passed (2); Swift output/watch fixture tests
  passed (2), including required-field rejection.
- The first configured Watch CLI attempt found the existing Home lacks
  `task_flow_positions`. The reader now reports `position_unavailable` while
  retaining available history and Runs, without migrating the database. The
  focused read-only missing-table regression passed and verified no table was
  recreated. This is explicit unavailable evidence, not a legacy position reader.
- Final configured proof: the rebuilt `lf task watch LOO-293 --json` succeeded
  from `/tmp` against the existing Home in 0.343 seconds, returning three Runs,
  zero retained invocation plans/attempts, and `position_unavailable`, with no
  stderr. It proves passive access and truthful older-Home evidence, not a live
  stage diagram. The synthetic CLI proof also returned both healthy Runs and
  `discovery_incomplete` through Watch with the corrupt manifest present.
  `cargo fmt --all -- --check`, `git diff --check`, and
  `cargo clippy --all-targets -- -D warnings` passed after integration. The
  missing-attempt settlement regression was also checked against the final
  compiled test executable. No broad gate or configured Mac Watch demo was run
  by this implementation pass; the separate concurrent gate report retains its
  own resource-preflight limitation.

- 2026-09-23: transactional flow facts implemented. Final focused command
  `cargo test -p loopflow --lib durable_store_tests --no-fail-fast`: 8 passed,
  including exact bindings, Iterate/retry, restart, completed history, rejected
  writes, atomic rollback, and no parent outbox delivery. The initial 2-pass /
  3-fail run exposed old assertions that counted only progress/failure events;
  they now distinguish flow history.
- 2026-09-23: `cargo fmt --all -- --check` and
  `cargo clippy --all-targets -- -D warnings` passed on the final code. No
  affected-suite/full-repo gate or configured Mac demo was run in this slice.
- 2026-09-23: native Claude/Codex JSONL record keys and OpenCode SQLite schema
  inspected; no client launched or interrupted. Native normalization, mutable
  part continuation, source-location receipts, and live demo are still pending.
- 2026-09-23 review-slice: eight store tests passed again. Controller review
  exposed two empty-ledger assumptions; these now assert exact history is
  unchanged by resumable failure. Eighteen controller tests passed initially;
  both corrected tests passed in a focused rerun. The Task's first-slice capture
  coverage remains unproved; the native contract above is a proposal, not
  implemented capture. See `scratch/review-slice.md` for the evidence matrix
  and required next implementation. No publication or demo readiness claimed.

- 2026-09-23 native/output implementation: provider Session receipts now retain
  an optional launch/hook-owned JSONL or OpenCode database location. Passive
  readers use that location or the exact recorded account; they never select an
  account, refresh credentials, resume a provider, or write its transcript.
  `lf task output` and `RegistryQuery.taskOutput` share the new output fixture.
- 2026-09-23 focused proof: five output-reader tests passed (complete lines,
  malformed/reset handling, oversized-record continuation, Codex mirror
  exclusion, journal normalization, and equal-timestamp OpenCode revisions with
  a concurrent SQLite writer). Two native-source tests passed for effective
  roots and exact-account isolation; the Session-receipt preservation test
  passed. The Task discovery test covers 51 old Runs, a newly added auxiliary
  Run, no replay, absent worktree, and cross-Task cursor rejection.
- 2026-09-23 wire proof: Rust `--test dto_fixtures task_output` and Swift
  `DTOFixtureTests/taskOutputFixture` each passed. Swift built the typed reader;
  this is not Watch UI evidence.
- 2026-09-23 configured passive proof: Run
  `run_033f7ed7985145ee8ca727ececb7f7cd` produced one new prose item and five new
  tool records while its exact owned Codex client remained unchanged (one
  client; offsets 7,416,215 → 7,614,044). The ignored configured test passed in
  22.14 seconds. An earlier quiet Session produced no new records in 45 seconds
  and did not satisfy live proof. No client was launched, attached, or replaced.
- 2026-09-23 configured CLI: the branch-built `lf task output LOO-293 --json`
  ran from `/tmp` against the existing read-only registry and returned 167
  records from three attributed sources (one journal, two native Codex), no
  source gaps, and no stderr. The first attempted CLI used the ordinary Task
  runtime/store wrapper and hit the development-schema guard; output now has
  direct read-only dispatch. The proof did not advance production Task state.
- Configured Codex records exposed a 6.5 MiB compaction record and large tool
  results. The finite JSONL page bound is now 8 MiB; larger records report a gap
  and skip to the next complete record without permanently stalling output.
- Remaining: configured Claude/OpenCode live proof, bounded incremental manifest
  discovery, separate historical/live cursors, Watch snapshot and UI, filtering
  and Follow live tests, full configured demonstration and human gate. OpenCode
  arbitrary compaction coverage remains limited as noted in `questions.md`.

- Summary-only journal attempts now report `limited_capture`: their normalized
  stream lacks complete tool results. The full Task still requires resolving
  those autonomous/auxiliary capture paths; explicit gaps are failure handling,
  not satisfaction of complete output coverage.

- Final validation for this implementation pass: `cargo fmt --all -- --check`
  and `cargo clippy --all-targets -- -D warnings` passed after the final code
  change. The focused journal proof passed again after adding `limited_capture`;
  the discovery proof passed after directory errors became explicit. No broad
  affected-suite gate, full repository matrix, or Mac Watch demo was run.

- 2026-09-23 compression: removed TaskOutputRecord and the top-level parallel
  records list. TaskOutputSource now owns labels, records, and evidence state;
  OutputRecord passes directly from native/journal readers through Rust, CLI,
  and Swift. Source and record array order preserve the prior flattened order.
  No alias, old-shape decoder, new store, or execution behavior was added.

- 2026-09-23 review validation: the timestamp-only OpenCode regression failed
  before correction. After correction, `cargo test -p loopflow --lib opencode_
  --no-fail-fast` passed all 63 matching tests (including launch/harness/auth
  tests, not 63 passive-reader tests). Formatting and full-target Clippy passed.
  No full repository suite or configured Watch demo ran; review disposition
  remains return to implementation.

- 2026-09-23 reader review: fresh configured grouped CLI reads from `/tmp`
  returned 568 records over four pages, with no repeated revisions or source gaps.
  All pages still had historical output pending; this is not fresh live arrival.
  Review reproduced an OpenCode timestamp-only replay and corrected retained
  cursor timestamps. It also established that a 2 MiB command argument fails on
  this host below the reader's 4 MiB cursor bound. See `review-slice.md` for the
  evidence matrix and required continuation/capture work. No publication or
  human-gate readiness is claimed.

- 2026-09-23 continuation implementation: CLI cursor input is now `--cursor
  FILE` or `--cursor -` for stdin. RegistryQuery transports cursor values in a
  private temporary directory removed after the query; no compatibility parser,
  provider-storage writer, or durable watcher cache was added. Input and output
  both enforce the existing 4 MiB bound, and opaque cursor version is now 2.
- OpenCode sweeps now freeze their upper timestamp and retain both inclusive
  boundaries until paging ends. The extended interleaving test failed before
  correction. An intermediate pruning change replayed the old boundary; keeping
  both boundaries corrected it. The final three focused OpenCode tests pass,
  including timestamp-only updates and 1,000 distinct-timestamp completed parts
  with a serialized source cursor below 2 KiB throughout and no replay.
- Fresh configured CLI transport proof from `/tmp`: initial/file/stdin reads
  returned 196/145/128 records (469 distinct revisions) across three sources,
  with no gaps or stderr. The stdin cursor was 2,186,420 bytes: the valid opaque
  JSON state was padded with whitespace before encoding, so this proves transport
  capacity, not a real Task with that much retained state. History remained;
  this is not new live-arrival proof. No production Task or provider was mutated.
- Focused verification: `cargo test -p loopflow --lib
  run_record::output::tests::opencode_ --no-fail-fast` passed 3 tests;
  `cargo test -p loopflow --bin lf task_output_cursor_file --no-fail-fast`
  passed 1; the old/new auxiliary Run discovery test passed 1;
  `swift test --package-path swift --filter RegistryQueryTests/taskOutputLargeCursor`
  passed 1. Branch CLI build, formatting check, and full-target Clippy passed.
  The earlier seven-reader run had six passes and the intermediate boundary
  pruning failure; the final focused three-test rerun covers the corrected path.
  Swift emitted the existing Ghostty symbol warnings. No broad gate or UI demo ran.
- Remaining continuation limits: per-Run state, large timestamp boundaries and
  unfinished parts can still reach the explicit cursor cap; discovery is still a
  full manifest/flow-fact scan. Separate historical/live continuation and arbitrary
  backdated/compacted OpenCode history remain unfinished. This pass does not claim
  complete capture, Watch, publication readiness, or the configured human gate.

### Prior continuation work retained in the checkout

Repair continuation at its existing owners. `--cursor FILE` reads the opaque
value from a file, and `--cursor -` reads stdin; the CLI never puts cursor content
in argv. RegistryQuery uses one private temporary directory for the query and
removes it after success or failure. This is transport only, with no retained
watcher store or provider-storage writes. The shared operation still consumes
an opaque value and enforces the same input/output size bound.

OpenCode freezes each paginated sweep's upper timestamp. Newer writes wait for
the next sweep so they cannot advance the watermark past same-timestamp edits
behind the current page. Retain hashes for the old and new inclusive boundaries
and unfinished parts; discard interior completed-history hashes while paging.
Opaque cursor version changes because the continuation semantics changed.

Focused proof: a 2 MiB cursor traverses file and stdin transport; the Swift query
cleans up private cursor files on failure; equal-timestamp edits and newer writes
between pages arrive once; timestamp-only changes do not replay; 1,000 completed
parts remain readable without accumulating historical hashes in continuation.
Use the configured Task CLI passively to prove the new transport, printing counts
rather than conversation content.

This does not establish independent history/live cursors, bounded manifest/flow
fact discovery, or arbitrary backdated OpenCode rewrites. Retained same-timestamp
and unfinished parts plus the Task's per-Run cursor map can still hit the finite
cursor bound; exceeding it must fail explicitly rather than return a continuation
that the next request cannot consume. These remain full-design requirements,
alongside complete capture, Watch, and its configured demonstration.


### Watch snapshot implementation — 2026-09-23

- Added `lf task watch ISSUE --json`, the Rust TaskWatchSnapshot projection, and
  RegistryQuery.taskWatch with required-field Swift models and a shared fixture.
  Plans, labels, ancestry, attempts, Iterate/retry edges, and settlement come from
  retained facts. The active position and Task events share a SQLite read
  transaction; manifest attribution includes auxiliary Runs and missing Run gaps.
- Review retained blocked attempts when a retry binds, avoided inventing completion
  on replacement, and made a settlement with no attempt explicitly incomplete.
  Watch and output now share the same read-only Task lookup. No provider control,
  transcript writer, schema migration, current-YAML reconstruction, or Swift
  lifecycle reducer was added. The source-availability reader remains task output.
- Focused Rust proof passed: `--lib
  flow_history_preserves_repeated_skills_iterate_retries_and_completion` (1),
  `--lib flow_history_retains_restarted_plans_without_accepting_stale_writes` (1),
  `--lib watch_settlement_without_attempt` (1), and `--test dto_fixtures task_watch`
  (1), all with `--no-fail-fast` and inherited LF_RUN_ID unset. The store proof
  reads live readiness and auxiliary attribution, then retained attempts/edges
  after completion and database reopen; deleting the plan receipt proves gaps
  without reconstructing history. Replacement does not turn its bound attempt
  into a completed attempt.
- `swift test --package-path swift --filter DTOFixtureTests/taskWatchFixture`
  passed (1). Branch CLI build, `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed.
- Configured branch CLI from `/tmp`: `lf task watch LOO-293 --json` returned the
  exact Task ID, 3 attributed Runs, 0 retained invocations, no active-stage proof,
  and one `position_unavailable` gap in 2.267 seconds, with no stderr. The Home
  predates Task flow-position storage. Its absence stays explicit and does not
  trigger migration or imply completion. This is configured read-path proof;
  it is not a live diagram, stage transition, or complete Watch demonstration.
- Concurrent lifecycle activity committed drafts and changed reader discovery
  during this implementation. Those changes were preserved. The receipts above
  are focused command results on the shared working tree, not a passed gate for
  a frozen combined head. The externally added reader fixes retain their own
  proof requirements; this pass does not claim their tests as its own.
- Remaining: bounded history/discovery before polling, independent output
  history/live continuation, complete configured capture, the Mac Watch surface,
  filtering/Follow live, and the full configured human demo. No publication,
  landing, or Task completion was performed by this implementation pass.
