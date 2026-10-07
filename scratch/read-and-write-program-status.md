# Read and write Program Status (LOO-398)

Draft implementation design — 2026-10-07. Jack Heart's scope and steers below
are accepted direction; the mechanisms here are proposals. No implementation,
GhosttyKit upgrade, installed acceptance or provider adoption is claimed.
Baseline: `6448e3c9e7e519585378feaffe04606bc1b55d3e`.

## Outcome and demo

Every Desktop workspace pane understands a program saying it is working,
waiting, finished or failed, including a plain shell pane. A Session's report
feeds the same Waiting judgment used by Session lists and Task views. A machine
holding a relayed Session can record this with no Desktop or viewer connected.
Someone running `lf` in another terminal receives its own headless-work and
Flow reports without installing a terminal-specific integration.

Reading demo: a small spec-authored test emitter sends `blocked`,
`kind=question`, and a short message in a plain shell pane, then in a native
Session pane. Both panes show the question as literal text; the Session appears
in `lf session list --waiting --json` and its Task's Waiting group. Answering
makes it working. Repeat with Desktop absent and the relay still running;
reconnect to the same Session and read its current status without replaying a
question or starting another provider.

Writing demo: `lf run status-demo` uses a fixture Flow with a deterministic stub
provider, proceeds through working → explicit question → working → done in a
supporting terminal. Flow and step coexist with a provider's root record. A
headless PTY capture can demonstrate the emitted reports until a real consumer
is available; it does not establish third-party terminal rendering.

This serves Infrastructure's KR that people need not track Sessions or chase
work. No quantitative chapter target is supplied. Acceptance measures truthful
state and uninterrupted terminal behavior, not an invented KR score.

## Accepted direction and dependency boundaries

- Jack Heart expanded writing to reading on October 7, then specified **every
  pane**, native-provider passthrough, unattended relay observation, and `lf`
  reports only for state the underlying program cannot describe.
- Implement from the [revision 0.2 specification](https://www.superlogical.com/rex/docs/build/program-status),
  not copied code, tests or help from Ghostty, cmux, herdr or another implementation.
  Using the existing GhosttyKit dependency and its public API is the intended
  reading path. Inspecting an API is not permission to transplant a parser.
- LOO-394 owns the transparent PTY relay, detach/attach, remote transport and
  provider lifetime. Its October 7 Task status shows PR #1484 implements only the
  Home→machine rename. The relay is absent here. The relevant sibling draft
  (`scratch/work-on-another-machine-name.md`, sections 4–5) selects one detached
  holder per Session but leaves Codex engine replacement and shell restoration
  unresolved. This Task does not choose either or build a second holder.
- LOO-402 owns the broader sidebar and pane-strip Waiting presentation. Supply
  its existing Session attention value plus optional reported detail; implement
  the minimal pane status/detail needed for this Task, not a parallel inbox.
- LOO-384 remains useful **only for non-reporting native programs**. No Claude
  release with complete reporting was verified. Do not cancel it based on verbal
  support or add a transcript tailer here. Once a generation reports, transcript
  and quiet-time inference cannot override it. Reassess the remaining LOO-384
  scope after an actual supported Claude version passes the same scenario.

## Inventory and resolved risks

| Current building block | Evidence and consequence |
| --- | --- |
| `harness/attention.rs` | `Attention` maps Claude stream JSON, Codex RPC and OpenCode events into tool/input counts and hand-back. Retain these for pipe-driven work. `InputRequested(String)` loses permission/question kind; enrich this signal instead of adding another provider parser. OpenCode permissions answered automatically by its driver must remain non-blocking. |
| `session.rs`, `store/sqlite/session_events.rs`, `store/sqlite/sessions.rs` | One `session_activity` row, current driver generation, and `waiting_sql` select Waiting before paging. The 120-second fallback is real; never export that timeout as `blocked`. `next_quiet_waiting` must exclude reporting generations too. |
| `session_record/activity.rs` | CPU/event sampling diagnoses Running/Stalled/Unknown. It is a different diagnostic with process-preservation consumers, not program input intent. Preserve it and never map Stalled to protocol blocked. |
| `engine/agent.rs`, `harness/{claude,codex,opencode}.rs` | Native launch inherits terminal I/O; headless paths capture provider output or drive RPC. Native OSC already passes through. No raw PTY relay exists at this baseline; `directive_relay` is unrelated shell-command plumbing. |
| `lf/commands/flow.rs::Driver` | Owns compiled graph, cursor and child Exec launch; `ops/flow_run.rs` owns append-only FlowExec records. Emit from these real transitions, not reconstructed logs or a new progress table. |
| `GhosttyManager.swift`, `GhosttyTerminalView.swift` | Surface callbacks carry `TerminalIdentity.session` or `.shell`; bell and title resolve identity while the surface pointer is valid. Copy status values inside that callback before crossing actors. Retained surfaces, rather than only mounted views, must own subscriptions. |
| `SessionsView.swift` | `bellRinging` is transient bell presentation, cleared on focus; it is not Waiting. Keep actual bell behavior. Never repurpose a bell or title as status. |
| `session_record.rs::current_terminal_id`, `ops/human_session.rs` | PTY-checked terminal markers and current client receipts associate native Sessions with shell surfaces. Reuse this association; protocol `id`, `app`, title and cwd never select a Session or Task. |
| GhosttyKit artifact and patches | Pinned `4c838723…`, artifact `GhosttyKit-4c83872-lf3`. Preserve patch 0001 command blocks/selection/reflow and patch 0002 embedded login-session opt-out. Upgrade the pin, artifact checksum, packaged shell resources and `GhosttyRuntimeResources.sourceRevision` together. |

The spec and author's [article](https://mitchellh.com/writing/program-status-osc7501)
were retrieved directly on October 7 after the web reader failed. They establish
protocol semantics and reported libghostty integration, not a shipped consumer.
Jack Heart independently supplied this boundary in comment
`6fbe3b94-8d14-4480-8a0f-683c3aea461a`. Direct checks against candidate commit
`a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0` confirm both
`src/terminal/osc/parsers/program_status.zig` and libghostty-vt’s public
`GhosttyTerminalProgramStatusFn` / `GHOSTTY_TERMINAL_OPT_PROGRAM_STATUS`.
The latter delivers validated, decoded, callback-lifetime strings; its contract
explicitly leaves record storage and prompt/exit lifetimes to the caller.
Its reset callback supplies root clear. Query replies require the registered
callback plus the write-PTY callback. The exact commit’s embedder header has
`PROGRESS_REPORT` and `COMMAND_FINISHED`, but **no Program Status action**;
header history reports `714fe9b90373f88300e0f6ba3c51837439164a7e` as its last
change. Parser presence does not prove record storage, lifetime handling or a
usable callback. No upstream parser implementation was read or copied.

Therefore an artifact bump alone is not a supported plan. The first implementation
slice must prove the embedded API. Use `a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0` as the build candidate;
carry the two patches forward and add `0003-program-status.patch` to forward
validated reports into the embedded action API. The patch also exposes the
prompt/reset/exit boundaries the app needs; it does not assume libghostty-vt
stores records. Author the bridge from the public API and lifetime behavior
from the spec, with original fixtures. If implementation selects a newer revision,
repeat the header check and omit the third patch only when that revision proves
the equivalent action and boundary behavior. Do not create a competing Swift terminal parser. A missing callback or
failed patch preservation test blocks the Desktop integration cut, not the Rust
codec work; do not label that cut complete until the ABI works headlessly.

## Chosen architecture

### Reports and one Waiting judgment

Add `rust/loopflow/src/program_status.rs`: a bounded incremental OSC recognizer,
validated report/record types, lifecycle reduction, and the small encoder. It is
not a screen emulator. The relay observes each output chunk before fan-out or
capture, without rewriting provider reports (query ownership is specified below).
Desktop consumes Ghostty's
validated action and keeps a bounded per-surface record reducer; it does not
scan rendered text or parse escape sequences. That reducer owns plain-shell
display only; Session Waiting continues to come from Rust. These are independent physical
terminal endpoints using the same spec fixtures, not two Session authorities.

For a Session, extend **the existing `session_activity` owner**, using one migration
draft, with an optional bounded reported snapshot and its stream identity.
A snapshot holds records keyed by protocol id, report-seen state, observation
sequence, and provider/terminal generation. Preserve existing fallback counters.
No new event journal, status sidecar, second Waiting table or heartbeat. Persist
changed snapshots, not each received byte or repeated identical report. Keep
raw terminal capture policy with LOO-394; do not inject locally synthesized OSC
into `session_events` or provider logs.

Writer selection is explicit:

- A relayed Session's PTY holder is its only report writer, attached or detached.
  Desktop shows the Session's committed Rust projection; its Ghostty callback
  must not write a duplicate observation for that Session.
- Before relay support, a locally embedded native Session can publish an observed
  snapshot through a public `lf session observe-status <session>` input stream.
  The app sends bounded structured snapshots and a current terminal/client receipt;
  Rust resolves the association and accepts only the active surface incarnation.
  This grants display observation only, never a driver claim. It remains the
  local surface observation path when no relay owns the stream, not a second
  holder or an arbitrary set-Waiting command. Coalesce on one long-lived input
  stream per observed Session rather than spawn an Exec per report.
- A plain shell with no Session retains records in its Ghostty surface. It shows
  pane detail and does not manufacture a Session or Task. If a current client
  receipt maps it to a Session, subsequent live observations may feed that Session;
  never replay earlier shell records into a newly discovered association.

Session association changes reset the observation cursor. All writes compare
stream identity and increasing sequence inside the same transaction; a delayed
callback from a closed surface or replaced provider cannot overwrite a new one.
The report stream follows provider lifetime, not a conversational driver's lease:
a driver handoff with the same provider does not erase status. Ending a stream
is distinct from losing its viewer. LOO-394 supplies the exact relay incarnation
and reconnect snapshot interface; integrate those directly when available.

Rust's single projection has this precedence within the current generation:

| Evidence | Session Waiting | Other display |
| --- | --- | --- |
| Any live reported `blocked` record | yes, immediately | kind, literal message, record/terminal identity |
| Reported `idle`, no blocked record | yes for an interactive Session | idle; plain shell idle is only idle |
| Reported working/done/error, no blocked or interactive idle | no | reported state and optional progress/detail |
| Valid clear after reports | no reported Waiting; do not resurrect stale inference | remaining records, or empty |
| No report in this generation | existing explicit stream/transcript fallback, then existing quiet rule | unknown when no usable observation |

Resolve a missing app through the nearest existing ancestor at display time;
do not duplicate inherited values in stored child records. A blocked child
survives an idle or working parent. Multiple records are visible
in detail; choose a summary deterministically: blocked, error, working, done, idle;
most recently updated breaks ties. Compute Waiting across the set independently
of that visual priority. `waiting_sql` and `next_quiet_waiting` must use the same
precedence and continue filtering before pagination. Source details are part of
an optional `program_status` field on `SessionRecord`, mirrored in Swift and DTO
fixtures with no defaults. Existing `attention` stays the sole Waiting result.
Update the existing `sessions` revision trigger for changed projected detail;
`lf monitor work --watch`, Session inventory and Task consumers read that result.

A report says what a program claims, never that its process exited. `done` and
`error` do not close Sessions, settle Flows, move Workflows, or finish Tasks.
Protocol IDs identify terminal records only. Unknown ownership stays unknown.

### Lifetimes, malformed input and presentation

A complete report replaces its record; omitted fields disappear. Clear removes
the addressed subtree, with component boundaries (`a` does not clear `ab`).
Handle OSC 133 A and observed attached-process exit: remove working/blocked,
preserve done/error. Select dropping idle at those boundaries. RIS clears all;
DECSTR and alternate-screen switches preserve records. No timer expires valid
reports. Actual keyboard interaction dismisses retained done/error; focus alone
does not. The relay observes input without changing it; Desktop dismissal of a
relayed Session travels through the relay's existing input path, not a second
local database mutation. A new program generation resets report precedence.
The prompt/reset recognizer needs only these fixed sequences, not terminal state.
Old fallback counters cannot cross that generation boundary either: a fresh
non-reporting generation needs its own observations before inference resumes.

Use the spec's grammar and hard limits: 4096 bytes per framed sequence, 16-byte
keys, 128-byte IDs, at most 8 segments of 32 bytes, 32-byte app, 2732/2048-byte
encoded/decoded message and 256/192-byte title. Select 64 records per terminal
(the minimum supported cap), evicting the least recently updated on insertion.
Validate every pair before mutation; skip malformed pairs and unknown keys,
last duplicate wins, ignore unknown state/invalid id, normalize unknown kind
or invalid progress to absent. Bad base64, invalid UTF-8, forbidden controls
or any hard-limit violation discards the whole report, retaining prior state.
Handle terminators and chunk boundaries correctly, including ESC-backslash and
BEL; incomplete/oversized sequences are bounded and resynchronize without
altering passthrough bytes. DCS payloads containing apparent OSC are not reports.

Treat message/title as plain strings, never Markdown or links. Disarm bidi and
invisible formatting at the display boundary, and identify the source pane;
`app` is self-reported, not a trust label. This Task adds no OS notification or
sound trigger, so fast status changes cannot produce notification storms.
Coalesce UI/SQLite updates to at most four per second per terminal, latest state
within 250 ms under normal scheduling. Forwarding never waits for SQLite.
Persistence failures leave terminal I/O working and expose stale/unavailable
observation; they must not silently appear as a newly observed idle state.

### Writing without intercepting native output

`lf` never re-emits a provider's terminal reports. Native direct and relay paths
remain byte-transparent. Pipe-driven providers cannot describe the outer lf
operation, so their explicit harness signals feed the encoder. Enrich the
existing signal with permission/question/auth detail when actually known. Auth
blocked means a live operation awaiting login; a login failure that exits is
error. Hand-back is idle for interactive work; success/failure of a headless Exec
is done/error. Interruption/cancellation emits idle when cleanup can run. Forced
process death has no invented final report.

The Flow driver emits its own start/position/exit. Progress is completed steps /
known total only for a statically linear graph; omit it for loops or routing,
rather than fabricate a denominator. Clear the prior step record when advancing
so a Flow never leaves an old blocked child behind.

Reserve the **root for the native program**. lf writes only under
`lf/<32-hex-Exec-id>`; Flow steps use `lf/<32-hex-Flow-id>/<32-hex-step-id>`.
These fit segment, depth and total-length limits. The driver owns the Flow
record, the step owns its child record; do not also emit a standalone step root.
Use existing Flow/Exec launch context, not a protocol id supplied by terminal
text, to choose these paths. Nested agent-issued commands use their own lf
subtree and do not clear the caller. Emit `app=loopflow` each time; never issue
root clear. An external program can still clear the entire terminal; do not
intercept that valid report. Subsequent lf transitions restore only lf’s own
records. Default messages/titles are omitted; no prompts, Task titles, paths,
provider answers or credential details escape into terminal notifications.

Open `/dev/tty` only for an eligible interactive outer operation, independently
of stdout/stderr. Send each bounded report through the terminal output owner as one write when
possible. Bound its queue to one pending report; coalesce unsent replacements.
On short write, retain and finish the tail before another report or locally owned
terminal output; never discard a half-written escape sequence. On unavailable
tty or backpressure drop whole unsent reports, never delay/fail work. Exercise
partial writes in the terminal fixture. The output owner supplies readiness
handling; do not add a status-specific background service.
A captured child does not discover and write an ambient ancestor tty on its own:
its explicit launch context names the step and approved terminal sink. Suppress
new reports for `--json`, log-only execution and detached work with no terminal.
Typed activity still updates for remote readers. lf read commands, help, status
and completion commands do not emit operation reports.

Choose zero startup round trips: send well-formed reports directly on eligible
terminals, without claiming support was detected. The spec permits emission;
`Pst` permits emission without a query too. Do not infer support from a terminal
brand or add a stale global capability cache. Where the relay owns the input
stream, it may answer `OSC 7501;?` as a consumer even when detached; it must then
consume the matching downstream reply rather than send the provider two answers.
Other terminal queries remain transparent. Direct native launch adds no query,
stdin reader or response interceptor. No tmux passthrough wrapping is added:
unsupported multiplexers may suppress status but must preserve ordinary I/O.
Headless unsupported-terminal probes below are a delivery condition for this
zero-round-trip choice; a demonstrated visible corruption reopens this choice.

## Alternatives and failure analysis

Desktop-only observation cannot satisfy the no-app remote Waiting case. A new
resident status service repeats the relay's ownership and is rejected. A shared
screen emulator or title/CPU heuristics adds fragile guesses; keep the bounded
sequence recognizer. Replacing pipe-provider stream handling with OSC loses
headless requests from providers that do not emit terminal bytes, so retain the
existing stream adapters with explicit precedence. Terminal-brand gating misses
ssh/multiplexer paths and is not feature detection.

Wild success: any program can join both experiences without a Loopflow plugin;
a remote question is already Waiting when the laptop returns. Wild failure:
viewers duplicate reports, stale callbacks label a different Session, or a quiet
provider causes external notifications. Single stream ownership, generation
ordering and explicit-only emission are therefore acceptance boundaries, not
optional cleanup. The ABI and relay prerequisites remain real work; naming an
interface is not evidence either exists.

## Delete and preserve in the same change

- Replace `InputRequested(String)` and its kind-erasing exclusive assertions
  with typed explicit input intent in `harness/attention.rs`; do not install a
  second Codex/Claude/OpenCode event parser.
- Replace unconditional fallback portions of `waiting_sql` and
  `next_quiet_waiting`, including tests that infer quiet Waiting despite a
  current explicit working report. Preserve fallback behavior/tests for truly
  non-reporting generations and filtering before pagination.
- Remove any LOO-384 reporter-path transcript polling if it arrives before
  integration; preserve its non-reporter path, native conversation identity,
  usage/history readers and existing captures. None exists to delete here today.
- Replace the GhosttyKit pin/artifact/resources together; remove patch hunks only
  where upstream demonstrably supplies equivalent behavior. Keep the command
  selection/reflow, exit-code and embedded launch counterexamples.
- Preserve actual terminal bells, OSC titles, CPU stall diagnostics, released
  migrations and process custody. They solve different problems. No OSC 7501
  implementation, flag or status table was found to remove at the baseline.
- Update docs/architecture-reference.md, docs/lf.md and affected module READMEs
  to describe report precedence, passive observation and terminal-only emission.
  Remove the claim that Waiting necessarily needs an owned provider JSON stream.

Forbidden: a second attention enum in Swift with independent decisions; provider
report rewrites; status-as-exit authority; fake Sessions for shell panes; dual
relay/Desktop Session writers; cloned terminal parsers in Swift; OSC in JSON or
newly synthesized capture events; unconditional transcript/quiet overrides;
a parallel PTY holder while waiting for LOO-394.

## Internal sequence and acceptance

These are internal cuts toward the complete two-direction outcome, not permission
to close the Task early.

1. **This slice: record contract and embedded API proof.** Write spec-authored
   fixtures and the Rust parser/reducer/encoder. Replace the kind-erasing input
   signal; extend session_activity, projection, DTOs and fixtures together.
   Prove the Ghostty API/export and patch preservation on a pinned revision before
   wiring Swift. Focused command: `cargo test -p loopflow program_status` with
   assertions that fragmented blocked → working supersedes quiet inference,
   malformed input preserves prior state and a stale generation cannot write.
   This filter and the test names below are planned additions, not existing passes.
2. Wire all retained Ghostty surfaces, passive native observation and plain-shell
   detail; use committed Session attention in Task/Session consumers. Build and
   publish the immutable GhosttyKit artifact through its documented workflow
   after its patch checks; re-download and verify the checksum. Scope includes
   both SwiftPM Ghostty-enabled and Xcode fallback builds.
3. Emit pipe-work and Flow transitions at the existing owners. Prove terminal
   isolation, hierarchy, cancellation and honest linear progress. No real provider
   or credential is needed for deterministic terminal fixtures.
4. Integrate LOO-394's actual relay once its PTY ownership and observation seam
   exist: detached parsing/persistence, query ownership, input dismissal and
   reconnect snapshot, with no duplicate app writer. This cut is currently
   dependency-blocked; the whole Task remains unfinished without it. Carry the
   current Codex/shell-restoration uncertainties to LOO-394 rather than inventing
   a second solution here.

Gate's main scenario is one deterministic fixture program and Session in a
fresh store: emit a fragmented blocked child, read Waiting through the public
CLI (including paginated inventory), observe the existing monitor stream and
Swift Session model, answer and observe working, then report done and exit.
Repeat under the detached relay, restart the viewer, and confirm identical
Session/Task IDs and unchanged provider bytes. A sibling shell pane gets its
own detail without a Session row. The test also reads the resulting DTO fixture
in Rust and Swift; the literal message never becomes markup.

| Headless gate command (new tests where named) | Required result |
| --- | --- |
| `cargo test -p loopflow program_status` | Spec grammar, duplicate/absent keys, exact size/depth caps, invalid UTF-8/base64/control text, fragmented sequences, clear subtree, reset/prompt/exit/key lifetimes, 64-record eviction, generation replacement and no-heartbeat behavior. Materialized-schema SQL test covers actual Waiting selection. |
| `cargo test -p loopflow --test program_status_terminal` | Public-CLI fixture crosses PTY → SQLite → Waiting filter/watch → DTO, plus no-viewer relay/reconnect. Explicit question beats open-tool count; explicit working never becomes quiet Waiting. Report done cannot settle an Exec/Task. Native bytes pass exactly once. |
| Same terminal integration target | Flow/step/provider records coexist; linear progress only; no OSC bytes in captured stdout, stderr, JSON or synthesized stored events; `--json`, no tty, background child and backpressure remain harmless. Stub unsupported terminal, selected tmux versions and available real headless terminal engine preserve input/output. Skipped consumer probes remain explicit unproved compatibility, never a pass. |
| `uv run python scripts/loopflow-dev.py ghostty-build` | Extend its current Zig test filters to execute Program Status/lifetime/action tests as well as command, semantic-prompt and execCommand checks. New exact revision builds; exported action and fixed query reply work without a display. |
| `swift test --package-path swift --filter ProgramStatusTests` | Both shell and Session identities, copied callback payload lifetime, literal/control-safe detail, actor delivery, source routing, teardown/reused surface and stale callback rejection. Include a headless real Ghostty action fixture; fabricated callbacks alone do not prove parsing. |
| `uv run python scripts/test.py --swift` and `uv run python scripts/test.py --loopflow` | Affected headless Swift suite and fallback app/test-runner build compile. Unavailable platform checks go to capable CI; no mounted UI runner is required. |

Also run Rust formatting/Clippy and affected migration/DTO suites at gate. Reuse
identical-tree results; no broad builds for this design-only change. No installed
state mutation, release, provider launch or mounted demo is required for kickoff.

Done when a plain program's report changes its pane and associated Session's
shared Waiting state, survives viewer absence via the relay, and lf's own Flow
and headless work can be understood by a supporting terminal without corrupting
normal output or claiming authority from display text.

Check: exact-commit spec/API inspection, `lf task status LOO-394 --json`, `git diff --check` and `lf context --skill kickoff --json` checked the plan and budgets; implementation tests not run (design only).
