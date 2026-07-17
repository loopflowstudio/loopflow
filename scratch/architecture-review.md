# Architecture foundation review

## What was implemented

- Researched the current Wave/Project/Task runtime and wrote the core cutover
  plan around `Work → Epoch → Run → Launch → optional Turn`, with Steer, Basis,
  Wait, Home authority, migration rules, races, and deletion guards made
  explicit. This branch implements foundation slices, not that full cutover.
- Added a prompt-authoring surface: `lf prompt`, the canonical `PROMPTS.md`
  guide, focused evidence methods in the skills that exercise them, and a small
  universal Evidence Loop in the standard operating prompt. The website serves
  the same guide at `/docs/prompts` rather than maintaining a second copy.
- Replaced provider-wide steering capability with a per-active-Turn
  `send_current` outcome: `Sent`, `NotSteerable`, `Failed`, or `Unknown`. Codex
  correlates `turn/steer` with the exact expected vendor Turn and its JSON-RPC
  response; Claude, OpenCode, and opaque TUIs fall back to the next seed without
  ordinary Steer interrupting execution.
- Made `agent_turns` the sole additive spend store. `run_events` now owns only
  exec/trace lineage; `usage`, `top`, `runs`, `doctor`, JSON, and Mac telemetry
  read the same Turn query. The public Turn-spend wire names Turn, Launch,
  trace, and exec and is pinned by one Rust/Swift fixture.

## Key choices

1. **Provider acceptance is transport evidence, not incorporation.** Every
   steering outcome remains available to a later seed. `Sent` improves latency
   but cannot bless the active Turn's older Basis.
2. **Dynamic outcome, not static capability.** Steerability changes by exact
   Turn kind and races its boundary. The controller asks the active Turn and
   handles the typed result.
3. **No temporary ChildCommand incorporation layer.** Crash-proof live steering
   needs the planned Steer + Basis persistence transaction. Patching the old
   command ledger would create the dual architecture this cutover is removing.
4. **One additive usage fact.** A provider measures Turns. Exec boundaries,
   raw Codex log files, and UI groupings do not get parallel totals. Missing,
   zero, and cache-only measurements remain distinguishable.
5. **Prompt doctrine rides where it is exercised.** The universal prompt pays
   only for the evidence floor; authoring doctrine lives in `PROMPTS.md` and
   `lf prompt`; research/debug/QA/portfolio methods stay in their own skills.

## How it fits together

```text
authored direction ──> controller ──> send_current(exact Turn)
       │                                  │
       └──────────── later seed <─────────┘  Sent / reject / fail / unknown

provider events ──> agent_turns ──> one Turn-spend query
                                      ├─ lf usage / lf top / lf runs / doctor
                                      └─ Mac telemetry
```

The architecture documents define the intended authority and lifecycle model.
The code in this branch clears two prerequisites: provider-neutral delivery
outcomes and one execution-usage authority. The existing Session/body runtime
remains authoritative until the structural migration can replace it in one
cutover.

## Risks and bottlenecks

- A confirmed Project/Task live Send still becomes an anonymous in-memory seed
  after its `ChildCommand` is accepted. A controller crash can lose that seed.
  This is the explicit Phase 1+3 blocker: immutable Steer plus Basis must land
  together before live steering is crash-durable or completion-fenced.
- Codex 0.144.5 uses JSON-RPC `-32600` for policy races and malformed requests
  alike. The adapter matches the two observed race messages and treats unknown
  wording as loud `Failed`; vendor prose changes degrade noisily, not silently.
- A timed-out Codex request keeps only a retired request id until a late response
  arrives or the Launch ends. This prevents a late rejection from becoming a
  new Turn failure without retaining its oneshot waiter.
- OpenCode Task/Project launches still do not report usage. The one ledger now
  exposes that gap through `lf doctor`; normalizing the two parser/producer paths
  is separate W2-289 work.
- Phase 0 is now reconciled in `scratch/architecture.md`: the workshop
  alternatives are gone and the stored constraints, transactions, races, and
  next durable-input slice are explicit. None of that core persistence exists
  yet.

## What's not included

- Work/Epoch/Basis/Home persistence or migration;
- Run/Launch containment and keeper recovery;
- durable Steer/Send rows, typed decision inputs, and completion fencing;
- reconstruction without provider transcripts;
- Wait/attention/status collapse and removal of Handoff/Review concepts;
- OpenCode usage parser normalization;
- the final Session/body/ChildCommand purge.

## Validation

- Focused Rust steering, Turn-spend, trace attribution, and top tests pass.
- Shared Rust/Swift `turn_spend.json` round-trips; focused Swift DTO and registry
  query suites pass (25 tests).
- Website suite passes: 61 passed, 3 skipped.
- Final all-suite results are recorded in the PR body after the complete gate.

The gate caught four cross-surface faults and fixed them: Mac telemetry still
decoded the deleted boundary-span JSON; `lf top` still mixed a raw Codex reader
with Turn totals; and a late timed-out Codex rejection could surface as a fresh
provider error after its waiter was released. The full Wave-resolution matrix
also invoked the real provider after resolving `project promote`; it now puts a
failing provider stub first on `PATH`, keeping the resolution test hermetic.
