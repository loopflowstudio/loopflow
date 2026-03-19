---
linear_id: fdbe4d4a-ffe1-45b6-bd69-fb092ce851d2
---
# 02: VSM Flow

**Finish line:** `lf vsm` runs a five-system viable system audit against a chord-wave's members. s5-s2 assess and schedule; s1 executes a batch of parallel work. This is the first flow where a single wave run produces multiple PRs via subwave runs.

## Context

`tend` works — it runs live against the redesign chord. VSM is a different lens: where tend is scan → assess → maybe tune, VSM walks Beer's five systems in descending order. Each level has its own altitude and concerns.

Algedonic signals are live: repair lineage, error classification, retry limit with backoff, and escalation to the attention queue all work end-to-end. s3 can read real algedonic history and repair chain data.

The reference implementation (moskov/scaffold) maps VSM to an agent OS: S5 as kernel/privileged mode, S3 as cgroups, S2 as process scheduler, S1 as user-mode processes. S2 is coordination *between* S1 units — a neutral arbiter deciding what can safely run in parallel.

## The shape

Two phases:

**Governance (s5 → s4 → s3 → s2):** Each level assesses, updates wave plans, and optionally implements something urgent at its altitude. The output is an updated backlog per wave and a next batch — items to execute now, in parallel.

**Execution (s1 × N):** Launches the batch. Each item becomes a subwave run in its own worktree, producing its own PR. This is the first case where a wave run spawns multiple PRs.

No scheduling beyond one batch. The next VSM cycle reassesses from scratch and picks a fresh batch. Backlog plans made two cycles ago are stale — that's the whole point of cycling through s5-s2 every time.

## Governance levels

Each governance level follows the same pattern:

```
assess → update wave plans → or(implement, continue)
```

Assessment always happens. Implementation is opportunistic — when something is so urgent at this level that passing it down would be negligent.

### s5 — Identity and Policy

**Assess:**
- Is this chord still responsible for the right things?
- Does the member roster match the chord's purpose?
- Are autonomy levels appropriate given recent algedonic history?
- Has the direction drifted from intent?

**Implement (when):** The chord's boundary is wrong — a wave needs creating, archiving, merging, or splitting. Direction has drifted enough that continuing without correction would compound the error.

### s4 — Intelligence

**Assess:**
- What changed in the environment since last cycle?
- Upstream changes that affect member waves?
- New dependencies, deprecations, API changes?
- What's coming that members should prepare for?

**Implement (when):** An environmental change is urgent — a breaking dep update, a security advisory, an API sunset with a deadline.

### s3 — Control

**Assess:**
- Are members performing? Run status, velocity, error rates.
- Algedonic history — which waves needed repair? How often?
- Where to allocate attention? What's blocked, stalled, or idle?
- Resource allocation — how many items in the next batch?

**Implement (when):** A wave is blocked by something mechanical — failing CI, a stalled PR, a configuration error. Something s3 can fix directly.

s3 is the resource allocator. Its assessment determines batch size — how many parallel items this cycle can sustain.

### s2 — Coordination

**Assess:**
- Are any members working on overlapping areas?
- Are there conflicts between member PRs?
- Is work oscillating (one wave undoing another's changes)?
- Should triggers or dependencies between members change?

**Output:** The next batch. s2 takes the prioritized backlogs from s5-s4-s3 and decides what can safely run in parallel. Two items touching the same files can't be in the same batch. Two items in different waves can.

```
backlog: updated per wave (reprioritized, new items, stale items removed)
next batch: [items to execute now, in parallel]
```

**Implement (when):** Active interference between waves — conflicting PRs, duplicate work, trigger loops.

s2 simulates member wave perspectives to coordinate them. The first version reads all wave state and reasons about conflicts. Later, Letta integration adds persistent memory across coordination cycles.

## Execution

### s1 — Operations

s1 launches the next batch. Each item in the batch becomes a subwave run:
- Own worktree (sibling directory, per loopflow convention)
- Own branch and PR
- Runs through `ship-roadmap` machinery (ingest → kickoff → build → gate → land)

s1's job is to launch these runs and monitor them. When a run fails, algedonic signals flow back to s3 on the next VSM cycle.

This is the architectural novelty: a parent flow step that spawns child wave runs across multiple worktrees, each producing its own PR. The machinery for worktrees and wave runs exists — what's new is orchestrating multiple in parallel from a single step.

## Flow definition

```yaml
# flow: vsm
- vsm/s5
- vsm/s4
- vsm/s3
- vsm/s2
- vsm/s1
```

s5-s2 each write to scratch/ and update wave plans. s2 writes `scratch/vsm-batch.md` with the batch manifest. s1 reads the manifest and launches runs.

## Reference alignment

Compared against the moskov/scaffold VSM implementation:

**Same structure:** s5-s2 governance, s1 execution. S2 as neutral arbiter. S3 as resource allocator. S1 gets branch isolation per task (their ECS tasks = our worktrees). S4 scans environment, routes through project-specific filters.

**Where we're stronger:**
- Our S1 units have persistent wave context (ship-roadmap knows wave history). Theirs are stateless ECS tasks.
- Our S2 is proactive — decides the batch before launching, prevents conflicts. Theirs is reactive — mediates live resource contention.
- Our cadence is flow-based (wave mode controls it). Theirs is fixed 4-hour cycles.

**Gaps to consider later:**
- Their S3 has teeth: hard resource limits (time caps 30-180min, cost caps $2-$30), auto-pause on repeated failures, concurrency limits per project. Our s3 assesses and recommends but doesn't enforce. Resource budgets in wave config would close this.
- They have capability tiers (full/standard/read_heavy/readonly) limiting what S1 executors can do. We don't restrict tool access per wave yet.
- They have a persistent Meta S1 orchestrator. Our orchestration only happens during VSM runs, not continuously. Fine for now — continuous orchestration is a separate concern.

## Relationship to tend

`tend` is interactive: scan the territory, assess health, maybe tune. Good for a human checking in on a chord.

`vsm` is systematic: walk every governance level, assess, schedule, execute. Good for a chord's autonomous self-governance cycle.

Both are valid chord flows. Neither replaces the other.

## Done when

- Five builtin steps exist (vsm/s5, vsm/s4, vsm/s3, vsm/s2, vsm/s1)
- `lf vsm` runs them in order against a chord-wave
- s5-s2 produce updated wave backlogs and a parallel batch manifest
- s1 launches batch items as subwave runs in separate worktrees
- Each subwave run produces its own PR
- After s1, algedonic signals from failed runs are available for the next cycle
