# RLM: Recursive Language Model — Design Review

## What was implemented

Added RLM (Recursive Language Model) as a core capability for loopflow agents. RLM lets agents process inputs too large for a single context window by splitting work across sub-agents, delegating via `lf` batch invocations, and aggregating results.

Three pieces:

1. **RLM.md documentation** — bundled instructions that teach agents the examine → split → delegate → aggregate pattern. Injected into every prompt alongside LOOPFLOW.md.

2. **Environment propagation** — `RLM_DEPTH` auto-increments on each nested `lf` invocation. `RLM_MAX_DEPTH`, `RLM_MAX_PARALLEL`, and `RLM_MODEL` pass through to child processes. Config values seed the initial environment.

3. **Config fields** — `rlm_model`, `rlm_max_parallel`, `rlm_max_depth` in `.lf/config.yaml` with sensible defaults (10 parallel, depth 3).

## Key choices

- **Agent-driven, not engine-driven**: RLM is instructions for the agent, not runtime machinery in `lf`. The agent creates step files, runs `lf` in batch mode, and reads results. This avoids new execution paths and keeps the pattern composable with existing steps/flows.

- **Environment variables over CLI flags**: Depth tracking uses env vars because sub-agents are spawned as separate `lf` processes. Env vars propagate naturally through the process tree without modifying the `lf` CLI interface.

- **Always included**: RLM instructions are injected whenever `loopflow_doc` is present (which is always). This matches the philosophy that RLM is a core capability, not an opt-in feature. Token cost is ~97 lines of markdown.

- **`.lf/rlm/` for intermediate files**: Gitignored working directory for chunks and results. Keeps the repo clean.

- **`default_summary_tokens` reduced from 10000 to 5000**: Makes room for RLM instructions in the token budget. Repos with explicit config (which set 25000) are unaffected.

## How it fits together

```
Config (rlm_model, rlm_max_depth, rlm_max_parallel)
  → seed_rlm_env() sets process env vars
    → propagate_rlm_env() forwards to child Command
      → Sub-agent sees RLM_DEPTH, RLM_MAX_DEPTH, etc.
        → RLM.md instructions tell agent how to use them
```

The agent reads the RLM instructions from its prompt, decides whether to use the pattern, creates step files under `.lf/steps/rlm-*`, runs them with `lf <step> -b`, and aggregates from `.lf/rlm/results/`.

## Risks and bottlenecks

- **Token budget**: RLM.md adds ~97 lines to every prompt. For small context budgets this may crowd out other content. The trimming system would drop area docs and summaries first, so this is low risk.

- **Daemon path**: The daemon executor builds commands with `build_agent_command` and spawns via PTY — it doesn't call `launch_agent` or `propagate_rlm_env`. Daemon-spawned agents won't get RLM env vars unless separately seeded. This is acceptable for now since daemon waves don't typically use RLM patterns.

- **No depth enforcement**: `RLM_MAX_DEPTH` is advisory — the agent is told to check it, but nothing prevents spawning past the limit. This is intentional (agent autonomy) but could lead to runaway recursion if an agent ignores the instruction.

## What's not included

- **Automatic chunking**: The agent must manually split inputs. No built-in chunking logic.
- **Daemon RLM env seeding**: Daemon-launched agents don't get config-derived RLM env vars yet.
- **Depth enforcement at the engine level**: Only advisory via instructions.
- **Token budget for RLM doc**: No separate toggle to exclude RLM instructions (follows `include_loopflow_doc` pattern — always on).
