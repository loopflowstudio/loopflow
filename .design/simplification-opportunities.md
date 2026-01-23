# Simplification Opportunities

## Product intent

Loopflow assembles context and prompts for AI coding agents. Three tiers of execution: **Step** (single prompt run), **Flow** (chained steps), **Agent** (autonomous loop that spawns FlowRuns). The product is a CLI-first tool for engineers who want reproducible AI workflows.

---

## Opportunity 1: Naming drift between Python and Swift/TypeScript ✅ DONE

**Misalignment**: Python renamed `Job` → `FlowRun` and `Session` → `StepRun`, but Swift and TypeScript still have stale file names and partial references to old terminology.

**Changes made**:
- Split `Job.swift` → `FlowRun.swift` + `Agent.swift`
- Renamed `JobService.swift` → `AgentService.swift`
- Deleted backwards-compat aliases (`JobService`, `LoopService`)
- Split `job.ts` → `flow-run.ts` + `agent.ts`
- Deleted `loop.ts` (pure re-export adapter)
- Renamed AppState properties: `loopService` → `agentService`, `sessionWorktreeMap` → `stepRunWorktreeMap`, etc.

---

## Opportunity 2: Results panel uses `Session*` naming but domain is `StepRun*` ✅ DONE

**Misalignment**: The results panel tracks step executions but uses `Session*` terminology internally in some places.

**Changes made**:
- Renamed `SessionResult.swift` → `StepRunResult.swift`
- Updated ResultsService comment to reference "step run results"
- Renamed AppState maps: `sessionStepMap` → `stepRunStepMap`, `sessionStartMap` → `stepRunStartMap`

---

## Opportunity 3: `LoopService` alias masks the actual service hierarchy ✅ DONE

**Misalignment**: Swift had `LoopService` as an alias, but Agents aren't just loops — they can be loops, subscriptions, or schedules.

**Changes made**:
- Deleted all aliases
- Renamed service to `AgentService`
- Concerto uses "Agent" as the user-facing term

---

## Opportunity 4: Duplicated model definitions across layers ✅ DONE

**Misalignment**: `StepRun` field names differed between Python, Swift, and TypeScript (`prompt` vs `step`, `worktreePath` vs `worktree`).

**Changes made**:
- Aligned Swift `StepRun.prompt` → `StepRun.step` to match Python
- Aligned TypeScript `StepRun.worktreePath` → `StepRun.worktree` to match Python
- Added `tests/test_schema_alignment.py` to catch future drift
- Schema tests verify core fields match across all three layers

---

## Aligned areas

**Context assembly (`lf` module)**: Clean separation between gathering (`context.py`), formatting (`format_prompt`), and execution (`step.py`, `flow.py`). The `PromptComponents` dataclass is the right abstraction.

**Daemon protocol (`lfd/daemon/`)**: Request/Response/Event protocol is well-defined. Server and client share the same types. Events are namespaced (`worktree.*`, `step_run.*`, `agent.*`).

**Flow definition (`FlowDef`, `FlowStep`)**: Good 1:1 mapping between YAML config and runtime representation. `resolve_flow` cleanly handles fork/join/choose without special cases leaking elsewhere.

**Worktree model**: Consistent across all layers. `Worktree` struct in Swift matches Python's representation. Service methods are verb-first (`list`, `create`, `remove`, `sync`).
