# Demo: AgentProcessId split (e2cbe1), 2026-11-06

Headless demo run; no person present. The agent stood in for the demo decision.
Branch HEAD `0cb8bf465`, PR #1840. Design: `scratch/e2cbe1-give-agentprocess-its.md`.

## Observed

- **History over the real store.** Branch `target/debug/lf history list --all --json`
  vs installed `lf`, same `LF_HOME`, back to back: 100 entries each, `kind`
  present on every row (76 lf / 24 agent); 97 id+kind pairs match. The 3 that
  didn't were rows written between the two runs (live machine). Per kind, the
  only key difference: lf rows drop `agent_session_id`; agent rows drop
  `caller_agent_process_id`, `caller_session_id`, `via_agent`. All of them were
  null in the released output. Nothing was added.
- **Compile-time swap.** Appending `store.lf_process(agent)` with
  `agent: &AgentProcessId` fails `cargo check`: `E0308 expected &LfProcessId,
  found &AgentProcessId`. Reverted.
- **Done-when greps.** `agent*: LfProcessId` fields: 0. `ProcessKind` in
  rust/swift: 0. `LfProcessId::new()` in `processes.rs`: only lf test fixtures.

## Not demonstrated

- `lf server describe --json` `$defs.AgentProcessId`: needs #1813 (still OPEN).
  Stays in Remaining: rebase after #1813, regenerate `server_schema.json`, run
  Swift `ServerProtocolTests`. That belongs to gate.
- Desktop `TaskWorkView` "Agent" rows: not rendered here (no display); gate.

## Decision

Advance. The changed behavior holds on real data and the type split is
enforced by the compiler; the remaining item is gated on another PR, not a
defect in this one.
