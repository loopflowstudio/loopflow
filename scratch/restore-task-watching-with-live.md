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

`lf task watch ISSUE --json` → TaskWatchSnapshot: Task identity, invocation plans,
stages/transitions, attributed Runs, source availability, history cursor.

`lf task output ISSUE --json [--cursor FILE]` (use `-` for stdin) → TaskOutputPage: ordered source
entries, each carrying its Run/provider/stage labels, ordered records, availability,
reset state, and gaps. OutputRecord carries only source item identity, revision,
and the normalized conversation event. Clients retain source order and record
order without joining a separate status list. The page carries Task identity and
opaque continuation; separate historical continuation remains to implement.
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
merges records, preserves filters and last-good evidence, and marks stale reads
explicitly. Poll once per second while visible; cancel subprocess reads when
hidden. Reuse the existing Session navigation for checkpoints and LOO-291's
shared Task identities where available. No dependency on its landing.

## This slice

Close the narrow provenance/capture-contract checkpoint requested by the human:
retain immutable invocation plans and exact stage/iteration/Run bindings,
transitions, readiness/failure, and settlement in the owning transactions.
Prove repeated skills, retries, human Iterate, restart, completion, rollback,
and parent-observation exclusion with focused Rust tests. Keep the native
passive-read contract above and provider-specific proof gaps explicit.

The existing output-query, native-reader implementation, and Swift output-model
work are preserved as later-slice working-tree evidence, outside this checkpoint.
Do not extend those paths, build Watch, run the configured demo, publish, or
complete the Task in this pass. Full-design requirements below remain unchanged.
See `narrow-checkpoint.md` for the checkpoint boundary and validation receipts.

## Remaining slices and full Done When

1. Flow facts are implemented. Current: native source receipts, passive readers,
   bounded output continuation, discovery, and the shared output fixture.
2. Finish provider-specific configured capture proof, the Watch snapshot, and
   separate historical paging/live continuation.
3. Watch UI and all Task entry points. Swift tests prove filtering and Follow
   live across transitions. Complete the configured-path demo and human gate.

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
