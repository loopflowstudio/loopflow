# Agent API status slice — review guide

## What was implemented

Wave snapshots now expose the durable `WorkStatus` derived by the shared store
instead of the separate `WavePresence` enum. Rust and Swift round-trip the same
Ready, Running, Waiting, Done, and Abandoned wire values, including every typed
Wait variant. RFC 3339 annotations make the durable timestamps one explicit
cross-language contract.

The public agent package now distinguishes an external harness acting as the
User from a Loopflow-launched worker. The former may use the User-facing
`lf chat` surface; the latter remains on its established `lf radio` channel.

## Key choices

- Reuse `WorkStatus` directly. Wave status is not wrapped in another snapshot,
  traffic-light enum, or Swift-only lifecycle.
- Keep liveness independent. A listener answering at an endpoint is evidence
  about reachability, not another Work state.
- Share Swift's `WorkReference` and `WorkBasis` across status and Launch DTOs
  instead of keeping Launch-specific copies.
- Extend the existing `skills/loopflow/SKILL.md` and `docs/agent-api.md`
  package. No second agent guide, client, or transport was added.

## How it fits together

`SharedStore::work_status(WorkRef::Wave)` is the lifecycle authority.
`WaveSnapshot` carries that value to terminal and JSON consumers, and Swift
decodes the same fixture into `WorkStatus`. UI lenses may render the value, but
they no longer derive Wave lifecycle from tmux or listener presence.

## Risks and bottlenecks

- The architecture landing creates the durable spine for registered Waves;
  authored-but-never-registered Waves still use the existing discovery repair
  path until the Wave-creation slice gives them identity at creation time.
- `paused` remains an authored execution policy, not a `WorkStatus` value. The
  eventual typed UserStart Wait must replace that policy before lifecycle
  start/stop can be fully consolidated.
- The architecture landing deliberately leaves Project/Task Session execution
  and shared-Home residency for follow-up work. This PR does not present those
  targets as shipped.
- The complete six-suite local gate passed. The separate hosted UI attempt
  built and signed the app, but macOS canceled LocalAuthentication before the
  UI test runner initialized; no UI test body ran. CI's authorized UI host
  remains the execution proof for that gate.

## What's not included

- `lf start`, shared Home residency, or remote lifecycle through `lf ssh`
- authoritative pre-Run `WorkRef -> HomeId` placement
- `lf wave create` and deletion of Swift filesystem discovery/synthetic ids
- Session-controller deletion or roadmap's remaining Session projections

Those need the next architecture slice rather than compatibility types in this
one.
