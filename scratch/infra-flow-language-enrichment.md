# Flow Language Enrichment (Infra Pass 3, Milestone B)

## Problem

Flows are static sequences. A push event runs the same steps as a cron poll. A fork branch runs one step and hands off to synthesize. Run decisions are ephemeral — replay re-evaluates everything from scratch.

This limits three things users need:

1. **Reactive flows.** A push to `src/` should run CI. A push to `docs/` should rebuild docs. A manual trigger should skip both. Today every activation runs the same pipeline regardless of trigger context.

2. **Substantial fork branches.** `implement → compress → lint` per direction would let forks produce higher-quality drafts before synthesis. Today each branch gets one step — shallow output, heavy synthesis burden.

3. **Deterministic replay.** Debugging a failed run means re-running and hoping the same branches fire. Decision history isn't recorded, so replay behavior can diverge from the original.

Wave goals advanced: "Invest in the flow system (the differentiators)" and "Faster reactions and richer wave composition."

## Approach

Four changes that build on each other. Each is independently testable.

### 1. Activation payload persistence

Every activation already records `from_sha`, `to_sha`, and `ActivationSource` in the audit log. Extend this with a structured payload containing everything `when` predicates need.

**New struct:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationPayload {
    pub stimulus_kind: String,          // "watch", "cron", "loop", "manual", "listen"
    pub activation_source: String,      // "poll", "push", "listen", "manual"
    pub from_sha: String,
    pub to_sha: String,
    pub changed_paths: Vec<String>,     // file paths changed in this activation
    pub source_wave_id: Option<String>, // for listen activations
}
```

**Schema change:**

```sql
ALTER TABLE activation_log ADD COLUMN payload_json TEXT;
```

**Changed paths computation:** At enqueue time, compute changed paths from the activation context:
- GitHub push webhooks: extract from `commits[].added/removed/modified` in the payload.
- Generic `/hooks/git`: compute `git diff --name-only {from_sha} {to_sha}` at ingestion.
- Poll/cron/loop with SHA range: same `git diff --name-only` at ingestion.
- Manual/listen without SHAs: empty changed_paths (predicates matching on paths won't fire — correct behavior).

Payload is JSON-serialized into `activation_log.payload_json` at creation time. The payload is also copied to `wave_runs` when a run is created from an activation, so it's always available during flow execution.

```sql
ALTER TABLE wave_runs ADD COLUMN activation_payload_json TEXT;
```

### 2. Conditional steps (`when`)

Add `when` field to `Step`. A step with `when` is evaluated against the activation payload. If the predicate doesn't match, the step is skipped — no agent launches, flow advances to the next item.

**Step struct extension:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<WhenPredicate>,
    // ... existing fields unchanged
}
```

**Three predicate types (closed set, `#[non_exhaustive]`):**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct WhenPredicate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stimulus_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_paths_any_prefix: Option<Vec<String>>,
}
```

**Evaluation rules:**
- All specified fields are AND'd. All must match for the step to run.
- `stimulus_kind`: exact match against payload's stimulus_kind.
- `activation_source`: exact match against payload's activation_source.
- `changed_paths_any_prefix`: true if ANY changed path starts with ANY specified prefix.
- Unspecified fields are ignored (always match).
- If `when` is `None`, step always runs (backwards compatible).
- If the activation payload is absent (legacy runs), all `when` predicates evaluate to true (graceful degradation).

**YAML syntax:**

```yaml
# Only run on push events
- step:
    name: fast-ci
    when:
      activation_source: push

# Only run when src/ files changed
- step:
    name: build
    when:
      changed_paths_any_prefix: ["src/", "Cargo.toml"]

# Only run on manual trigger
- step:
    name: full-release
    when:
      stimulus_kind: manual

# Combined: push events that changed docs/
- step:
    name: rebuild-docs
    when:
      activation_source: push
      changed_paths_any_prefix: ["docs/"]
```

**Flow execution change:** In `next_action()` (or the step dispatch loop in wave executor), when the current item is a step with `when`:
1. Load activation payload from the current `WaveRun`.
2. Evaluate the predicate.
3. If false: record a `when_skip` decision, emit a `FlowBranchSelected` event, advance `step_index`, continue.
4. If true: record a `when_match` decision, emit event, run the step normally.

Steps within fork branches can also have `when`. A skipped step within a fork branch doesn't fail the branch — the branch continues to its next step.

### 3. Multi-step fork branches

Extend fork branches from single-step to multi-step. Each branch becomes a sub-flow: ordered steps running sequentially in the branch's worktree.

**Type changes:**

```rust
// Before
pub struct ConcreteFork {
    pub branches: Vec<ConcreteStep>,
    pub flow_parents: Vec<String>,
}

// After
pub struct ConcreteFork {
    pub branches: Vec<ConcreteForkBranch>,
    pub flow_parents: Vec<String>,
}

pub struct ConcreteForkBranch {
    pub steps: Vec<ConcreteStep>,  // 1+ steps per branch
    pub flow_parents: Vec<String>,
    pub label: String,
}
```

**YAML syntax:**

```yaml
# Shorthand: same steps per draft, different directions
- fork:
    steps: [implement, compress]    # plural — all drafts run this sequence
    drafts:
      - direction: infra
      - direction: ux
      - direction: ceo

# Explicit: different steps per branch
- fork:
    branches:
      - steps: [implement, compress, lint]
        direction: infra
      - steps: [polish, review]
        direction: ux

# Single-step still works (backwards compatible)
- fork:
    step: reduce
    drafts:
      - direction: infra
```

**Parsing changes in `parse_fork_value`:**
- Accept `steps:` (sequence of strings) alongside existing `step:` (single string).
- When `steps:` is on the fork level with `drafts:`, each draft runs the full sequence.
- When `steps:` is on a branch in the `branches:` format, that branch runs those steps.
- `step:` remains shorthand for `steps: [step_name]`.

**Expansion changes in `expand_fork`:**
- Remove the single-step enforcement. Each branch expands to `Vec<ConcreteStep>` instead of one `ConcreteStep`.
- Nested forks within branches remain rejected.
- FlowRef branches that expand to multi-step are now allowed.

**Execution changes in fork executor:**
- Each branch's async task loops through its steps sequentially, calling `build_step_prompt` → `launch_agent` for each.
- If any step in a branch exits non-zero, the branch stops and reports failure.
- All branches still run in parallel (each in its own worktree).
- Synthesize still runs after all branches complete.

**Fork manifest extension:**

```rust
pub struct ForkManifestBranch {
    pub index: usize,
    pub steps: Vec<ForkManifestStep>,  // replaces single `step` field
    pub direction: String,
    pub worktree: String,
    pub branch: String,
    pub exit_code: i32,               // first non-zero or 0
}

pub struct ForkManifestStep {
    pub name: String,
    pub exit_code: i32,
}
```

### 4. Decision persistence and replay

Record every flow decision at evaluation time. On explicit replay, use recorded decisions instead of re-evaluating.

**Schema:**

```sql
CREATE TABLE wave_run_flow_decisions (
    run_id TEXT NOT NULL REFERENCES wave_runs(id) ON DELETE CASCADE,
    node_path TEXT NOT NULL,
    decision_type TEXT NOT NULL,      -- 'when_match', 'when_skip'
    selected_branch TEXT,             -- predicate result details
    payload_snapshot TEXT,            -- JSON: the predicate inputs used
    decided_at BIGINT NOT NULL,
    PRIMARY KEY (run_id, node_path)
);

CREATE INDEX idx_flow_decisions_run ON wave_run_flow_decisions(run_id);
```

**`node_path` format:** `step[{index}]` — stable positional identifier within the expanded flow. Example: `step[2]` for the third item in the flow.

**Recording:** When a `when` predicate is evaluated, write a row with:
- `node_path`: position in the flow
- `decision_type`: `when_match` or `when_skip`
- `payload_snapshot`: the activation payload fields the predicate matched against
- `decided_at`: timestamp

**Replay semantics:**
- **Fresh run** (new activation → new wave run): always evaluate predicates from the activation payload. Record decisions.
- **Replay** (explicit re-run of an existing wave run): load decisions from the original run. Skip predicate evaluation. If a decision record exists for a node_path, use it. If not (flow was modified since original run), evaluate fresh and record.

Replay is triggered via API: `POST /v0/waves/{wave_id}/runs/{run_id}/replay`. The new run gets a `replay_source_run_id` field linking to the original.

```sql
ALTER TABLE wave_runs ADD COLUMN replay_source_run_id TEXT REFERENCES wave_runs(id);
```

### 5. Decision observability

New WebSocket event emitted whenever a flow decision is made:

```rust
Event::FlowBranchSelected {
    wave_id: LfdId,
    wave_run_id: LfdId,
    node_path: String,
    decision_type: String,         // "when_match", "when_skip"
    step_name: String,
    is_replay: bool,
    timestamp: OffsetDateTime,
}
```

Emitted from the decision recording path — same code path for fresh evaluation and replay. The `is_replay` flag distinguishes them.

Log output at `info` level: `"flow decision: step[2] deploy → when_skip (activation_source=poll, needed push)"`.

**API:** Decisions for a run are queryable via `GET /v0/waves/{wave_id}/runs/{run_id}/decisions`, returning the `wave_run_flow_decisions` rows.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Expression engine for predicates (Lua, CEL, JSONLogic) | Full flexibility, arbitrary conditions | Explicitly out of scope. Three constrained predicates cover the real use cases (trigger type, trigger method, changed files). An expression engine adds parser complexity, security surface, and makes deterministic replay harder. Add specific predicates later via `#[non_exhaustive]`. |
| `when` as a separate `FlowItem` variant (if/then/else node) | More powerful — can wrap forks, can have else branches | Over-engineered. Step-level `when` covers the use case: "skip this step unless conditions match." Two steps with complementary predicates replaces if/else. Keeps the flow model flat. |
| Fork branches as full sub-flows (recursive `Flow` type) | Maximum flexibility — branches could contain forks, flow refs, nested when | Adds recursive complexity to the executor and fork manifest. Multi-step sequential is the 80% case. Nested forks in fork branches are an edge case we can add later if needed. |
| Compute changed paths at predicate evaluation time (lazy) | No schema change for payload, paths always fresh | Breaks deterministic replay — re-running would compute paths against current repo state, not the original activation state. Must persist at ingestion time. |
| Decision persistence as event log (append-only events) | More flexible for debugging, supports partial replays | YAGNI. A simple decision table keyed by (run_id, node_path) is sufficient. The event log pattern adds query complexity without clear benefit for the replay use case. |

## Key decisions

1. **Predicates are a closed set, not an expression language.** Three types cover the real use cases. `#[non_exhaustive]` lets us add more later without breaking existing flows. Constrained predicates are deterministic, serializable, and trivial to evaluate.

2. **`when` skips, it doesn't branch.** No else clause. If you need "if A do X, otherwise do Y," use two steps with different predicates. This keeps flow YAML flat and readable.

3. **Changed paths are computed and persisted at activation time.** This is the only approach that supports deterministic replay. GitHub push webhooks provide paths directly. Git hooks compute them via `git diff --name-only`. Capped at 10,000 paths to bound storage.

4. **Replay is explicit, not implicit.** New activations always evaluate fresh. Only `POST .../replay` uses frozen decisions. Users won't be surprised by stale behavior on new activations.

5. **Multi-step fork branches run sequentially in one worktree.** Each branch is a mini pipeline. Fail-fast on first non-zero exit. The worktree-per-branch model is unchanged — branches still run in parallel, steps within a branch run in series.

6. **Fork manifest records per-step outcomes.** The synthesize step sees which steps each branch ran and their individual results. This lets synthesis prompts reference specific step outputs.

## Scope

**In scope:**
- `WhenPredicate` struct with three predicate types
- `when:` field on `Step` struct and YAML parsing
- Predicate evaluation in flow executor with skip semantics
- `steps:` (plural) support in fork YAML parsing
- `ConcreteForkBranch` with multi-step expansion
- Sequential step execution within fork branches
- `ActivationPayload` struct and `payload_json` on activation_log
- `activation_payload_json` on wave_runs (copied at run creation)
- Changed paths computation at push/poll ingestion
- `wave_run_flow_decisions` table with recording and replay
- `replay_source_run_id` on wave_runs
- `FlowBranchSelected` WS event
- `GET .../runs/{run_id}/decisions` API endpoint
- Unit tests for predicate evaluation, multi-step fork expansion, decision replay
- Integration tests for conditional flow execution

**Out of scope:**
- Arbitrary expression engine
- Conditional fork branch selection (all branches always run)
- Nested forks within fork branches
- `when` on fork nodes (only on steps)
- UI/Concerto display of decisions (future Concerto wave)
- `POST .../replay` API endpoint (can ship separately — decisions are recorded regardless)

## Implementation plan

| Order | What | Files | Tests |
|-------|------|-------|-------|
| 1 | `ActivationPayload` struct + `payload_json` column + payload computation at ingestion | `triggers/activation.rs`, `types/stimulus.rs`, `http/routes/hooks.rs`, migration 016 | Unit: payload construction from mock webhook data |
| 2 | Copy payload to `wave_runs.activation_payload_json` at run creation | `executor/wave/mod.rs`, `store/` | Unit: payload propagation |
| 3 | `WhenPredicate` struct + `when` field on `Step` + YAML parsing | `engine/flow.rs` | Unit: YAML round-trip, predicate serde |
| 4 | Predicate evaluation + skip logic in flow executor | `executor/wave/mod.rs` | Integration: flow with when-steps, verify skip/run behavior |
| 5 | `wave_run_flow_decisions` table + decision recording | migration 016, `store/`, `executor/wave/mod.rs` | Unit: decision persistence |
| 6 | `FlowBranchSelected` WS event + decision API | `types/event.rs`, `events.rs`, `http/routes/` | Unit: event emission |
| 7 | Multi-step fork YAML parsing (`steps:` plural) | `engine/flow.rs` | Unit: YAML parsing, expansion validation |
| 8 | Multi-step fork expansion (`ConcreteForkBranch`) | `engine/flow.rs`, `engine/fork.rs` | Unit: expansion produces correct branch structures |
| 9 | Sequential step execution in fork branches | `executor/wave/fork.rs` | Integration: fork with multi-step branches |
| 10 | Replay: load decisions from source run | `executor/wave/mod.rs` | Integration: replay uses frozen decisions |

## Validation

```bash
cargo fmt --all -- --check
cargo clippy -p loopflow --all-targets -- -D warnings
cargo test -p loopflow triggers
cargo test -p loopflow flow
cargo test -p loopflow fork
tests/e2e/test_smoke.sh
```

## Done when

- `when:` predicates skip steps based on activation payload — verified by test with mock payloads for push/poll/manual.
- Fork branches with `steps: [a, b, c]` run sequentially in one worktree — verified by test with multi-step fork.
- Activation payload (including changed_paths) is persisted at ingestion and available on wave_runs.
- `wave_run_flow_decisions` records all `when` evaluations — verified by inspecting decision rows after a conditional flow run.
- `FlowBranchSelected` WS event fires on every `when` evaluation.
- `GET .../runs/{run_id}/decisions` returns recorded decisions.
- Replay loads frozen decisions instead of re-evaluating — verified by test that modifies activation context between original and replay, confirming replay uses original decisions.
- All existing flow/fork tests pass unchanged (backwards compatibility).
