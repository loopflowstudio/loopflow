# Read and write Program Status (LOO-398)

Implementation and remaining work — October 7, 2026. Jack Heart selected reading
for every Desktop pane, native byte passthrough and writing only for pipe-driven
work and Flow position. Source reading is implemented; live lf emission and
detached relay observation remain unfinished. No installed acceptance or provider
adoption is claimed. Original design and preservation inventory:
`c62c19f5c:scratch/read-and-write-program-status.md` (baseline `6448e3c9e`).

## Accepted scope and delivery

Jack Heart authorized implementation without a design review stop and publication
for review in his conversation, not landing (comment `c51eda46-6f94-46a4-a176-2e174e6ea112`).
His same October 7 direction explicitly leaves LOO-394's absent relay as a named
follow-up. This supersedes the original design's relay prerequisite for this PR;
it does not establish the whole Task's outcome.

Jack Heart approved the small embedded Ghostty forwarding patch in comment
`7940002d-7311-4e26-adef-1cf78c3af16f`. Its publication uses the standing workflow
in `swift/GhosttyKitPatches/README.md`. Both existing patches remain. The parser
and behavior are authored from the [revision 0.2 spec](https://www.superlogical.com/rex/docs/build/program-status),
not another implementation's code or tests. The bridge consumes the existing
Ghostty parser API; it adds no terminal parser.

October 7 header steer `86d55faf-3262-4e88-8918-ac1bf63e89cb` is implemented as the
workspace breadcrumb bar showing the focused pane's state, blocked kind and
literal message, in addition to the pane strip. Whether Jack meant the Task page
header remains a review judgment. LOO-402 retains broader Waiting presentation.
LOO-384 stays relevant for non-reporters; no supported Claude release with complete
reporting has been verified.

## Current source

- GhosttyKit pins `a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0`. The third patch
  forwards validated, copied report payloads and prompt/reset/exit boundaries as
  an embedded action. Standalone Ghostty behavior is unchanged. The first two
  patches retain command-block selection/reflow/exit status and embedded login
  opt-out. The new upstream clipboard ABI is adopted, including explicit lengths.
- The immutable `GhosttyKit-a60e9e2-lf2.xcframework.zip` contains universal macOS
  libraries and matches the public download's SHA-256
  `c2add4ae90d1e8f3394fb19b8d6f28b76318cbb497509c7534f3f6e8a59826a9`.
  The build helper normalizes `ghostty-internal.a` to `libghostty.a` and updates
  plist paths; SwiftPM otherwise compiles Swift but fails to link every Ghostty
  symbol. Pin, shell/terminfo resources and runtime revision move together.
  The earlier lf1 artifact remains immutable and is not selected.
- Each retained Ghostty surface owns bounded records and its subscription,
  independent of mounts and focus. Callbacks copy borrowed payloads before actor
  dispatch and reject a retired surface incarnation. Whole reports replace
  records; clear removes a subtree at component boundaries. Prompt and actual
  exit drop working/blocked/idle; reset clears all; keyboard interaction removes
  done/error. No timer expires reports. Oldest-updated records are evicted at 64.
  The reducer contains no independent Session Waiting judgment.
- Plain shell panes show local records and never create Sessions. A unique active
  client receipt associates subsequent live shell reports with a Session; earlier
  shell records are not replayed. Explicit Session panes retain their initial
  reports while waiting for the first Session generation reading. Provider
  replacement resets their Session report set; driver handoff does not.
- `lf session observe-status ID --terminal MARKER --generation N` accepts bounded
  newline-delimited typed snapshots on one pipe. The initial receipt must identify
  one current native client and the requested provider generation. SQLite compares
  generation, stream and increasing sequence on every write. The pipe may deliver
  the final exit snapshot after that client dies; neither EOF nor a report closes
  the Session or proves process exit. Changed snapshots coalesce at four per second.
  App pipe writes are nonblocking, retain partial frames and use write readiness;
  association changes stop the previous stream. Closing drains for at most two
  seconds. Observation failure is visible as unavailable; it does not fabricate
  idle or restart with old records against a replacement provider.
- The existing `session_activity` table owns reports and fallback counters in one
  editable migration. Reports win within their provider generation: any blocked
  record, or idle for an interactive Session, means Waiting. Other records and
  explicit clear suppress old inference. Fresh non-reporters need fresh fallback
  observations. SQL filters before pagination and quiet deadlines use the same
  precedence. Session revision changes cover reported detail. Rust/Swift DTOs and
  fixtures include reports and provider generation without defaults.
- Session panes, Task consumers and the breadcrumb header use the committed Rust
  projection. Plain shell display uses its surface reducer. Text is literal;
  controls are rejected by validation and bidi/invisible formatting is removed
  only at display. Self-reported app names are not trust labels. No notification,
  sound, title/bell repurposing, transcript tailer or new status journal is added.
- Existing harness signals retain permission/question kind rather than erasing
  it into a request string. Automatically answered OpenCode permissions and auth
  refreshes remain non-blocking. This enrichment does not emit terminal reports.
  Native provider reports continue through the existing inherited terminal path.

## Remaining writing work: output ownership

The first implementation attempt disproved the proposed independent `/dev/tty`
sink. An isolated PTY accepted only 62 bytes of a 74-byte OSC frame. That is an
actual short write, not an assumed atomic terminal write. The existing headless
path prints provider output and errors directly; Flow children inherit ordinary
streams and their Flow ID, but have no approved status sink. There is no shared
terminal output owner to finish the tail before that ordinary output. Blocking
until the tail drains violates Jack Heart's no-downside condition, and dropping
it can leave the terminal inside an escape sequence. Interruption also lacks an
output owner that can drain the frame before exit.

The unsafe sink and unused relay decoder/reducer scaffolding were removed from
this diff. No live emitter, terminal-brand heuristic, status-specific service or
ambient tty discovery by captured children remains. This is a material unfinished
part of LOO-398, not completion of the write half.

A revised writing cut must give the existing interactive operation's output one
owner, including status and locally printed terminal bytes, with explicit sink
inheritance for Flow steps. Finish partial sequences using readiness without
stalling work; coalesce unsent replacements and safely dispose of the owner on
cancellation. If that ownership cannot meet the no-downside condition, return the
concrete alternative to review rather than emitting potentially corrupt output.
The proof must exercise the same short-write/backpressure boundary.

Preserved writing contract:

- Emit working and actual explicit unanswered question/permission/auth only;
  never export the 120-second quiet rule. Interactive hand-back is idle;
  headless success/failure is done/error. Cancellation may emit idle when cleanup
  runs; forced death invents no report. Default message/title stay absent.
- Native provider root records pass through once, unchanged. lf owns only
  `lf/<32-hex-Exec-id>` or `lf/<32-hex-Flow-id>/<32-hex-step-id>` using existing
  launch context. Clear the previous step subtree on advancement; never root
  clear or duplicate a step root. Progress is completed/known steps only for a
  statically linear graph, omitted for loops/routing.
- No OSC in JSON, captured stdout/stderr, synthesized events, logs, read commands,
  help or detached work without a terminal. Parent tty access must be explicitly
  supplied, not discovered by a captured child.
- The original zero-round-trip proposal remains unproved. Unsupported terminal
  and tmux paths must preserve ordinary I/O before enabling it. No support cache,
  query reader or passthrough wrapping was added in this PR.

## Relay follow-up

LOO-394 owns provider lifetime, transparent PTY transport and detach/attach. Its
PR #1484 currently covers naming, not a relay; its unmerged rename may require
later sync. This Task does not create a second holder or decide its unresolved
Codex engine/shell-restoration design.

When that holder exists, it alone observes reports for a relayed Session, even
without a viewer. Add the bounded spec-authored OSC recognizer at its real byte
observation seam and reuse `session_activity`, generation fencing and projection.
It must recognize BEL/ST, fragmentation, oversize resynchronization, prompt/RIS
boundaries and ignore apparent OSC inside DCS without changing passthrough bytes.
Integrate query ownership, actual input dismissal and reconnect snapshots. Disable
Desktop writes for relay-owned streams. A closed viewer must not end that stream.
No passive report grants a driver claim or process-control authority.

## Delete and preserve

Removed the kind-erasing `InputRequested(String)` shape and unconditional quiet
fallback for reporting generations. Removed unconsumed raw-stream decoder/reducer
and the unsafe proposed emission sink. The third Ghostty patch stays separable
so upstream's equivalent embedded action can replace it directly. No retired
inference implementation was found at baseline to delete; any future LOO-384
integration retains only the non-reporter path.

Preserve native conversation identity/history, released migrations, real bells,
OSC titles, CPU/stall diagnostics, provider client receipts, both existing
Ghostty patches and process custody. They have different responsibilities.
Reports cannot finish an Exec, move a Workflow, settle a Flow or complete a Task.

## Acceptance still to establish

The focused source checks cover spec validation, SQL precedence and generation
fences, a public CLI observer through Waiting inventory/pagination, DTO agreement,
real Ghostty parser-to-action payload ownership, literal Swift presentation,
stale callbacks and framework/app compilation. These segments do not constitute
a composed native program → live Ghostty surface → observer → Task view test.
That headless integration and affected full suites belong to gate/CI. The UI-host
runner and a mounted judgment are not prerequisites for implementation.

The read demo remains a deterministic emitter sending fragmented blocked/question
and a short literal message in both a shell and Session pane, then working and
done. The same Session must appear in CLI/Task Waiting and clear without identity
change. Header placement is a review judgment, not asserted approval. The future
relay proof repeats this with Desktop absent and later reconnects unchanged IDs
and provider bytes. The write demo remains a deterministic Flow progressing
working → explicit unanswered question → working → done in a supporting terminal,
or a PTY byte capture if no consumer is available.

Check: focused Rust Program Status/CLI and Swift Program Status/DTO tests, Python artifact packaging, Clippy and headless Xcode build pass; Ghostty patch checks pass (267, four platform skips), public lf2 download matches and SwiftPM links it; live emission, composed native-pane/Task proof and detached relay acceptance remain unproved.
