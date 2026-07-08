# Zero HITL

An agent run never hands a setup step back to a human.

## KRs

- Avoidable human-in-the-loop steps found in agent runs fall to 0 for one
  week.
- Privileged exec routes through the `/v0/exec` door inside `lf` (no lfq
  binary; coordinate with architecture's one-binary bet).
- Credential expiries (Linear, GitHub, vendor tokens) surface as tasks before
  they block a run.
