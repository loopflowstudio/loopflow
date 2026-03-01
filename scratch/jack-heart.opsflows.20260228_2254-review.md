# Review: `lf rebase` fast-path step

## What was implemented

Added `fast-path: lf ops rebase` to the rebase step, making `lf rebase` skip the agent session for clean rebases. Revised the step prompt to focus on conflict resolution — the only scenario where an agent is needed.

Two files changed (kept identical): `.lf/steps/rebase.md` and `rust/loopflow/src/engine/builtins/steps/ops/rebase.md`. The wave item was moved to `scratch/04-rebase-step.md`.

## Key choices

1. **`lf ops rebase` unchanged.** Its internal agent handles the first layer of conflict recovery. The step agent is the fallback — three recovery layers total (mechanical → ops agent → step agent).

2. **Step prompt serves two contexts.** Both the ops internal agent (via `run_builtin_agent`) and the step agent (after fast-path failure) use the same markdown content. The `<lf:fast-path-failure>` tag, prepended automatically, differentiates the two scenarios.

3. **Ambiguous conflicts surface, don't guess.** Interactive runs ask the user. Headless runs write to `scratch/questions.md` and stop. The old prompt said "abort and let the human decide" for complex conflicts — the new version is more precise about the mechanism.

4. **API section added.** Documents `lf ops rebase [--onto BRANCH]` inline so the agent knows the available flags.

## How it fits together

```
lf rebase
  → fast-path: sh -c "lf ops rebase"
    → clean → push → exit 0 → done (no agent)
    → conflicts → ops internal agent → exit 0 → done
    → conflicts → ops agent fails → exit non-zero
      → step agent fires with <lf:fast-path-failure> context
```

No recursion risk: `run_builtin_agent` loads the step content as a prompt without checking fast-path. This is the same pattern used by `lf ingest`.

## Risks and bottlenecks

- **Prompt dual-use.** The same prompt content drives both the ops internal agent and the step fallback agent. If either context needs materially different instructions, the shared prompt becomes a constraint. Current content is general enough for both.
- **No golden test for rebase.** Golden prompt tests don't include a rebase case. The step embeds correctly (build succeeds, existing golden tests pass), but prompt regressions for rebase specifically wouldn't be caught by golden tests.

## What's not included

- No changes to `lf ops rebase`, the fast-path runner, or any Rust code.
- No new test cases (the change is prompt-only; existing rebase tests cover the ops layer).
- No flow changes — rebase is already referenced in the `integrate` flow.
