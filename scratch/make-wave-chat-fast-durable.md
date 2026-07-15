# Durable Wave Chat: local-first thread

## User-visible outcome

Selecting a Wave paints its latest saved conversation without waiting for the
Wave process or SSE connection. A stopped Wave still has a readable thread;
missing or damaged history is named honestly. Repeated equivalent agent-body
failures occupy one actionable row instead of pushing authored conversation
off screen.

## End-to-end proof

Given a Wave journal containing more than the replay limit, three equivalent
failed attempts, and a later authored turn:

1. `lf chat --history --json --limit 12 -w <wave>` returns the latest 12 turns
   directly from the journal, marks the result truncated, and does not require a
   listener.
2. `WaveChatConnection.start()` installs that snapshot before discovering or
   opening the live endpoint, then merges the listener replay by stable turn id.
3. The transcript shows the authored turns and one failure notice with a count,
   latest timestamp, recovery state, and disclosed raw attempt details.

Rust command tests, the shared DTO fixture, Swift connection tests, and
transcript projection tests prove the path. `cargo test -p loopflow` and
`swift test --package-path swift` are the release checks for this slice.

## Source of truth

`.lf/journal/waves/<wave>/journal.jsonl` under the Wave's origin repository is
the only persisted conversation record. The new history response is a bounded
read-only fold over that journal. SSE remains the live continuation of the same
fold. Swift stores no second transcript and writes no cache file.

## Affected surfaces and consumers

- Rust journal read path: report complete, missing, partial, and unavailable
  reads while preserving the valid prefix of a damaged journal.
- `lf chat`: add the explicit JSON history query and limit.
- Wire contract: add a required-field history snapshot mirrored by Swift and a
  shared fixture.
- `WaveChatConnection`: load local history before network refresh, keep it
  visible through connection transitions, then upsert live frames.
- Wave Chat UI: distinguish saved-history states and render one failure roll-up
  with raw details on disclosure.
- Existing post/send and journal append behavior remain unchanged. The server
  still acknowledges a message immediately after the durable append, without
  waiting for an agent body or trace capture.

## Absent and error states

- No journal: `missing`, not an empty healthy transcript. The UI says there is
  no saved conversation and can still connect or start the Wave.
- Valid empty journal: `available` with zero turns.
- Malformed or future-version line: `partial`; valid preceding turns render and
  the UI says later history could not be read.
- Other read failure: `unavailable`; no turns are invented.
- Local query failure: the connection keeps attempting SSE and exposes the
  history failure until a live authoritative replay succeeds.
- Listener absent or reconnecting: saved turns remain visible; live phase does
  not clear them.

## Operational boundary

- Read only the journal once, keep at most the requested turn tail in the
  response, and default to the listener's 12-turn replay limit.
- Perform the CLI query off the main actor.
- Do not wait for plan, run-history, trace-capture, agent startup, or network
  work before installing the local snapshot.
- Preserve append-only journal ownership: the read path never creates,
  truncates, repairs, or locks the writer's file.

## Exclusions

- Inline Task/Project/PR reference links and popovers.
- A second transcript cache or offline message queue.
- Run History, Active Sessions, and broader Wave detail redesign.
- Changing journal format or removing raw operational evidence from audit
  storage.
- The 20-run cold-open benchmark; this slice establishes a deterministic local
  path and focused latency-safe tests before adding release telemetry.
