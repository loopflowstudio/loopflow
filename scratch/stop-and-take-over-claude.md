# LOO-447 — headless takeover and stop

Jack Heart requested provider-independent takeover and stop on 2026-10-09.
The Task outcome is accepted; this implementation plan is a draft (2026-10-10).
Both dependencies are integrated at `be4a2b2af`: #1519 and #1520. This meets
Jack Heart’s steer `28e0c5cc` to wait for the engine/driver strike (#1520).
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
2. Claude stop/interrupt now use the same recorded-group close as runtime
   settlement, including duplicate-owner refusal and exit recording under the
   attachment fence. Local teardown only drops the child handle. Move OpenCode stop,
   abort and failed-start cleanup under the saved attachment;
   remove `kill_on_drop(true)` only with replacement lifetime ownership in place.
   Its current stop sends HTTP abort and group signals without the attachment lock.
3. Replace optional named/anonymous lifelines with one per-AgentProcess named
   lifeline. Attach must hold it before ownership transfer commits; release must
   drop the superseded holder without killing the current attachment's provider.
   Specify attacher-first recovery before dropping the old holder: dropping it
   immediately makes the new holder’s death kill a group the launcher still
   needs. Retention without a bounded release protocol repeats the current leak.
   Remove process-global unbounded writer retention. Preserve pre-exec recording,
   closed-stdio descriptor safety and dash-compatible `kill -s TERM -- -pgid`.
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
no refusal or relay design has been accepted on Jack Heart's behalf. These are
unresolved implementation choices, not evidence that the requested outcome
needs to shrink.

## Transport cut — implementation proposal (2026-10-10)

The next cut gives Claude's existing stream reader a launcher-independent owner,
not another answer parser. A per-AgentProcess transport process owns the three
provider pipes, drains output while no client is connected, and keeps pending
request correlation with the existing native-history writer. A private Unix
socket carries reconnect/control. The saved endpoint is published before an
attachment can take over; writes validate the frozen attachment at this owner,
not just in the client. A lost client response never resubmits provider input.
Restart after transport death must preserve uncertain requests, not replay them.

Lifeline custody and write authority are separate. The launcher may remain a
non-writing standby holder after takeover so attacher-first death does not kill
its provider. Holders must release on observed group death or their own exit;
they must not accumulate dead providers in a process-global vector. A scoped
per-AgentProcess holder with group-death observation replaces `HELD_LIFELINES`.
New attachment acquires custody before claiming; failed claims release only their
own holder. Surviving standby custody does not restore a stale attachment token.
This proposal still needs both death-order and failed-claim fixtures before the
anonymous path can be deleted. No transport process is implemented in this cut.

## Related work (2026-10-10)

LOO-450’s local commit `5ce7cd5e4` routes headless Claude Flow steps through
ClaudeHarness and saves the native Session ID under the attachment fence. It is
not in this branch or local main. Reuse that turn-history path when integrated;
do not introduce another answer reader or lose schema, skill-input and correction
turn behavior in the transport change. Its recorded Flow fixture pass proves no
launcher-independent transport or takeover.

## Delete — do not maintain

Removed: Codex-only signaling and runtime live-close dispatch. The surviving
common close retains exact identity, duplicate-owner refusal and foreground
exclusion; Codex connection inspection retains unrelated-thread refusal.

Remaining:
- OpenCode’s unfenced stop/abort/drop cleanup; Claude's direct-child stop is removed.
- OpenCode’s anonymous launch lifeline.
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

The common close previously treated an already-dead leader as a dead group.
It now refuses settlement while helpers survive; absent leader identity grants no
new signal authority. The throwaway orphan-group fixture covers that boundary.
Claude's shared inventory read exposed invalid fixture trace IDs and foreground
defaults; both fixtures now save real LfProcess IDs and headless Claude metadata. No production schema change was needed.

Compression inlines the moved identity closure (now called only once) and
short-circuits duplicate-owner lookup. The identity recheck after Codex I/O stays:
the earlier runtime observation is not authority to signal a later process.

## Checks

`cargo test -p loopflow --lib --no-run` plus network-isolated test-binary filters `harness::claude::tests::`, `harness::agent_process::close_tests`, `session_record::runtime::tests` (serial): 15 passed, 2 configured-provider tests intentionally ignored; `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`: pass; public takeover and Linux acceptance remain with implementation/CI.
