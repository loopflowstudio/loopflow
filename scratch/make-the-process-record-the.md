# LOO-451 — Process-owned liveness

Jack Heart requested one process-backed judgment on October 9. Main at
be4a2b2af contains #1519 and #1520, including the engine/driver naming strike.

## Implemented design

AgentProcess completion and exact OS identity decide live/dead/unknown. Waiting
requires live evidence before pagination; ended records never reconnect even if a
socket answers. Unknown is displayed as unknown and retains refusal of replacement,
signaling and checkout cleanup. Elapsed time supplies no death evidence. This is
a conservative implementation choice, not acceptance of the bounded-time proposal.
No schema migration. Rust/Swift SessionState now has unknown, active and closed;
`tests/fixtures/dto/session_states.json` owns the shared variants.

## Delete — do not maintain

Removed socket connect-and-drop probing, attachment-exit-event input completion,
attachment-outcome Session state, and the derived Interrupted state. Client files
still locate terminals and support exact client controls; only conversations with
no AgentProcess use them for Session state. Activity and terminal reports describe
attention only after positive process liveness. Attachment tokens remain necessary
for handoff/write fencing; process death does not rotate them.

Input completion reads its latest recorded turn's LfProcess (the capture owner
before any turn), not a later attachment's exit. Review found preparation can exit
before another lf starts the turn; that creator cannot finish a live turn.
Provider completion and terminal events remain outcome/history evidence, not agent
liveness. No identified reader needs exit events to decide process liveness; turn events
retain identity links, not a second liveness judgment.

Review found that Work-watch ignored process changes and never periodically reread
Sessions. It now observes process revisions and uses the existing two-second OS
observation clock for Sessions, Task detail and Work activity. Client-file failures
cannot override a recorded agent's state; terminal navigation still retains receipts.

## Remaining

Gate owns affected suites and composed CLI/watch acceptance. Demo owns the real
headless approval → SIGKILL → ended/non-Waiting → fresh resume scenario in disposable
work. Tests cover killed approval/quiet/blocked agents with and without an exit save,
an ended record with an answering socket, missing input exit events, handoff fencing,
unknown replacement attention and shared state variants. These do not prove installed
acceptance. Unknown timeout remains unselected; no delivery or installation requested.

Checks: `cargo test -p loopflow --lib` with focused activity/history/connection/state/watch filters (41 tests), `swift test --package-path swift --filter sessionStatesFixture` (1), `cargo fmt` and `cargo clippy --all-targets -- -D warnings` pass; gate/CI own broader composed acceptance.
