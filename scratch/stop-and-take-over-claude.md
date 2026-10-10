# LOO-447 — headless takeover and stop

Jack Heart requested provider-independent takeover and stop on 2026-10-09.
The Task outcome is accepted; this implementation plan is a draft (2026-10-10).
Both dependencies are integrated at `be4a2b2af`: #1519 and #1520.
LOO-443's remaining item 3 was read at
`4f6ed76b2^:scratch/introduce-agentprocess-record-the-provider.md`.

## Outcome and preservation

A current attachment stops its headless AgentProcess; a superseded attachment
cannot. Takeover retains AgentProcess identity, native history and outstanding
turns in either launcher/attacher death order. Foreground terminal providers
remain outside group control. Tests signal only their own throwaway children.
No installed-store writes, configured-provider runs, or delivery are authorized
by this implementation step. Jack accepted the SIGKILL timing demonstration and
three historical orphans as out of scope.

## Approach and remaining work

1. Implemented: shared exact-identity/group termination across headless providers,
   keeping the attachment lock through runtime settlement. Codex still refuses
   servers hosting unrelated conversations; duplicate OS ownership and unknown
   evidence still refuse. Tests cover current/stale settlement and foreground
   exclusion for all three providers, plus group survival after leader exit.
2. Move OpenCode stop, abort and failed-start cleanup under the saved attachment;
   remove `kill_on_drop(true)` only with replacement lifetime ownership in place.
   Its current stop sends HTTP abort and group signals without the attachment lock.
3. Replace optional named/anonymous lifelines with one per-AgentProcess named
   lifeline. Attach must hold it before ownership transfer commits; release must
   drop the superseded holder without killing the current attachment's provider.
   Remove process-global unbounded writer retention. Preserve pre-exec recording,
   closed-stdio descriptor safety and dash-compatible group signaling.
4. Complete reconnect transport, not merely FIFO survival. OpenCode already saves
   its server URL/native Session but `start_inner` always spawns another server.
   Claude owns anonymous stdin/stdout/stderr in the launching lf, so retaining a
   watchdog FIFO alone cannot preserve communication on launcher SIGKILL. An
   attachable Claude transport needs a design covering input ownership, output
   draining and pending-turn correlation independently of that launcher. This is
   not evidence that Claude cannot support takeover at all; do not turn the
   Task's explicit-refusal allowance into a silent scope reduction.
5. Public attachment must use the same connection/claim path as harness takeover,
   with fenced native writes and no raw endpoint bypass. Prove both death orders,
   A → B → A stale-token rejection, current stop, interactive exclusion, missing
   identity, and no surviving stale lifeline. Linux CI owns platform proof.

A first stop-only slice is not completion or an independently shippable PR.
Dependent transport/lifeline work returns to design: the launcher-owned Claude
pipes invalidate a lifeline-only approach. The full acceptance above remains;
no refusal or relay design has been accepted on Jack Heart's behalf.

## Delete — do not maintain

- Codex-only group termination in `harness/codex_connection.rs`: keep its native
  unrelated-thread refusal, move signaling to the common AgentProcess owner.
- Codex-only live-close branch in `session_record/runtime.rs`: keep exact identity,
  duplicate-owner refusal and foreground exclusion for every provider.
- OpenCode's unfenced stop/abort/drop cleanup and anonymous launch lifeline.
- Claude's anonymous launch lifeline and launcher-owned-only transport, once its
  replacement preserves pending output/input and native history.
- `HELD_LIFELINES` and optional anonymous `open_lifeline` branch, replacing their
  exclusive fixtures with both-death-order proofs of the surviving mechanism.

## Review findings

Moving Codex signaling must retain its unrelated-thread refusal; the common
close path now owns signaling and leaves that refusal in connection inspection.
The group test moved with its behavior rather than retaining a Codex-only copy.
Architecture prose incorrectly claimed every harness signal was fenced; it now
names the remaining OpenCode paths instead of overstating the runtime repair.
No schema or installed data changed. Release memory's operation-entry lesson
also applies here: runtime unit tests cannot establish public attachment takeover.

## Checks

`cargo test -p loopflow --lib {session_record::runtime::tests,harness::agent_process::close_tests} -- --test-threads=1` (two network-isolated invocations): 9 passed; `cargo fmt` and `cargo clippy --all-targets -- -D warnings`: pass; gate/CI retain takeover, public-entry and Linux acceptance.
