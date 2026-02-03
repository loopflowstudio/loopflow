# Open Questions

Questions that emerged during lfd-primary implementation. Need resolution before or during next iteration.

## Architecture

- **RunWave overrides**: `flow`, `direction`, `area` overrides currently persist to `Wave`. Design says wave config shouldn't change. Should we add override fields to `WaveRun` instead?

## Scheduler slots

- **Watch/cron bypass slots**: Loop ticker acquires scheduler slots before creating WaveRun, but watch/cron triggers start runs without slot checks. Should they also acquire/release scheduler slots?
- **Interactive resumes skip slot checks**: ConnectWave/EndAgent resumes call executor directly without acquiring scheduler slots. Should interactive resumes also acquire/release slots?

## Deferred features

- **Choose/Fork selection**: `ForkSelect::One` picks first option deterministically (no LLM). `ForkSelect::Prompt` also picks first. When should we wire a choice agent?
- **Fork retry tracking**: Design has `fork_attempts` placeholder but it's not implemented. Add when we hit transient failures in practice.
