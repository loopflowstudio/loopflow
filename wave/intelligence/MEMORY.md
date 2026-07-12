# intelligence wave memory

Renamed from `memory` in the 2026-07-08 wave/project/task restructure. Intelligence
owns Context and Trace — there is no standalone Memory or Evals project. Context is
the operating contract a process receives; Trace is the monitoring layer over that
context, its launch situation, execution, and outcome.

## The ledger contract (post-057, the branch that made `run_events` an API)

- **`run_id` is the trace; `process_id` is the span.** A nested `lf` inherits
  `LF_RUN_ID` (by design) but never inherits `LF_PROCESS_ID` — it reads the
  parent's value as its `parent_process_id`, then overwrites the variable with
  its own freshly minted id. Before this, 134 run_ids carried more than one
  command (one carried nine) and `lf runs` spliced two processes into one row:
  label from the first event, cost from the last terminal row.
- **A terminal row is self-describing.** It carries the same `command` its
  `started` row carried. A reader never joins to learn what a cost bought.
- **`node ∈ {run, flow, skill}`, `event ∈ {started, completed, errored,
  escalated}`, enforced by CHECK.** Migration 054 renamed the column and not the
  values, splitting `step`/`skill` across history; the constraint is what stops
  the next half-landed rename.
- **`repo` is the absolute main-repo root**, never a basename. The old
  `.file_name()` derivation made 888 of 898 values temp roots.
- **Every usage field is cumulative to that point in the process.** A span's own
  figure is the diff against the previous boundary row in the same `process_id`;
  the terminal row is the process total. One rule, no per-field exceptions —
  cost was the exception (`=` where tokens used `+=`) and it silently
  undercounted 28 multi-skill runs.
- **`own_spend` (`lf/commands/runs.rs`) is the single home for that diff.** No
  consumer reimplements it. `boundary_spend_sums_to_the_run_total_without_double_counting`
  pins the contract: a skill boundary carries the spend, and the terminal run row
  that follows reports zero of its own rather than counting it twice.
- **057 truncated the table.** Pre-contract rows could not be attributed to a
  process, their cost was undercounted, and their repo was a basename. Carrying
  them would have forced a nullable `process_id`, a legacy branch in every
  reader, and a `lf doctor` that could never go green. `process_id` is
  `NOT NULL`; there is no legacy path anywhere. The per-repo file journals
  (`.lf/journal/runs/*/events.jsonl`) remain the durable per-repo record.
- **`SpanDto` is boundary-shaped, not process-shaped.** It carries `repo`,
  `wave`, `flow`, `skill`; `TraceSpan`'s id is composite, because one process
  contributes several boundary rows and `process_id` alone no longer identifies
  one. (This supersedes the earlier "one SpanDto per process" assumption, which
  could not coexist with `own_spend` diffing boundaries inside a process.)
- **`lf doctor` runs daily**, via `.lf/flows/telemetry-daily.yaml` wired into
  this wave's `crons:`. Six checks: continuity, vocabulary, attribution,
  identity, lineage, coverage. It exits 0 on the real ledger and must stay
  there — a monitor that is permanently red is one people learn to ignore, which
  is exactly how a 29-hour outage went unnoticed.

## The dashboard

- **Four cards, one payload each.** `lf usage --json --days 30` emits one row per
  *boundary* with the cumulative-diff rule already applied; the rows are additive
  and must reconcile with what `lf usage` prints (verified: 20,010 tokens summed
  across boundaries against 20,010 in the table). A chart that disagrees with the
  table is wrong. Cards: tokens by skill (a boundary with no skill is inline
  work, labelled, never dropped), tokens by `provider:model`, cache-hit ratio
  (`cache read / (input + cache read)` — the largest cost lever in the system),
  plus the codebase cards below.
- **Width is tokens, never wall-clock.** The first draft led with a wall-clock
  flamechart and a cost waterfall; neither answered the question anyone asks of a
  token ledger. A fast skill can be the widest thing on the page, and a cost
  chart should say so.
- **The silence ribbon became a row of `lf doctor` check dots** in the dashboard
  header. Same job — make the ledger's own trustworthiness the first thing you
  see — with no second implementation of the continuity query.
- **Movement metrics stay out** until a delivery record and `escalated` exist.
  Do not synthesize them from `runs.snapshot_pr`: 7 of 171 rows carry one, all
  from `break-test`. The dashboard's Home/Movement direction (a wave is the unit
  of investment, a landed PR the unit of output; expose the tension between
  throughput, cost, time, and intervention rather than hiding it in a synthetic
  productivity score) is unbuilt and waiting on those two records.

## `lf tokens` — what a model pays to read a repo

- Walks `git ls-files` (so `.gitignore` keeps `target/` and `node_modules/` out
  of the total) and weighs each file with the **same tiktoken `cl100k_base` the
  prompt assembler budgets with**, so the tree and the context budget speak about
  one quantity. Non-UTF-8 files are skipped, not estimated: a token count over
  bytes that are not text is a number with no meaning, and a wrong number is
  worse than a gap.
- `--days N` walks one commit per day through git plumbing with no checkout.
  Migration 058 caches line and token counts **by blob sha** — blob content is
  immutable, so the cache can never go stale — and `blob_weight` memoizes in
  process as well. A year (92 sampled commits, ~840 blobs each) walks in 1.9s
  warm against 2m07s cold.
- Measured: loopflow is 184,313 lines / 1,802,919 tokens; `rust/` is 57%. By
  extension, `.lock` is the third most expensive thing a model can read here.

## Complete local run records — persist first, optimize later

- **The scope is Jack's own machine.** This is not a multi-tenant telemetry
  service and does not need cloud-scale retention policy. If the record fits on
  the personal machine, keep it; compression and rotation can arrive when
  measured disk pressure makes them necessary.
- **Measured 2026-07-10:** Codex sessions are 2.09 GB, Claude project data is
  865 MB, and together they compress from 2.98 GB to 998 MB. The 30-day rate
  projects to roughly 16 GB/year raw or 5 GB/year compressed; even the unusually
  active last week projects to 45 GB/year raw or 15 GB/year compressed, against
  201 GiB free. Storage is not the near-term constraint.
- **Persist more now.** The durable core is the exact provider-facing prompts,
  component manifest and token weights, normalized user/assistant/tool events,
  usage, lifecycle, and artifact identities. Large records live on disk with
  pointers and summary dimensions in `run_events`, not transcript blobs in
  SQLite.
- **Vendor records are useful but not the contract.** Keep pointers to raw
  Codex/Claude sessions and degrade honestly if they disappear. Loopflow's own
  normalized record should remain. Deduplication, compression, and raw-artifact
  rotation are later optimizations, not blockers to complete capture.

## Evals retired as a project; controlled harness design parked

- On 2026-07-10 Jack retired Evals as an active project. Daily multi-repo use is
  the feedback source; the wave's weekly cadence reviews one smooth, one costly,
  and one failed or heavily steered run, then files the first Context or Trace
  failure. This is cadence, not a third project.
- The controlled-harness design below remains useful if delivery, intervention,
  complete context, and transcript evidence eventually justify reviving it as a
  measured bet. It is not current work.

- **Don't build a judge. Borrow the one that already exists.** For a commit `C`
  with parent `P`: split the diff into test files and code files, check out `P`,
  apply **only the test patch**, give the agent `C`'s title and body (never its
  diff), and grade on the exit code. The code patch never enters the worktree, so
  there is nothing to contaminate. Arms vary only the harness — same prompt,
  grader, model, base commit — and wall-clock is the runner's own timer, always:
  loopflow's overhead is the thing under test, never subtract it.
- **The validation rule is the whole design.** Not "the tests fail" but: at `P`
  with the test patch applied, *collection succeeds, the pre-existing tests pass,
  and the new tests fail*. Any other shape drops the task. manabot `c7e93d3`
  errored at import (`ModuleNotFoundError: managym._managym`, a compiled Rust ext
  pinned to cpython-3.12 in a 3.14 worktree); a naive validator accepts that
  task, every arm then fails identically, and a harness that measures nothing
  looks like it is working. The pass count is the validator, not the exit code —
  exit codes are per-runner and weaker. This is why SWE-bench ships containers.
- **Corpus, ordered by environment tax.** cadenza `server/tests/*.py` is pure
  Python and validated by hand in 0.5s (`9a3d164`: 2 failed, 11 passed) — start
  there. loopflow's harvestable tests are only `rust/loopflow/tests/*.rs`; its
  unit tests live inline in `#[cfg(test)]` blocks and cannot be split from their
  code at file level. manabot needs a native build pinned to the right Python ABI
  at the parent commit. hootro is unknown and dormant.
- **Slice 1 is the harvester, not the runner.** The corpus is the asset and it
  costs zero LLM spend.
- **Both arms land in `run_events`** tagged `wave = eval/<task>`, `flow = <arm>`.
  A loopflow arm journals itself; a bare vendor arm is parsed from its own
  `stream-json` by `engine::stream::StreamParser` and written by the runner.
  Evidence has one home and `lf trace <run-id>` explains either arm. No results
  server, ever.

## Decisions (memory model)

- **The fold lives in the mind, not in code.** Agents subscribe to the memory
  stream and consolidate into their own opaque working memory. There is no
  external consolidator process — the unit of work is a mind, and the mind holds
  the memory.
- **`MEMORY.md` is a checkpoint of a mind's compiled state; the stream is the
  delta since.** A new agent = load `MEMORY.md` + subscribe to the stream (or
  replay it) → re-fold in its own context.
- **`add` publishes a full fact to a replayable stream** (shipped, slice 1):
  journals `MemoryAdded { fact }`, pushes to a replay buffer, broadcasts the
  full fact on its own channel. `add` does not write `MEMORY.md`; the file stays
  compiled instead of becoming an accreting pile of raw bullets.
- **The replay buffer is adds-since-last-externalization, not adds-since-boot.**
  Load-bearing: it makes a fresh subscriber's seed exactly `MEMORY.md` (compiled
  checkpoint) + the stream (uncompiled delta), no overlap, no double-count.
  `append_memory` pushes; `update_memory` clears. The journal fold applies the
  same accumulate-on-`MemoryAdded` / clear-on-`MemoryUpdated` logic, so a server
  restart rebuilds the buffer deterministically from disk.
- **memory-add earns its own broadcast and SSE event name.** `memory-add`
  (full facts, replay-then-live) sits beside `memory` (curation summaries,
  live-only). The `memory` frame stays byte-stable — additive, not a wire break.
- **Only the wave's mind externalizes `MEMORY.md`.** Workers only `add`. Work
  lines have no memory, so there's no last-writer-wins clobber.
- **Externalization is forced at context-compaction and at land** — the two
  moments an in-head fold would otherwise be lost.
- **Letta: learn-from only.** Reimplement blocks + fold. No backend, no server
  dependency, no vector store. Letta's "sleep-time agent" is, in loopflow terms,
  a dispatched TUI session running the fold.
- **Bounded, not unbounded.** Memory is context-sized; `MEMORY.md` is the whole
  compiled form. No archive, no retrieval.

## Decisions (usage evidence)

- **`run_events` is the one home for token and cost evidence.** `run_token_usage`
  was a second table no production code ever wrote to — its only callers lived in
  `#[cfg(test)] mod tests`, so `lf usage` aggregated an always-empty table and
  printed "No token usage recorded yet." forever. Dropped.
- **Wiring `run_token_usage` would have silently lost tokens.** Its `run_id TEXT
  PRIMARY KEY` + `ON CONFLICT DO UPDATE SET input_tokens = excluded.…` overwrote
  rather than accumulated, and a `run_id` is shared by a run and every nested
  `lf` it spawns. Last writer would have won.
- **Aggregate only terminal run rows.** Skill-boundary rows carry a cumulative
  snapshot of the run so far, so `SUM()` over every row double-counts once per
  skill.
- **`lf usage` reads the ledger directly**, like `lf runs` and `lf trace`. It
  used to fetch `GET /v0/usage` from a running `lfd` — the sole consumer of the
  whole `lfd::client` module, which died with it.

## Constraints

- **A silent best-effort write is worse than the bug it hides.** `ledger_insert`
  degraded failures to `debug!`, so when the `step_index` drift broke
  `insert_run_event`, every ledger write on the machine vanished for 29.2 hours
  (2026-07-08 14:59 UTC → 2026-07-09 20:12 UTC) across every repo — while the
  readers failed loudly on every invocation. manabot ran `lf` four times during
  the outage; its `.lf/journal` has the runs, the ledger has none. The first
  ledger failure per process now logs at `warn!`. Read "the wave never ran there"
  as a hypothesis, not a fact: check `.lf/journal` before concluding
  non-adoption, because a command that writes no prompt log still journals.
- **Fresh-db tests cannot see ledger drift.** Every migration test builds a db
  from 001 forward, so a schema that only exists on a long-lived machine is
  invisible to CI. That is why CI stayed green while the only machine holding
  real history was broken. `TESTING.md` now names the drifted-db fixture as the
  guard. Trace work must be exercised against a real ledger copy, not a fresh db.
- **A migration's version id is its identity, not its content.** Editing an
  already-applied migration never re-runs it; the divergence becomes permanent
  and silent. Repair forward with a new migration listed in
  `RENAME_CONVERGENCE_MIGRATIONS`, which tolerates "no such column" so the same
  file is a no-op on ledgers that never diverged. (057 is deliberately *not*
  listed: it must not silently skip.)
- **Run the reader before trusting it.** The first continuity check measured only
  gap-days and pronounced the real ledger healthy — the 29.2-hour outage began
  and ended mid-day, so both days held rows. Longest-silence catches it. A
  surface nobody has queried on real data is a surface that does not work.
- **A shell launched by an `lf` run inherits `LF_RUN_ID`**, so every `lf` invoked
  from it joins that trace as a child span. That is the design. It also means a
  demo run from such a shell will not mint a fresh trace: prefix
  `env -u LF_RUN_ID` when you want a root.
- **Only `MEMORY.md` crosses the branch boundary.** The journal (and the
  `MemoryAdded` replay buffer it rebuilds) is per-machine and gitignored, so the
  stream replays only *within a server's life*.
- **The server holds the pen.** Only the live wave server writes `MEMORY.md`,
  under a lock, journaled + broadcast. Offline waves edit the file directly.
- **We cannot read a running agent's internal memory representation.** The only
  way to get "whole compiled memory" out is a mind externalizing via `update`.
- **Compaction is owned by the vendor CLI.** Externalize-at-compaction assumes
  loopflow can act *before* Claude Code compacts. Unproven — the least-certain
  mechanism in the design. Fallback: land-externalization + periodic updates.

## Running the Mac dashboard

`RegistryQueryLocal` shells out to `lf runs/trace/doctor/usage/tokens --json`, so
the app must resolve **this branch's** `lf` — it prefers the one in its own
bundle over `PATH`. `uv run python scripts/loopflow-dev.py run-debug` builds
`lf`, `lfd`, and the app, installs `~/Applications/Loopflow Dev.app`, and
launches it. Three ways this has actually gone wrong:

1. A *different* app — `/Applications/Loopflow.app` predates the dashboard.
2. `swift build` names the product `LoopflowMac` but `Info.plist` declares
   `CFBundleExecutable = Loopflow`, so a copy under the build's own name left
   macOS launching the stale binary — and codesigning bumped its mtime, so it
   looked freshly built. Fixed; the installer reads the plist.
3. `swift run LoopflowMac` produces a bare executable and `NotificationService`
   dies on `bundleProxyForCurrentProcess is nil`. The app needs a real bundle.

Stale-binary check: `strings "$D/Loopflow" | grep -c "Tokens by skill"`.

## Open work (not yet in Linear — the token is expired)

`lf pm show --wave intelligence` fails with *"Stored linear token has expired.
Run `lf auth linear` again."* This pass could not read or file tasks, so the
wave's next loop must run `lf auth linear` and file these before anything else:

- **A PR delivery record** (run/wave → PR number → merge event). Blocks every PR
  metric: landed PRs by wave, cost per landed PR, lead time. Not telemetry work.
- **An `escalated` event that fires.** `LfEventType::Escalated` is defined and
  emitted zero times, so human-intervention counts and first-pass completion have
  no source. Needs an event, not a query.
- **Steering verbs for `lf wavechat`.** `/status` is the only non-speech verb, so
  a bad run can be watched and talked to but not stopped. The wave-chat KR
  promises interruption: add `/pause`, `/resume`, `/interrupt` against the wave's
  doors. Keep the rule — a slash command is a steering verb, everything else is
  speech.
- **Is hootro meant to be driven through loopflow, or left as the control arm?**
  It is genuinely dormant (no `.lf/journal`, nothing since 2026-06-19). Both
  answers are defensible.

## Someday (explicitly not now — Jack: "maybe someday")

- **The controlled eval harvester** (cadenza first). The design above remains
  parked until Trace has delivery, intervention, complete context, and transcript
  evidence and the wave deliberately reopens Evals as a project.

- **Memory export is a reader-optimized summary, not context compaction.** Claude's
  context compaction is writer-optimized (preserve working state so the *same*
  mind continues). Memory export is the opposite: the producer already knows
  everything, so the artifact is for a *cold reader* (next session, parent reading
  a child's MEMORY.md, fresh worker). Quality bar: "can someone with none of my
  context act correctly from this?" — not "can I resume?" Drop the narrative,
  keep the durable conclusion. An export that reads like a session log is a bad
  export.
- **A compaction *tool* (not `lf memory update`).** Given the current MEMORY.md +
  the add-delta, *suggest* a compacted MEMORY.md the mind reviews and applies.
  Route → fold-per-block → assemble. NOT being built now.

## Glossary

- **trace** — a `run_id`. The whole tree, stable across nested `lf`.
- **span** — a `process_id`. One process, minted once, never inherited.
- **boundary** — one usage-bearing row inside a span (a skill frame, or the
  terminal run row). Readings are cumulative; `own_spend` diffs them.
- **add** — publish an immutable fact to the append stream. `lf memory add`.
- **stream** — the ordered, broadcast log of adds. Subscribed via `lf sub`.
- **fold** — a mind consolidating incoming facts into its working memory.
- **externalize** — a mind writing its compiled state to `MEMORY.md` via
  `lf memory update`. The only checkpoint operation.
- **block** — a typed, budgeted region of the compiled `MEMORY.md`.
- **checkpoint** — a committed `MEMORY.md`; the seed the next agent inherits.

## Code map (current state)

Memory is Rust-only under `rust/loopflow/src/`. `Memory { path }`
(`wave/memory.rs`); routes (`server.rs` `/memory`, `/events`); CLI
(`lf/commands/memory.rs`); injection via `wave_memory_section` →
`<lf:wave-memory>` (`engine/flow.rs`).

Telemetry: writer in `journal/mod.rs` (`RunContext`, `ledger_insert`,
`LF_PROCESS_ID_ENV`); storage in `lfdb/` (migrations 055–058); readers in
`lf/commands/{runs,usage,tokens,doctor}.rs`, with `own_spend` in `runs.rs`;
Swift consumer in `swift/Loopflow/Services/RegistryQuery.swift` and
`swift/LoopflowMac/Views/TelemetryDashboardView.swift`.

Still greenfield: typed memory blocks (slice 3); forced externalization at
land/compaction (slice 4). No cross-machine/branch replay — that boundary is
`MEMORY.md`'s; the journal is per-machine and gitignored.
