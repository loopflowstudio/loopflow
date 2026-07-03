# Wave spend budget (first-class)

Design + technical research for roadmap item
`wave/goals/04-wave-spend-budget-first-class.md` (was `2-wave-budget.md`).

## 1. Problem & finish line

A Wave that loops 24/7 racks up open-ended agent spend. The only guardrail today
is org-level, coarse, and after-the-fact: `scripts/check_monthly_spend.py` +
`deploy/budget.json` reads the Mercury company-card feed once a month, sums
vendor charges (AWS, Fly, Claude, Codex, …), and prints `BLOCK` when
`max(actual, projected) > $100`, exiting non-zero to say "stop and get human
approval." That gate cannot see a single runaway loop, cannot stop it in real
time, and only fires after the money is already spent.

**Finish line.** A Wave carries a first-class `spend_cap`. The loop accrues real
per-iteration cost as it burns tokens. When actual-or-projected spend crosses the
cap, the wave *pauses* and a *block surfaces to the human* — the same "stop for
approval" contract as the $100 gate, but per-wave, live, and partitioning the org
ceiling into named slices.

**Naming.** "budget" stays reserved for deploy spend (`deploy/budget.json`). The
wave field is **`spend_cap`**.

## 2. The open question — resolved

> How much budget machinery is built into loopflow vs. written by users?

**Resolution (Jack's lean, made concrete): a hard floor in core, policy in
user-land.** Draw the line at *enforcement authority*:

**Core owns (the floor nobody can skip):**
- The `spend_cap` field on `Wave` — a hard ceiling.
- Real-time-ish token→cost accrual per run (the measurement).
- At-cap behavior: pause the wave + raise an algedonic block to the human,
  identical in spirit to the $100 gate. The loop cannot override this.
- Chord rollup: a child never exceeds the parent's remaining headroom.

**User-land owns (policy shaped below the ceiling), via two exposed primitives:**
- **Cost signal** — spend-to-date and headroom injected into the goal's render
  context and exposed on the wire, so a Goal prompt can read
  "you have $3.10 of $20 left today" and pace itself (drop to a cheaper model,
  batch work, defer low-value iterations).
- **Pause/block primitive** — the goal can already escalate itself through the
  attention API; a user can author a richer budgeting Goal ("slow to sonnet under
  20% headroom", "stop non-critical work after $50") on top of the signal.

**Why this line.** A 24/7 loop that can silently spend is a foot-gun; safety must
not be opt-in, so the *ceiling* is core. But a rich budgeting DSL in the runtime
is exactly the kind of policy loopflow-as-language wants users to write as goals —
so *everything below the ceiling* is user-authored. Core guarantees you can't get
burned; it does not dictate how you economize before the limit. This is the same
shape as the max-iterations safety valve (a hard core stop) versus the goal's own
decision about what to do each iteration.

Two consequences flow from this line and shape the rest of the design:
- The cap enforcement must be *impossible for the loop to route around* — so it
  lives in the daemon's `loop_ticker`, not in a step the agent runs.
- The signal must be *cheap to expose everywhere* — so cost accrues onto the
  durable run record, not held only in a transient stream.

## 3. Design

### 3.1 The type and the field

No `Money` type exists in the codebase today. Introduce one as a cents newtype to
avoid float rounding in a money context (the Python gate already treats money as
`Decimal`; cents is the wire-safe integer equivalent).

```rust
// rust/loopflow/src/lfd/types/wave.rs

/// USD, stored as integer cents. Newtype per the domain-concept rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money(pub i64); // cents

/// Hard spend ceiling for a wave. Both fields required — this is the floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpendCap {
    /// Ceiling over a rolling window (the window is `rate_window`).
    pub rate: Money,
    /// Ceiling for one pathological iteration (a single run).
    pub per_iteration: Money,
    /// Window the `rate` applies over.
    pub rate_window: SpendWindow, // Day | Month
}

pub struct Wave {
    // ...
    pub spend_cap: Option<SpendCap>, // None = no cap (unbounded, as today)
}
```

`spend_cap` is `Option` — an uncapped wave behaves exactly as today. A `SpendCap`,
once present, has no optional fields (see §4). `SpendWindow` is a small enum
(`Day`/`Month`) so `rate` is unambiguous; it mirrors the month/day split the
Python gate already reasons about.

Wire in through `Wave::new` (default `None`), the sqlite/postgres wave row
(new nullable columns `spend_cap_rate_cents`, `spend_cap_per_iter_cents`,
`spend_cap_window`), and `WaveDto`.

### 3.2 Token→cost accounting — what exists, what to build

Research findings (grounded in the tree):

- **Turn budget is not cost.** `AgentConfig.max_turns` (`engine/agent.rs:65`) is
  forwarded to the CLI as `--max-turns`; there is no `turns_used` counter and no
  dollar accounting. The item's premise holds.
- **Cost is seen but thrown away.** `StreamEvent::Result` (`engine/stream.rs:17`)
  parses `cost_usd` from Claude's `total_cost_usd` (`stream.rs:431`) and OpenCode's
  `part.cost` (`stream.rs:409`), but **only renders it to stderr** — it is never
  returned or persisted. Codex `usage` is explicitly discarded
  (`stream.rs:278: let _ = usage`). Token counts appear only in test fixtures.
- **The run result carries nothing.** `LaunchResult` (`agent.rs:48`) is
  `{ exit_code, stdout, stderr }`. The daemon's persisted agent row
  (`store/rows.rs`) has the model id but **no token or cost columns**.
- **A pricing table already exists but is dormant.** `lfd/providers.rs` has
  `CostRates { input_per_mtok, output_per_mtok, cache_read_per_mtok,
  cache_write_per_mtok }` and `lookup_cost_rates(harness, model)`
  (`providers.rs:142`) — with real rates for OpenCode models (kimi-k2, qwen3-*).
  Claude/Codex have **empty** rate tables (subscription harnesses). And
  `lookup_cost_rates` is **called nowhere** — pure dormant helper, ready to wire.

**What to build — a `RunCost` captured at run completion:**

```rust
pub struct RunCost {
    pub harness: String,           // "claude" | "codex" | "opencode"
    pub model: String,             // e.g. "opus", "kimi-k2-0711"
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost: Money,               // effective dollars, see resolution below
}
```

**Effective-dollars resolution (the load-bearing decision):**
1. If the CLI reported `cost_usd` (Claude `total_cost_usd`, OpenCode `part.cost`),
   use it directly — it is the authoritative API-metered cost.
2. Else compute `lookup_cost_rates(harness, model)` × captured tokens.
3. Else (no rate table, no reported cost — e.g. Codex today) fall back to a token
   proxy and flag it: accrue `output_tokens` against a conservative default rate
   and mark the run's cost as *estimated*. Honest note in the block message.

This means: **stop discarding Codex `usage`** at `stream.rs:278` and surface token
counts on `StreamEvent::Result`; thread the result cost back through
`LaunchResult` and onto the persisted run row (new columns
`cost_cents`, `input_tokens`, `output_tokens`, `cache_read_tokens`,
`cache_write_tokens`, `cost_estimated`). Claude gives real dollars even on a
subscription (the `total_cost_usd` is equivalent-cost), so the common case is
exact; Codex is the weak spot until a Codex rate entry is added to `providers.rs`.

### 3.3 Where cost accrues

A run burns tokens whether it succeeds or fails, so accrue on both completion
paths in `lfd/executor/wave/mod.rs`:
- `finish_completed_run` (`mod.rs:700`)
- `fail_run` (`mod.rs:923`)

Both already own the `Run` at its terminal moment. Write `RunCost` onto the run
row there. Wave spend-to-date is then a store query:
`store.sum_run_cost(wave_id, since)` where `since` is the wave's
`cycle_start_iteration` boundary for the rate window (day/month → filter by
`started_at`).

### 3.4 At-limit behavior — pause + block→human

Two mechanisms already exist and compose exactly; the **max-iterations safety
valve is the direct precedent**:

- **Pause** is `wave.set_status(WaveStatus::Paused)` + `store.update_wave`
  (`types/wave.rs:355`). Every dispatch path checks
  `wave.status() == Paused` — `loop_ticker.rs:56`, `activation.rs:273/382`,
  executor listener `mod.rs:840`; the store's `list_loopable_waves` only returns
  `paused=0`. Pausing genuinely stops iteration.
- **Block→human** is an algedonic `AttentionItem` (`types/attention.rs`,
  `AttentionKind::Algedonic`). `create_step_failure_attention`
  (`lfd/attention.rs:16`) is the pattern: build the item, `upsert_attention_item`,
  emit `Event::attention_created`. It surfaces in Concerto via `GET /attention` +
  the event stream (`AttentionItemDto`, `http/dto.rs:210`). Queue blocks are
  already stored as algedonic attention items.

Note the current asymmetry this feature closes: **max-iterations pauses without an
attention item; repair-exhaustion raises an attention item without pausing.** A
spend cap is the first stop that does *both* — pause *and* block. Add a
`create_spend_cap_attention` helper mirroring `create_step_failure_attention`,
with a spend-specific `context` (cap, spend-to-date, offending run, `estimated`
flag).

**Enforcement lives in `loop_ticker`, beside max-iterations
(`loop_ticker.rs:90-112`):**

```
// after the max_iterations valve, before enqueuing the next activation
if let Some(cap) = wave.spend_cap {
    let spent = store.sum_run_cost(wave.id(), window_start(cap.rate_window)).await;
    let projected = spent + avg_iteration_cost(wave);   // actual OR projected
    if projected >= cap.rate {
        pause_wave(&wave);                               // set_status(Paused) + update
        create_spend_cap_attention(&store, &event_hub, &wave, cap, spent);
        continue;
    }
}
```

- **`rate` (actual-or-projected)** is checked at the top of the tick, before the
  next iteration launches — mirrors the Python gate's `max(actual, projected)`.
- **`per_iteration`** is checked at run completion in `finish_completed_run`/
  `fail_run`: if the just-finished run's `cost > per_iteration`, pause + block
  immediately, before the next run can launch. This catches a pathological
  iteration at the run boundary — *not* mid-turn. Mid-run termination needs
  streaming accrual (cost surfaces only at `Result` today) and is explicit future
  work, called out so nobody assumes a mid-turn kill exists.

### 3.5 Chord rollup — parent ceiling vs. sum of children

The parent's `spend_cap.rate` is the true ceiling; children draw from its
remaining headroom. Enforcement: when checking a child in `loop_ticker`, the
effective headroom is the **min over the wave and all its ancestors**:

```
effective_headroom(wave) = min over {wave} ∪ ancestors(wave) of
                           (ancestor.cap.rate - subtree_spend(ancestor))
```

where `subtree_spend(w)` sums `sum_run_cost` over `w` and all descendants. A child
with its own generous cap still cannot cross the parent's remaining headroom.

**Dependency, called out plainly.** This needs wave ancestry, which is a *current
open regression*: the model reduction dropped `parent_wave_id` from the durable
`Wave`, so `WaveAgentTree.child_waves` is always empty (see MEMORY.md and item
`03-wave-ancestry-chord-structure.md`). The single-wave `spend_cap` (rate +
per_iteration + pause + block) ships independent of ancestry; the **chord rollup
lands only after item 03 reintroduces ancestry.** Sequence accordingly.

### 3.6 Concerto surfacing (item 05)

Spend-to-date and headroom are read-only telemetry for the per-repo looping
dashboard (`05-concerto-per-repo-looping-sessions.md`, backend-b live dashboard).
Expose them as computed fields on `WaveDto` (present only when a cap exists):
`spend_to_date`, `spend_headroom`, `spend_window`. For the shallow backend-a
(cloud launcher) they need not render. The block itself already flows to Concerto
as an `AttentionItemDto`, landing in the "queue of decisions needed" surface the
item describes.

## 4. DTO impact

`spend_cap` crosses the `lfd` HTTP boundary and is mirrored in Rust, Python, and
Swift — it is a wire type, bound by the **no-defaults DTO rule**:

- **Rust** `WaveDto` (`http/dto.rs:77`): add `pub spend_cap: Option<SpendCap>`.
  `SpendCap`'s own fields (`rate`, `per_iteration`, `rate_window`) are **all
  required** — no `#[serde(default)]`, no `Default` derive. `Money` serializes as
  a bare integer (cents). The `spend_cap` field is Optional (absent = uncapped),
  which is a real option, not a masked default. Add read-only
  `spend_to_date`/`spend_headroom`/`spend_window` as `Option` (present iff capped).
- **Python** `Wave` (`python/loopflow/models.py:97`): add
  `spend_cap: Optional[SpendCap]` and a `SpendCap` BaseModel with required
  `rate: int`, `per_iteration: int`, `rate_window: str` — no Pydantic field
  defaults on the wire model. Add the read-only spend fields as `Optional`.
- **Swift** `Wave`/`WaveDto` mirror: `spendCap: SpendCap?`, `SpendCap` with no
  init defaults; absent → `nil`, decoded as `T?`, no `?? value` fallbacks.

**Fixture obligation.** Round-trip fixtures live under `tests/fixtures/dto/`
(today: `session.json`, `create_session_request.json` — there is **no wave
fixture yet**). Add `wave.json` covering the wire shape with a populated
`spend_cap`, and add the matching round-trip test in each language (Rust +
Python + Swift), per the "adding a DTO field means adding it to the fixture and to
each language's fixture test" rule.

## 5. Build plan + test plan

**Build (single-wave cap first; chord rollup gated on item 03):**
1. `Money` newtype + `SpendCap`/`SpendWindow` in `types/wave.rs`; `spend_cap:
   Option<SpendCap>` on `Wave` + `Wave::new`; sqlite/postgres wave columns +
   migration.
2. Capture usage: extend `StreamEvent::Result` with token counts; stop discarding
   Codex `usage` (`stream.rs:278`); define `RunCost`; compute effective dollars
   (CLI `cost_usd` → `lookup_cost_rates` × tokens → estimated proxy). Thread cost
   through `LaunchResult` and onto the run row (new columns).
3. Accrual: write `RunCost` in `finish_completed_run` + `fail_run`; add
   `store.sum_run_cost(wave_id, since)`.
4. Enforce `rate`: `should_pause_for_spend_cap` beside
   `should_pause_for_max_iterations` in `loop_ticker`; on cross → pause +
   `create_spend_cap_attention`. Enforce `per_iteration` at run completion.
5. DTO: `WaveDto.spend_cap` + telemetry; Python + Swift mirrors; `wave.json`
   fixture + round-trip tests.
6. Concerto (item 05): render headroom/spend-to-date on the backend-b dashboard.
7. **Chord rollup (after item 03):** `subtree_spend` + `effective_headroom`
   min-over-ancestors check in the child's `loop_ticker` path.

**Test:**
- *Unit.* `should_pause_for_spend_cap` — below / at / projected-crossing the
  `rate`; `per_iteration` catch at completion. Effective-dollars: CLI-cost path vs
  tokens×rate path vs estimated fallback (mirrors the `providers.rs` rate tests).
  `Money`/cents arithmetic (no float drift).
- *Round-trip.* `WaveDto` with a populated `spend_cap` through the `wave.json`
  fixture in Rust, Python, and Swift; assert an absent `spend_cap` decodes to
  `None`/`nil` (no default leaks).
- *Integration — the item's Done-when.* A Wave with a small `spend_cap` runs real
  iterations, accrues real cost per run, and **pauses with an algedonic
  AttentionItem when actual-or-projected spend crosses the cap** — assert
  `wave.status() == Paused`, an `attention_created` event fired, and the block's
  context carries cap + spend-to-date. Then a **two-level chord**: root cap `$R`,
  two children; drive both until the *sum* of children's spend approaches `$R` and
  assert the next child activation is refused against the parent ceiling even
  though each child's own cap exceeds `$R`. (Chord test gated on item 03 landing.)

**Consistency check with the $100 gate.** Same contract — `max(actual,
projected)` crossing a ceiling ⇒ stop and get human approval. The wave cap is the
fine grain (live, per-wave, partitioning the org ceiling); the Mercury gate stays
the coarse monthly backstop. Neither replaces the other.

## Open dependencies / risks

- **Chord rollup blocks on wave ancestry** (item 03 — current open regression).
  Single-wave cap ships without it.
- **Codex cost is a token proxy** until a Codex rate entry is added to
  `providers.rs`; Claude (`total_cost_usd`) and OpenCode (`part.cost`) are exact.
- **`per_iteration` is a run-boundary catch, not a mid-turn kill.** Mid-run
  termination needs streaming cost accrual (cost surfaces only at `Result` today).
