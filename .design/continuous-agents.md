# Continuous Agent Loops

Implemented. See `.design/feature.md` for reference and `src/loopflow/maestro/runner.py` for details.

## Future Work

- **Schedule trigger**: Cron-like trigger condition (currently only `always` and `main-changed`)
- **Webhook trigger**: External webhook to trigger agent runs
- **Pause on user activity**: Detect when user is actively working and pause agents
- **Cost tracking**: Track LLM API costs per agent
