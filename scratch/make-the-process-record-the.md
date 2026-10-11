# LOO-451 — Process-owned liveness

Jack Heart requested one process-backed judgment on October 9. Main at
be4a2b2af contains #1519 and #1520. His October 10 steer `5228ab69` removes
client-file compatibility, including old Sessions, and leaves unknown unchanged.

## Design and deletion cut

AgentProcess completion and exact OS identity alone decide live/dead/unknown.
Waiting needs positive life before pagination; completed records never reconnect,
even with an answering socket. Unknown cannot authorize replacement or cleanup;
no timeout is selected. Rust/Swift and the shared fixture agree on unknown,
active and closed. No reader needs exit events for process liveness.

Delete — do not maintain:
- `ProviderClientRef`, `ProviderClientStop`, stop reasons and receipt guards;
  their JSON readers/writers, approximate command/age matching and file locks.
- Client-file Session fallback, terminal lookup and stopping in util,
  human_session and primary succession; exclusive receipt fixtures.
- Socket probes, attachment-outcome/Interrupted Session state and attachment-exit
  input completion (removed in the earlier cut).

Preserve native history, exact stop fencing, intentional Move outcomes, terminal
navigation and report provenance. Terminal placement now lives on the attached
LfProcess (`process_terminal`, one draft); old receipt bytes are untouched and
unread. Explicit native Move stops the exact recorded PID under the attachment
lock, never its shared foreground group. Headless stops retain group/descendant
judgment. Codex remote takeover fences the previous client rather than signaling
the provider; the old client may remain a passive display.

Launch audit: Claude, Codex and OpenCode headless starts use agent_process::spawn;
native starts use spawn_native, both recording identity before exec. Only explicit
remote clients bypass provider creation, under the surviving agent's attachment.
New terminal registration uses that fence; takeover invalidates old placement.

Input completion reads the latest turn's LfProcess, or capture owner before any
turn. Preparation exiting cannot finish a later live turn. Provider outcomes stay
history, not process life. Work-watch rereads Sessions, Task and Work activity on
process revisions and its two-second OS clock, without requiring store writes.

## Remaining

Gate owns affected Session/history/connection, primary succession, Work-watch and
headless Desktop suites, including materialized migration coverage. Demo owns
approval → SIGKILL → closed/non-Waiting across wave sessions, task status, monitor
and Work-watch, then fresh resume rejecting the saved dead socket. Source fixtures
are not installed acceptance. No publication or installation requested.

Review found terminal ownership and intentional Move were also receipt consumers;
they moved with the deletion rather than becoming a replacement receipt format.
Move after an already-dead provider preserves its unknown outcome rather than
claiming the request interrupted it.
Released-history migration and takeover tests retain identity, unknown refusal,
terminal placement and completion boundaries. Configured remote-client exit remains
unproved; fenced passive display is not provider death.

Checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and network-isolated `cargo test` focused native Move/launch, legacy state, publication, migration and Program Status CLI: 9 tests pass; gate/CI own remaining suites and composed acceptance.
