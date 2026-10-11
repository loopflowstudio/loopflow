# LOO-451 — Process-owned liveness

Jack Heart requested one process-backed judgment on October 9. Main at
be4a2b2af contains #1519 and #1520. His October 10 steer `5228ab69` removes
client-file compatibility, including old Sessions, and leaves unknown unchanged.

## Implemented boundary (reconciled 2026-10-10)

AgentProcess completion and exact OS identity alone decide live/dead/unknown.
Waiting needs positive life before pagination; completed records never reconnect,
even with an answering socket. Unknown cannot authorize replacement or cleanup;
no timeout is selected. Rust/Swift and the shared fixture agree on unknown,
active and closed. Missing process rows use the same Unknown value, not a
second optional state. No reader needs exit events for process liveness.

Deleted:
- `ProviderClientRef`, `ProviderClientStop`, stop reasons and receipt guards;
  their JSON readers/writers, approximate command/age matching and file locks.
- Client-file Session fallback, terminal lookup and stopping in util,
  human_session and primary succession; exclusive receipt fixtures.
- Socket probes, attachment-outcome/Interrupted Session state and attachment-exit
  input completion.

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
turn. The latest-turn join selects its owner once; missing turn ownership never
falls back to an ended creator. Preparation exiting cannot finish a later live turn. Provider outcomes stay
history, not process life. Native completed events and terminal outcomes still end
completed work; they do not decide whether its agent lives. Work-watch rereads Sessions, Task and Work activity on
process revisions and its two-second OS clock, without requiring store writes.

## Remaining

Source inspection at `85490d96ec` agrees with the accepted deletion. Main's
#1518 context delivery is integrated; its fixture repair does not reopen client
compatibility. The Release child memory reinforces testing the public entry path,
not inferring composed acceptance from store or projection tests.

Gate covered affected Session/history/connection, primary succession, Work-watch and
headless Desktop suites, including materialized migration coverage. Demo still owns
approval → SIGKILL → closed/non-Waiting across wave sessions, task status, monitor
and Work-watch, then fresh resume rejecting the saved dead socket. Source fixtures
are not installed acceptance. Jack Heart authorized shipping below, not installation.

Review found terminal ownership and intentional Move were also receipt consumers;
they moved with the deletion rather than becoming a replacement receipt format.
Move after an already-dead provider preserves its unknown outcome rather than
claiming the request interrupted it.
Released-history migration and takeover tests retain identity, unknown refusal,
terminal placement and completion boundaries. Configured remote-client exit remains
unproved; fenced passive display is not provider death.

Gate repaired stale Move-help DTO/Desktop fixtures, Waiting fixtures without live
process identities, historical-completion expectations and watch tests that forbade
OS-clock rereads. Monitor now tests recorded process completion separately from
legacy Session completion; the public watch fixture observes a throwaway provider's
SIGKILL without any database write. No production changes were needed. The first
lifecycle repair touched the wrong assertion; it was restored and both cases passed.

Checks: `scripts/test.py --base 5f44fd9e --reuse-passing` passed architecture, fmt/Clippy, website and 411 headless Swift tests; materialized Rust had 2,382 passes/8 stale-fixture failures, repaired via `materialize_rust_tests.py -- cargo … nextest run` (62/64, then final 2/2); final fmt/Clippy and isolated `monitor list`/`usage`/`doctor --json` passed; CI owns the full final-tree matrix.

## Delivery decision (2026-10-10)

Jack Heart approved PR #1523 and requested `ship` in the Task conversation.
Proceed through the captured code workflow's ship edge (gate, then landing).
Approval does not claim the kill/reconnect demo or installed acceptance passed.
