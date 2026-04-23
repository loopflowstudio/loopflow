---
requires: lfq show/usage data, recent run history, algedonic history, CI status, PR status
produces: scratch/vsm-s3-scan.md
---
Scan control and health state.

## Goal

Observe how the member waves are actually operating.

Read live status, run history, retry patterns, CI state, PR stalls, and any
available token / cost usage so s3 can judge capacity from real signals.

## Workflow

1. Read `lfq show <wave> --json` for each member wave.
2. Read recent run history for throughput, completion times, and retries.
3. Read token / cost usage data when available.
4. Read algedonic history, repair chains, and escalation patterns.
5. Read CI status and blocked or stalled PRs.
6. Record what the system is doing right now.

## Output

Write `scratch/vsm-s3-scan.md`:

```markdown
# VSM S3 Scan — <date>

## Live State
<status, active runs, queue state>

## Throughput Signals
<velocity, completion times, finish-line crossings>

## Failure and Retry Signals
<error rates, repair chains, escalations>

## Cost / Usage Signals
<if available>

## CI and PR State
<blocked, stalled, failing>
```

## What to avoid

**Judgment words.** Record evidence. Assessment comes next.
