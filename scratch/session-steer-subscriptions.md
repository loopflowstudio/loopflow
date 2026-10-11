# Session steer subscriptions

Task `lf-9c03797`, 2026-10-11. Jack Heart accepted Session-only delivery with
repository/Work wrappers (“ok, pursue”), then renewed pursuit after the provider
gaps (“pursuse!”). One indivisible PR replaces publication, launch seeding and
live injection together. The replacement is implemented locally; authored review
and gate/CI acceptance remain. No product approval is missing.

Reconciled October 11 against `cd2c38184f74`: the iteration feedback describes
the withdrawn cut at `0ffdb9c04a`, before replacement `0d04805ab3` and compression
`27ce387bdc`. Current source contains the public `session_steers` fixtures and
shared dispatch; the old injection paths remain deleted. This supersedes the
feedback's source inventory, not its acceptance obligations. `f96a7d2378` corrects
capability status; `cd2c38184f` removes publication-time Session scans and reuses
reconciled applicability. Source review also corrected README's obsolete claim
that independent Task conversations receive no direction. No runtime repair was
needed in this reconciliation.

## Remaining work

- Gate runs affected suites and automated acceptance once. Configured
  Codex/OpenCode and native-model acceptance remain unproved. Protocol stand-ins
  establish transport and receipt behavior, never model acceptance.
- Review/demo exercises the experience below. No follow-up Tasks or separate
  cutover PRs are selected. Repository/Wave exchange remains explicitly unavailable;
  existing remote routing selects the destination Machine's Session store.
- SwiftPM's nested sandbox could not start in the previous step. The changed
  author model compiled and round-tripped independently; CI owns the full suite.

The complete prior design, provider-source findings and dated implementation
account are preserved at
`0d04805ab35f2ba0ee4d72dec3cc9caa1d1aa051:scratch/session-steer-subscriptions.md`.
That account is evidence, not configured acceptance.

## Accepted behavior

Publish authored repository, Wave, Task or direct Session direction for delivery
at the next supported provider boundary. A Session receives its scope and
ancestors, never siblings or parents. Taskless Sessions receive repository and
direct direction. Existing Rust checkout/explicit associations and Wave ancestry
own matching; no second Work tree or Session registry.

Each Session receives direction independently. Publishing starts no turn, worker,
replacement conversation or terminal input. Idle Sessions stay idle. A new
Session or newly added association receives existing applicable, nonsuperseded
publications through the same bounded seed. Leaving and re-entering a scope does
not replay emitted history. Resume, compaction and provider replacement preserve
receipts; a new Flow-step Session has its own receipts.

Routine agent progress stays excluded. Authored history, names, immutable source
revisions, planning conflicts and outbound Linear receipts survive. Unknown
historical attribution remains unresolved, never inferred from the account owner.
Linear publication and local hook emission establish neither model reading nor
compliance. This work is separate from LOO-427's Desktop/CLI changes.

## Data and public surface

```sh
lf comment TASK "keep the public name" --steer
lf wave comment WAVE "preserve native history" --steer
lf repo comment "retain source attribution" --steer
lf session steer SESSION "inspect the last failure"
lf session steers SESSION --json --limit 32
lf session steers SESSION --after 42 --json
lf session steers SESSION --source PUBLICATION
lf session steers SESSION --retry PUBLICATION
```

The existing `lf comment` Task shorthand survives the scoped commands.
Publication returns saved identity immediately. Repository and Wave commands
explicitly report Machine-local persistence and unavailable shared exchange.
Remote Session commands use existing destination routing; claims and receipts
never enter shared planning transport. No new cross-machine bus is authorized.

`SteerPublication` owns UUID, local sequence, source/revision key, author and exact
text. Unique source/revision prevents reacquisition echoes; equal text or time
is not identity. A correction supersedes pending older revisions of its source;
already emitted revisions remain history and the correction is new direction.

`session_steers` owns the idempotent Session/publication association and explicit
retry attempt. Receipt reads and callbacks reconcile their Session from existing
Work membership; publication only saves immutable direction. Repository, Wave
and direct Session wrappers share one save path, without scanning every Session.
Reconciliation refreshes applicability before cancelling prepared batches and
removing undelivered obsolete rows; those operations reuse the refreshed flags.
No editable membership list exists.
The unused public `steer_session` wrapper and write-only
`session_steer_subscriptions` table are removed: its association JSON and activation
time had no readers, while native activation already belongs to `steer_adapters`.

`steer_batches` records provider identity, observed attachment claim, boundary,
payload hash and evidence. `steer_deliveries` records the selected publication,
attempt and excerpt flag. Public status pages expose pending counts and adapter
observations without materializing the whole backlog.

Task comment save/acquisition and definition adoption publish inside their existing
planning transaction. The single migration draft converts historical Task/Project
steer events and nested TaskObserved copies to publication references, including
orphaned copies, while retaining complete text and planning receipts. Historical
Project direction stays readable; new Project publication is outside scope.
Released migrations remain immutable. No installed store was migrated for this work.
Main's v0.13.12 merge materialized `process_terminal` as
`0.13.12.001_release.sql`; the sole `session_steers` draft now follows that released
frontier. Release's operation-entry lesson applies: store migration and receipt
proofs do not establish public launch or configured provider acceptance.

## Provider authority and startup exclusion

The capability belongs to the selected AgentProcess and native conversation,
not the launching client's attachment token. Codex's live provider survives
A → B → A client handoff; the installed callback must survive too. Replacement
revokes its authority. Blindly substituting the latest client claim would authorize
stale callbacks. Removing Codex live handoff is outside this Task.

`agent_process::{spawn,spawn_native}` reserves the capability against the frozen
attachment and supplies private `LF_PROVIDER_CONTEXT` immediately before launch.
This includes Claude's `ensure_process` replacement and native OpenCode startup.
A root startup callback may activate an unbound native identity; later callbacks
must match the recorded identity. Child callbacks with `agent_id` are excluded
before activation. Capability reservation is not installation or acceptance;
`activated_at` means an authenticated callback was observed. Public receipt
status says “capability reserved” until that observation, replacing the
unsupported “installed” claim found during reconciliation.

Provider input dispatch and context output hold shared Session exclusion through
bounded writes. Spawn, stop, replacement and attachment mutation stay exclusive.
Claude serializes stdin through `&mut self`; Codex's writer queue and proxy each
own their socket; OpenCode serializes HTTP submission. SQLite transactions finish
before transport I/O. Neither dropping the fence before output nor increasing
its timeout can substitute for this split.

The withdrawn design used exclusive input and hook fences. Claude 2.1.296 invokes
SessionStart before draining stdin: the hook waited for the writer, the writer
waited for drainage, and only 512 of 131,133 bytes wrote within two seconds.
A 4,194,365-byte probe also wrote only 512 bytes. The installed-provider probe used
a disposable Home, synthetic authentication and an unavailable loopback endpoint;
it proved ordering, not model delivery. Its `--shared` variant wrote all 131,133
bytes within the same bound. Preserve this counterexample.

The reproducer is archived at
`0d04805ab35f2ba0ee4d72dec3cc9caa1d1aa051:scratch/claude-startup-fence.py`.
The withdrawn incomplete diff remains at
`0ffdb9c04acdca129767b0e45719e6f00390bed2:scratch/session-steer-subscriptions.patch.gz`,
based on `a839b8fad2`. Four receipt tests from that attempt proved no cutover.
Main #1522 (`6016eca305`) supplies the shared Claude harness and native answer
correlation; its deleted batch launcher and transcript-answer fallback stay deleted.

The replacement's public `session_steers` fixture forces startup/input overlap
without draining the combined seed, then invokes the actual hook command. It
preserves 150,000 UTF-8 clipboard bytes, both skill/query records, native identity
and correlated answers. Resume uses a fresh AgentProcess and capability while
preserving native identity and distinct answer correlation. Oversized direction
stays within the combined rendered ceiling and is not replayed on resume.
The dispatch race proves takeover waits for both shared writers and rejects the
former claim afterward. The handoff fixture preserves the original launch
environment across A → B → A and rejects children, foreign native identity and
replacement. These are public protocol proofs, not configured Codex reconnect.

## Delivery transaction and bounds

Startup prepares the first batch after Session/AgentProcess allocation. The Task
brief is assembled independently and contains no steer bodies. This avoids a
second seed reservation before native identity exists. Main #1518/#1521's context
reader remains the sole owner of refreshed checkout files and the rendered
10,000-byte ceiling; compact-only skill context and native history survive.

A boundary validates authority, reconciles scope, reserves at most eight revisions
in acquisition order and records the observed attachment claim. Concurrent
callbacks cannot select the same receipt; repeated boundary IDs produce no second
payload. Rendering materializes only selected immutable source files. Each inline
item is capped at 1,024 bytes with UTF-8-safe excerpts, attribution and a complete
source pointer. Combined reference/steer context stays within 10,000 rendered
bytes. Receipts record excerpt-only delivery; pointers prove no source read.

After rendering, reconcile again and cancel a prepared batch if a source revision
or association changed. New arrivals wait for the next boundary. Rendering or
source-write failure releases the reservation. Do not hold SQLite transactions
across output. `emit_session_steers` returns whether it wrote a payload so the
caller can supply ordinary reference context when no steer was emitted.

| Evidence | Meaning |
| --- | --- |
| Pending | Saved; no emission attempt began |
| Prepared | Selected and reserved; no bytes exposed |
| Uncertain | Write was about to begin or failed; acceptance unknown |
| Emitted | Local output completed; provider acceptance unconfirmed |
| Confirmed | Owned provider input/history contains batch ID and matching hash |

Mark uncertain before output and emitted afterward. No current adapter produces
confirmed evidence. Exit zero, elapsed time, a later tool or model-generated
acknowledgement cannot confirm acceptance. Uncertain/emitted attempts never replay
automatically; explicit retry creates another attempt without waking the Session.
Retry cannot select inapplicable history.

Recovery cancels prepared reservations only after acquiring exclusive, nonblocking
Session exclusion, proving that no shared emitter remains. Age or liveness
inference grants no recovery authority. Emitted history survives scope changes.

## Provider boundaries

| Provider | Supported adapter boundary | Retained proof limits |
| --- | --- | --- |
| Codex | Synchronous PostToolUse; startup/resume/compact and UserPromptSubmit | Exact declaration trust hashes cover each event. Published 0.161.0 `agent_id` can identify children even when `session_id` names the parent. Configured nested code-mode visibility remains unproved. |
| Claude | PostToolUse, PostToolUseFailure, startup/resume/compact and UserPromptSubmit | Validation rejection and cancellation need not emit a failure hook; direction waits for a supported boundary. |
| OpenCode | Messages transform before the next model request | 1.18.33 gives empty transform input, not a provider request ID. Generate an adapter invocation UUID. Internal processor retries reuse transformed input; later callbacks replace the ephemeral part without reselecting consumed receipts. Lost transforms remain emitted/unconfirmed. |

The shipped JavaScript protocol fixture invokes the real receipt command and
covers retry-input reuse, lost transforms and later callbacks. It proves no
configured OpenCode acceptance. Provider source findings and exact URLs remain
in the prior design reference above. Preserve user hooks/settings and native
titles, guide loading and compaction. No global trust bypass. Already-open
providers without declarations need a provider start; Codex attachment reconnect
strips configuration overrides and is not adapter installation.

## Delete — do not maintain

Completed in this branch; no known production deletion target remains:

- `RunningFlow.steers`, per-node cursors, `Cli.steers_after` and forwarding/seed
  filters; exclusive repeated-node test in `tests/flow_tests.rs`.
- `ops/child.rs::inject_live_steers`, TaskInput steer polling/cursors/seed receipts
  and exclusive live-injection test. Interrupt polling and attached-client input
  remain; other `send_current` callers remain valid.
- Event-backed `task_steers`, `steers_since`, `append_steer` and
  `append_task_steer_in`; duplicate event bodies and launch prompt attribution.
  Comment/definition writers, activity/Wave readers and CI human-assistance
  projection consume publications. Progress exclusion and history tests survive.
- Consumed direction in refreshed Task summary references; the refreshed brief
  remains independent of publication receipts.
- Write-only `session_steer_subscriptions` and unused `steer_session` entry point.
  The draft is edited in place; no intermediate-schema migration is added.
- Publication-time all-Session reconciliation and the duplicate direct Session
  save transaction. Reads and callbacks already resolve current membership;
  the mixed-batch correction fixture preserves unaffected pending direction.
- Scratch startup reproducer after the public replacement fixture passed;
  precise historical source and counterexample are retained above.
- `scratch/questions.md`: resolved engineering decisions duplicate this design;
  no unresolved product question remains.

## Acceptance experience

Use two conversations on one Task, a sibling Task conversation, parent Wave and
taskless Session; keep one idle. Publish repository, ancestor Wave, Task and
direct Session direction. Active conversations receive exactly their applicable
publications at supported tools, with author/source; idle starts nothing and gets
pending direction on ordinary continuation. Inspect per-Session receipts, then
resume and compact without replay. Exercise failed tools, Unicode/backlog bounds,
concurrent/duplicate callbacks, pre/post-write failures, explicit retry, replacement,
association/ancestry changes, revision deduplication and released-frontier history.

Headless fixtures use disposable stores, cleared inherited `LF_*` and fixture
`LF_BIN` inside the network boundary. Gate owns broader Flow/lib suites, remote
routing, unavailable/untrusted/open-provider cases and configured acceptance;
review owns the real experience. Never report unavailable checks as passes.

Check (October 11): `git diff --check` and `lf context` pass; prose-only reconciliation reuses the prior `uv run python scripts/test_network.py cargo test -p loopflow --lib store::sqlite::steers < /dev/null` (8), `--test session_steers < /dev/null` (3), `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` passes; gate/CI retains full Swift, broader suites and configured-provider acceptance.
