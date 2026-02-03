# Open Questions

- RunWave overrides (`flow`, `direction`, `area`) are currently persisted to `Wave` to let the executor see them. Design says wave config shouldn’t change; should we add override fields to `WaveRun` or another mechanism instead?
- Choose execution currently picks the first option deterministically (no LLM choice). Is this acceptable for the first draft, or should we wire a choice agent now?
- Fork retry/attempt tracking (`fork_attempts`) and max-retry behavior are not implemented yet. Should we add it now or defer?
- Loop/watch/cron execution uses scheduler slots only in `loop_ticker`; watch/cron runs start without slot checks. Should they acquire/release scheduler slots too?
- ConnectWave/EndAgent resumes call executor directly without acquiring scheduler slots. Should interactive resumes also acquire/release scheduler slots?
- Nested flows: fork branches only support steps (or flow refs that expand to a single step). Is that restriction acceptable, or should fork branches allow multi-step flows?
- Nested flows: `fork/<label>` is stored in `flow_parents`, and `display_path()` suppresses duplicate step names when label matches the step. Is this the desired commit path format?
- Fork selection: `select: prompt` still deterministically chooses the first branch (no LLM). Should we wire a choice agent now?
