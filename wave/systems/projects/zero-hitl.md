# Zero HITL

An agent run never hands a setup step back to a human.

## KRs

- Avoidable human-in-the-loop steps found in agent runs fall to 0 for one
  week.
- The exec-door story completes: `lfq` is live today as the door's client
  (bin/lfq.rs; subagents escape their sandbox through it) — fold it into
  `lf` or re-charter it explicitly; coordinate with architecture's
  one-binary bet (Linear item).
- Credential expiries (Linear, GitHub, vendor tokens) surface as tasks
  before they block a run (validated 2026-07-08: Linear token expiry
  silently blocked pm close-out).
