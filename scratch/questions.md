# Open Questions

- RunWave overrides (`flow`, `direction`, `area`) are currently persisted to `Wave` to let the executor see them. Design says wave config shouldn’t change; should we add override fields to `WaveRun` or another mechanism instead?
- Choose execution currently picks the first option deterministically (no LLM choice). Is this acceptable for the first draft, or should we wire a choice agent now?
- Fork retry/attempt tracking (`fork_attempts`) and max-retry behavior are not implemented yet. Should we add it now or defer?
- Loop/watch/cron execution uses scheduler slots only in `loop_ticker`; watch/cron runs start without slot checks. Should they acquire/release scheduler slots too?
