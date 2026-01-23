# Simplification Opportunities

## Product intent

Loopflow assembles context and prompts for AI coding agents. Three tiers of execution: **Step** (single prompt run), **Flow** (chained steps), **Agent** (autonomous loop that spawns FlowRuns). The product is a CLI-first tool for engineers who want reproducible AI workflows.

---

## Opportunity 1: Naming drift between Python and Swift/TypeScript

**Misalignment**: Python renamed `Job` → `FlowRun` and `Session` → `StepRun`, but Swift and TypeScript still have stale file names and partial references to old terminology.

**Symptom**:
- `swift/LoopflowCore/Models/Job.swift` — file named "Job" but contains `FlowRun` and `Agent`
- `swift/LoopflowCore/Services/JobService.swift` — file named "JobService" but struct is `LfdService`; has backwards-compat aliases `JobService`, `LoopService`
- `web/src/models/job.ts` — file named "job" but exports `FlowRun` and `Agent`
- `web/src/models/loop.ts` — re-exports from job.ts (adapter file for old name)
- `swift/Concerto/AppState.swift` — uses `loopService` (line 134) for what's now `LfdService`
- AppState has `sessionWorktreeMap`, `sessionStepMap`, `sessionStartMap` but events are now `step_run.*`

**Realignment**:
1. Split `Job.swift` into separate files by concept:
   - `FlowRun.swift` — `FlowRun`, `FlowRunStatus`
   - `Agent.swift` — `Agent`, `AgentStatus`, `MergeMode`
2. Rename `JobService.swift` → `AgentService.swift` (Agents are the primary entity; FlowRuns are what they spawn)
3. Delete backwards-compat aliases (`JobService`, `LoopService`) — no external callers
4. Split `job.ts` similarly:
   - `flow-run.ts` — FlowRun types and helpers
   - `agent.ts` — Agent types and helpers
5. Delete `loop.ts` (pure re-export adapter)
6. Rename AppState properties: `sessionWorktreeMap` → `stepRunWorktreeMap`, `loopService` → `agentService`

**Cascade**: Removes ~40 lines of compatibility shims. One concept per file. Grep for concepts returns consistent results.

---

## Opportunity 2: Results panel uses `Session*` naming but domain is `StepRun*`

**Misalignment**: The results panel tracks step executions but uses `Session*` terminology internally in some places, creating confusion about what's being tracked.

**Symptom**:
- `ResultsService.swift` line 8: `captureBaseline(sessionId:` but stores in `StepRunBaseline`
- AppState line 609: `sessionBaselines[event.id]` stores `StepRunBaseline`
- AppState line 611: `sessionResults[event.id]` stores `StepRunResult`
- Variable names mix: `stepRunBaselines` but event param is `event.id` from `SessionEvent`
- `SessionResult.swift` model file exists alongside `StepRunResult` in SessionResult.swift

**Realignment**:
1. Rename `ResultsService.captureBaseline(sessionId:` → `captureBaseline(stepRunId:`
2. Rename AppState maps consistently: already `stepRunBaselines`/`stepRunResults` — just fix the event handler variable names (`event.id` → `stepRunId`)
3. Audit Swift model: `SessionResult.swift` should probably be renamed to match its content (`StepRunResult`)

**Cascade**: One name for one concept. The domain model is Step → StepRun (execution). Session was an intermediate name that didn't stick.

---

## Opportunity 3: `LoopService` alias masks the actual service hierarchy

**Misalignment**: Swift has `LoopService` as an alias for `LfdService`, but the name implies it only handles loops. The service actually manages Agents (which can run loops, watches, or cron) and their FlowRuns.

**Symptom**:
- `JobService.swift` line 267-270: `typealias LoopService = LfdService`
- AppState line 134: `private let loopService = LoopService()`
- The service handles agents, flow runs — not just "loops"

**Realignment**: Delete aliases. Rename to `AgentService` — Agents are the primary entity. This matches the mental model: you create an Agent, the Agent spawns FlowRuns.

**Cascade**: Naming reflects the domain. "Agent" is a user-facing concept (CLI: `lfd agent create`).

---

## Opportunity 4: Duplicated model definitions across layers

**Misalignment**: `StepRun` is defined in both `Step.swift` (lines 72-107) and referenced from Python's `lfd/models.py`. The TypeScript layer has its own `StepRun` interface. Three implementations of the same concept.

**Symptom**:
- `swift/LoopflowCore/Models/Step.swift` lines 72-107: `StepRun` struct
- `swift/LoopflowCore/Models/SessionResult.swift`: `StepRunResult`, `StepRunBaseline`
- `web/src/models/step.ts` lines 34-42: `StepRun` interface
- Python `lfd/models.py` lines 196-249: `StepRun` class

**Realignment**: Keep Python as source of truth. Swift and TypeScript should mirror the Python schema exactly. Currently they mostly do, but field names differ (`prompt` vs `step`, `worktreePath` vs `worktree`). Audit and align.

**Cascade**: Schema changes in Python propagate cleanly. Reduces "which field name?" questions during development.

---

## Aligned areas

**Context assembly (`lf` module)**: Clean separation between gathering (`context.py`), formatting (`format_prompt`), and execution (`step.py`, `flow.py`). The `PromptComponents` dataclass is the right abstraction.

**Daemon protocol (`lfd/daemon/`)**: Request/Response/Event protocol is well-defined. Server and client share the same types. Events are namespaced (`worktree.*`, `step_run.*`, `agent.*`).

**Flow definition (`FlowDef`, `FlowStep`)**: Good 1:1 mapping between YAML config and runtime representation. `resolve_flow` cleanly handles fork/join/choose without special cases leaking elsewhere.

**Worktree model**: Consistent across all layers. `Worktree` struct in Swift matches Python's representation. Service methods are verb-first (`list`, `create`, `remove`, `sync`).
