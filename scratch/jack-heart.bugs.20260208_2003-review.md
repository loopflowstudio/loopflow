# Review: Rebase agent delegation + WaveRow cleanup

## What was implemented

Three changes across Rust and Swift:

1. **Rebase conflict resolution delegated to agent** (`rebase.rs`): Instead of parsing conflict file lists, attempting resolution, then retrying the rebase manually, the code now aborts the failed rebase and hands the entire workflow to the built-in `rebase` step. The agent gets the full step prompt (with conflict resolution strategy) and the `onto` target, and handles rebase + conflict resolution + continue as a single operation.

2. **PR draft detection fix** (`pr.rs`): Added `rename = "isDraft"` to the `GhPr` serde deserialization. The `gh` CLI returns `isDraft` (camelCase) in JSON output, but the Rust field is `is_draft` (snake_case). Without the rename, `is_draft` was always `false`, meaning draft PRs were never detected.

3. **WaveRow status indicator removal** (`WaveRow.swift`): Removed the inline status dot (●/◐/○/◷/✗) and its help text from the sidebar wave row. The status indicator property still exists on `WaveViewModel` and is used in `WaveDetailPanel`.

## Key choices

**Agent-first rebase over manual retry loop.** The old code parsed conflicts, launched an agent to fix files, then retried the rebase — a two-phase approach that duplicated rebase logic and couldn't handle cases where the agent's fixes triggered new conflicts on `git rebase --continue`. The new approach gives the agent the built-in rebase step which has a full conflict resolution strategy (understand intent, rebase, resolve, continue, verify). The agent handles the entire rebase lifecycle.

**Agent success = rebase success.** If the agent exits 0, the rebase is assumed complete. If it exits non-zero, `OpsError::AgentFailed` propagates. This is simpler than the old approach which ran a retry rebase after the agent — if the agent says it's done, trust it.

**isDraft serde rename.** The `gh pr list --json isDraft` returns `"isDraft": true/false`. Serde's default snake_case conversion doesn't apply to `--json` field selection — you need to request the field in its original camelCase form and rename on deserialization.

## How it fits together

`rebase_with_recovery` tries a fast-path rebase. If it succeeds (no conflicts), push and return. If it fails, `rebase()` aborts the in-progress rebase and returns. Then `run_rebase_agent` assembles the standard prompt context + the built-in rebase step content + the `onto` target, and launches the configured agent in auto mode. The agent owns the full rebase from there.

## Risks and bottlenecks

- **Agent reliability.** The rebase outcome now depends entirely on the agent successfully completing `git rebase`, resolving conflicts, and running `git rebase --continue`. If the agent hallucinates or fails mid-rebase, the branch could be in a dirty state. The built-in rebase step has abort/recovery instructions, but this is still riskier than deterministic code.
- **No post-agent verification.** The old code verified via a retry rebase that conflicts were actually resolved. The new code trusts the agent's exit code. A faulty agent stub (exits 0 but didn't finish) would silently succeed.

## What's not included

- No changes to the built-in rebase step prompt itself.
- The WaveRow status indicator removal doesn't change the model — `statusIndicator` remains on `WaveViewModel` for use in `WaveDetailPanel`.
