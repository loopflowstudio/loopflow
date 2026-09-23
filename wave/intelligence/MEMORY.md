# intelligence wave memory

Renamed from `memory` in July 2026. The accepted September chapter combines
Trace & Context in one measured bet: human-selected work is explainable from
durable local evidence. No standalone Memory, Evals, or Runtime Monitoring
Project is implied by the historical research.

## Current evidence boundary (2026-09-23)

The [accepted chapter](../../.lf/chapters/20260923T000959Z-502f011b/start.md)
requires exact authored/submitted context, causal identities, an honest attempted-
operation population, and usable reconstruction against long-lived records.
Intelligence owns evidence; Infrastructure owns execution repair; Product judges
external user value. The broad replay and monitoring proposals in the summer
research were not all accepted commitments. Current Task directives govern scope.

- **Run records are Home-local bundles.** Immutable manifests own launch
  identity, parentage and Work attribution; normalized event streams own prose;
  terminal receipts own settlement. `lf runs` and `lf usage` scan these records,
  without an authoritative Run index or an `lfd` read dependency. Historical
  SQLite-ledger contracts below explain past failures, not current Run ownership.
- **Trace joins never transfer authority.** Follow intent/Work → Home/runtime
  artifact → Run/context → provider attempt/native Session → observed or owned
  process → terminal result → Work consequence/delivery. Each edge needs a source,
  identity, observation time, freshness and missingness. Unterminated is not live,
  visible PID is not control authority, unavailable Home is not stopped, and
  process success is not delivered progress. Include failures before Run creation.
- **Exact conclusions need completion receipts.** Codex deltas once lost the
  completed message's final phase. Preserve that normalized completion while
  the chat fold suppresses repeated prose. `lf runs <id> --final` prefers tagged
  final text, then an untagged completion, then explicitly labeled streamed prose.
  Preserve all three evidence qualities; old narration cannot become an exact
  report just because a reader wants one. Parentage and final text alone do not
  establish successful child settlement.
- **The denominator must match the question.** Exact direct-child scans are
  uncapped; recent Run lists are presentation windows; activity includes starts
  or completions inside its window. The shared Work filter serves CLI, Wave
  status, activity and behavioral tests. A fixture-only copy is not production
  reader proof.
- **Historical coverage is incomplete even when current enumeration is full.**
  The summer baseline lacks a start snapshot and KR revision dates. The review
  judged 39 KRs; the later start freeze contains 42 with List's three additional
  claims. Recompute totals from exact rows, exclude definition verdicts, and
  preserve rewritten claims. An incomplete duration window is unknown unless
  dated counterevidence already disproves its universal/conjunctive claim.
- **Scope operational evidence explicitly.** The research observed a 200-row
  activity cap and same-name Intelligence results from another repository.
  Restrict attribution to stable repository/Work identity; zero returned delivery
  receipts do not disprove a Linear-asserted merge. Empty, truncated, unavailable,
  and stale are different evidence states.
- **Contracts moved faster than operating proof.** The September baseline found
  settled records missing context, launch contracts, tokens or cost, plus no
  unattended replay cohort. A 20/20 context audit did not satisfy the separate
  budget/duration obligations. These are dated baseline observations, not current
  health claims. LOO-288/289/290 own capture, population, and usable reconstruction;
  retain observations while the serial implementation slot advances.

## Lessons from the retired SQLite ledger

The [pre-chapter memory](../../.lf/chapters/20260923T000959Z-502f011b/sources/wave/intelligence/MEMORY.md)
retains exact migration, schema, dashboard and historical code-map details.
Do not restore those paths to satisfy a present evidence gap.

- One identity cannot stand for multiple processes: 134 old run ids carried
  several commands, mixing labels and cost. Terminal evidence must describe what
  it measured; canonical repository identity cannot be replaced by a basename.
- Usage must have one accumulation rule per source. Overwriting cumulative cost
  undercounted 28 multi-skill runs; summing cumulative skill and terminal rows
  double-counted others. A boundary chart and its table must reconcile against
  the same population, and missing usage must not silently become zero.
- An unwritten `run_token_usage` table made a working reader permanently empty.
  Never build a second evidence store merely because its schema looks convenient.
- Expose the evidence layer's own health before its charts. Compare like
  repository/flow/provider cohorts, preserve unknown skill attribution, and keep
  delivery/intervention metrics parked until their source records exist. Do not
  infer delivery from sparse PR snapshots or create a synthetic productivity score.

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
  references in the Run manifest; do not put transcript blobs in SQLite.
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

## Wave memory

- **`wave/<name>/MEMORY.md` is the whole durable memory.** There is no journal
  delta, replay buffer, runtime copy, broadcast, HTTP write route, SSE frame, or
  second checkpoint to reconcile with it.
- **Memory is Wave-owned, not process-owned.** A running listener or provider
  does not hold a more authoritative version. Reviewed repository edits curate
  the file, and reading it is an ordinary file read that works with every
  server stopped — there is no CLI surface for memory.
- **Inheritance is explicit and file-based.** Prompt assembly reads applicable
  ancestor Wave memories oldest-first. Project and Task Work do not own memory,
  and unrelated recent Wave chat is never ambient prompt context.
- **Keep the file cold-reader sized.** Fold durable decisions, constraints, and
  evidence-backed learnings into it; do not turn it into a session log or raw
  fact archive. Git carries its history.
- **No memory backend.** Loopflow has no vector store, remote memory service,
  live compactor, export protocol, or vendor-memory dependency.

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
- **Do not infer present Run identity from the old inherited-span model.** The
  harness publishes one immutable Run manifest before launch and records verified
  parentage separately. Inspect that manifest when attributing a demonstration.
- **Only committed `MEMORY.md` crosses a branch or machine boundary.** Per-run
  journals remain execution evidence, not an uncompiled memory tail.
- **Provider context is not Wave memory.** Loopflow cannot and need not extract
  a provider's opaque working state; the next Work Run starts from durable
  authored context.

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

## Someday (explicitly not now — Jack: "maybe someday")

- **The controlled eval harvester** (cadenza first). The design above remains
  parked until Trace has delivery, intervention, complete context, and transcript
  evidence and the wave deliberately reopens Evals as a project.

## Code map

Current capture and readers live in `run_record.rs` and
`lf/commands/{runs,usage,replay}.rs`. Codex normalization lives in
`harness/codex.rs`; shared chat folding lives in `chat/turns.rs`. Runtime ownership
and context contracts are documented in `docs/architecture/execution.md`.
`RunSnapshot` is a disposable Run projection; `FinalAnswer` reports prose and
exactness. Neither becomes a wire-model default or a source of execution authority.

Wave memory remains committed `wave/<name>/MEMORY.md`, curated by ordinary
repository edits. There is no live delta, memory backend, or cross-machine replay
protocol beside it.
