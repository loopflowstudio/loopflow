# 07: Release orchestrator extraction

**Finish line:** `lf ops release` orchestrator is gone. `lf release` (step) is the single orchestrator. All ops release sub-commands are mechanical.

## What to remove from ops

- `publish_release` — the ops-layer orchestrator in `ops/release.rs`
- `diagnose_release_failure` — belongs in the `lf release` step
- `bootstrap_release` — belongs in `lf init`

## What to keep in ops

The decomposed primitives stay mechanical:

```
lf ops release-check    → exit 0 if changes, exit 1 if empty
lf ops release-bump     <version>
lf ops release-tag      <version>
lf ops release-status   → workflow status
```

`release-notes` becomes mechanical: dump raw changelog (commit list), no narrative. The `lf release` step adds narrative judgment.

## Step orchestration

The `lf release` step calls the decomposed ops primitives and adds agent judgment for notes. Cron release waves (`wave/release-patch/`, `wave/release-minor/`) work through `lf release` step.

## Cleanup

After this sprint, `OpsError::AgentFailed` should be unused in `ops/` — remove it. Verify with:

```bash
grep -r "launch_agent\|run_builtin_agent\|launch_ops_agent" rust/loopflow/src/ops/
# Should return nothing
```

## Done when

- `publish_release` deleted from `ops/release.rs`
- `diagnose_release_failure` and `bootstrap_release` removed from ops
- `release-notes` is mechanical (raw changelog only)
- `lf release` step orchestrates the full release workflow
- `OpsError::AgentFailed` removed
- Cron release waves work through `lf release` step
- Every `lf ops X` command works without network access to any LLM provider
