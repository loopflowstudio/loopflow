# Codex handoff: history and pending approval review

2026-09-28 · LOO-298 · Bounded independent source review.

Two findings: the live reconnect path has no completion/usage recorder after the
original receiver exits, and approval preservation across transfer remains
unproven. Neither finding requires terminating a surviving provider engine.

No build, test, provider, installed-Home operation or mutation outside this note
ran. Main retains executable ownership and the build slot. This is source
evidence, not a reproduced runtime failure or whole-Task review.

## Scope and stable source

HEAD was `d07e569330c8dedceb5dd238ce9b22a7b6137006`, with active implementation
edits. References below address working bytes, not that commit alone. The
teardown and public-connect fixture changed during inspection; the final paths
were reread. These SHA-256 prefixes were stable across the final inspection
window for the core files (additional reader hashes were collected at the end):

| Path | SHA-256 prefix |
| --- | --- |
| `scratch/data-model-one-table-per.md` | `96cbc642bf96f61a2f861521fc2012eccc` |
| `rust/loopflow/src/harness/codex.rs` | `cda9969311c85bcd233d1aaa6cbaad53b` |
| `rust/loopflow/src/harness/codex_connection.rs` | `a9d59427ffcb5aff515e374f9a96fb37d6` |
| `rust/loopflow/src/ops/human_session.rs` | `967e9e2025410b9c0f852eb7e65cf8321` |
| `rust/loopflow/src/engine/agent.rs` | `e127df22a2ead6d465247efbbe10df476` |
| `rust/loopflow/src/run_record.rs` | `17e74156153513cb3991b59f162bb8965` |
| `rust/loopflow/src/store/sqlite/execs.rs` | `3b24f5b028cb56a05c357781f0b61af204` |
| `rust/loopflow/src/store/sqlite/flows.rs` | `4d52ec1ab55b2535111ca9f12b9d579c6` |
| `rust/loopflow/src/store/sqlite/runs.rs` | `e854e1720d4f929dd1739b4dc48cdf20e` |
| `rust/loopflow/src/lf/commands/flow.rs` | `326fdfcf4c4e84d41a53772c9eeeddb4` |
| `rust/loopflow/tests/exec_ownership_tests.rs` | `7e0f22a90bf783083c54be65fd4cb8922` |
| `tests/e2e/codex_connect.py` | `199cabcf8ec6bb50312ff37747da21c5f` |

The governing design at lines 145–209 separates command exit from provider
completion, places outcomes and usage in AgentSession history, and requires
FlowSession to consume an exact selected successful completion. Its history
cutover is still a target requirement. Current Run symbols below identify
existing consumers; retaining them as another product object is not proposed.

The inspected handoff already has useful boundaries: SQLite serializes driver
comparison with socket dispatch (`harness/codex_connection.rs:103`);
`claim_session_driver(..., false)` preserves provider generation while changing
driver (`ops/human_session.rs:1253`, `store/sqlite/execs.rs:114`); established
conversation teardown detaches even when it still owns the driver
(`harness/codex.rs:925`). This review does not retain the superseded
check-then-kill concern as a finding.

## 1. Native continuity outlives the only completion/usage receiver

**Concrete source counterexample.** Finish the initial headless turn, reconnect
to its retained engine, then finish another turn. The new native turn can
succeed without producing corresponding Loopflow completion or usage history:

- `ops/human_session.rs:1253–1296` claims the driver, starts a relay and runs
  native resume. It creates no capture/event consumer and reads no completed
  provider turns back into history. On client exit it aborts the relay.
- `harness/codex_connection.rs:75–80` forwards upstream messages to the native
  client without recording them. Its store access fences writes; it does not
  ingest completion or usage.
- The original recorder is attached to `engine/agent.rs:1639`. It exits its
  drive loop at `TurnCompleted` (`:1656`) and calls `harness.stop()` (`:1696`).
  Detachment calls `shutdown_tasks`, which aborts that reader
  (`harness/codex.rs:925–936`, `:729–731`). The provider remains reachable.
- Normalized final usage is produced by that reader's `process_notification`
  (`harness/codex.rs:334–375`) and written through
  `run_record.rs:1888–1927`. A surviving native transcript is not an alternative
  input to this completion/usage writer on connect.

**Active-turn exit makes the mismatch more consequential.** A launch timeout
returns an execution error and detaches before native completion
(`engine/agent.rs:1684–1696`). Implicit capture maps that error to `failed`
(`:1177–1184`); a dropped unsettled capture also settles failed
(`run_record.rs:1652`). `finish` records that outcome and a provider-attempt
finish (`:1944–1967`); SQL updates only a previously null outcome
(`store/sqlite/runs.rs:348–353`). Thus driver failure can become the retained
agent outcome while the engine is still able to finish successfully. The later
native success has no ingestion path through reconnect. A forced process death
instead can leave missing completion; disappearance itself supplies no result.

**Reachable Flow consequence, not a claim that the target history API exists.**
Today's recovery reads this outcome in
`store/sqlite/flows.rs:453–491`: failed blocks, absent agent completion waits,
completed is accepted. The shared executor uses the selected completed attempt
at `lf/commands/flow.rs:668`. These are the consumers that must switch to exact
AgentSession completion evidence; successful native display alone cannot repair
their missing/incorrect evidence. An old observer receiving the original turn's
real completion is useful evidence and need not acquire conversational authority.

**Smallest discriminating proof.** Extend the existing public-connect fixture's
second turn with explicit nonzero usage and a known native turn ID. After it
finishes, query Loopflow history and usage, reconnect again, and require one
completion and one final usage receipt for that ID, retaining the initial turn.
Then run the same held-turn case with the original receiver exiting before
release: preserve command failure separately and ingest the later provider
success exactly once. For the accepted Flow cutover, bind this case to one
captured agent boundary and assert one fenced advance referencing that exact
success; a subsequent independent continuation must not satisfy another boundary.
This uses AgentSession history and FlowSession references, not another attempt
object or provider-engine termination.

**Existing proof limit.** `tests/e2e/codex_connect.py:412–465` already performs
the first counterexample's control path after `_launch_contract` has returned.
It checks native completion, provider-generation retention and nested Exec
parentage, but reads no per-turn Loopflow history or usage. `_gated` at
`:676–680` likewise checks client notifications, not persisted completion or
Flow consumption. These assertions cannot establish the missing preservation.

## 2. Outstanding approvals have a rejection fence, but no demonstrated handoff

**Missing preservation proof with a concrete source boundary.** The original
harness receives a server request and queues its approval response
(`harness/codex.rs:1286–1300`). Transfer can occur before dispatch. The fenced
writer then rejects the old generation, emits `codex_dispatch_rejected`, and
exits its writer loop (`:1139–1176`). The new relay similarly drops stale
approval replies, intentionally without a response (`codex_connection.rs:128`).
Those rejections protect authority and should remain.

However, the connect path opens a separate upstream connection and performs
`thread/resume`; it does not transfer pending server-request identity or fetch
outstanding approvals. The relay merely forwards messages its own connection
receives. No inspected code establishes whether Codex reissues a request that
was delivered to the former connection, or accepts its answer from the new one.
The original drive loop does not terminate on `codex_dispatch_rejected`
(`engine/agent.rs:1650–1654`), so a stranded request may also leave that driver
waiting. **Actual stranding is not established:** provider replay/routing
semantics could preserve it. That uncertainty is precisely the needed proof.

**Smallest discriminating proof.** With actual Codex and the synthetic upstream,
trigger one real native approval request and hold its old-client response until
after public driver transfer. Assert the old response has no effect, the current
client receives an actionable request for the same pending action, and its one
answer resumes that turn once. Disconnect the old client and retain a shared
engine sibling throughout. Persist the resulting exact completion/usage using
finding 1's checks. If native resume does not reissue the request, that is a
measured protocol limitation to handle within the existing Session connection
path; no new durable approval/attempt product is implied.

`tests/e2e/codex_connect.py:267` and `:439` select `approvalPolicy: never` in
the inspected fixture paths. The stale-write matrix at `:644–659` covers
start, steer, interrupt and name, not a server approval request crossing the
transfer. Its passing assertions would leave this boundary untested.
