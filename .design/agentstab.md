# Agents & Pipelines

Adds DAG-based pipeline execution, cron scheduling, and emoji-tagged naming for background agents.

## Review

**Verdict:** Ready to ship

Clean implementation. Tests pass (52 new tests across 4 files). No style violations. Code follows existing patterns.

### Minor observations (not blocking)

1. **`callable` type hint** in `pipelines.py:151,197`. Using `Callable` from typing would be more precise. Noted in questions.md as deliberate simplicity tradeoff.

2. **No integration test for `execute_pipeline`**. The async execution is tested only indirectly. Low risk since it's straightforward asyncio orchestration.

3. **`parse_agent_branch` accepts any prefix as emoji**. The function at `naming.py:48-52` treats any non-"agent" prefix as an emoji. Fine for known-good branches.

## Design notes

### Two pipeline systems coexist

- New `lfd/pipelines.py` loads from `.lf/pipelines/*.yaml` for agent DAG execution
- Existing `pipeline.py` uses `config.yaml` for `lf ship`

Both work. Consolidation is future work.

### Emoji in git branches

Branch names like `🔒/security-bot/001` work on GitHub/GitLab. Design doc flags this for verification.

### Implementation decisions

Documented in `.design/questions.md`:
- Goal files: pure markdown prompts
- `.research/` structure: not enforced by code
- Pipeline coexistence: intentional, consolidate later
- DB migration: `DEFAULT ''` handles existing rows

### Future work

State sync, conflict handling, rate limiting captured in `.research/agents-future.md`.
