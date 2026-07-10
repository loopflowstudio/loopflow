# Research: Trace and product auditability

> Background only. `scratch/intelligence.md` is the executable design for this
> branch; do not expand implementation into Product/Auditability UI work.

## System understanding

Trace is Loopflow's durable evidence plane. Product auditability is the path a
person follows through that evidence. They meet at readers; they should not
share ownership of ingestion or invent parallel run models.

### Architecture

`journal/mod.rs` writes process lifecycle and cumulative usage boundaries to
the local `run_events` table. A `run_id` identifies a trace, a `process_id`
identifies a process span, and `parent_process_id` supplies lineage. The same
events are also appended to per-repo JSONL journals.

The CLI is the reader API:

- `lf runs` folds recent events into one row per process.
- `lf trace` reconstructs the process tree and points to prompt/output logs.
- `lf usage` diffs cumulative boundaries into additive spend.
- `lf doctor` checks continuity, vocabulary, attribution, identity, lineage,
  and token coverage.
- `lf tokens` measures tracked code and its history with the same tokenizer as
  prompt budgeting.

`RegistryQuery` mirrors these JSON shapes in Swift. The standalone Telemetry
window consumes `lf usage`, `lf doctor`, and `lf tokens` directly. Its charts
already show tokens by skill and model, cache ratio, and codebase size/history.

The product control plane is separate. `lf status` reads the `runs` table and
returns wave runs, tasks, branches, errors, PR state, and attention. `WavesView`
and `WaveDetailPane` render that snapshot alongside the wave plan and chat.
Live `op` frames report current run motion over the wave SSE connection.

### Data flow

Agent process -> journal writer -> `run_events` -> `lf runs/trace/usage/doctor`
-> `RegistryQuery` -> Telemetry or a future run-detail surface.

Wave execution -> `runs` + terminal sessions + attention -> `lf status` ->
`RegistryQuery` -> `WaveDetailPane`.

The missing join is load-bearing. Only 3 of 188 control-plane `runs.id` values
currently occur as a `run_events.run_id`. The wave pane therefore cannot move
from a task/PR row to its durable trace reliably. `RegistryQuery` has a
`recentRuns()` reader but no `trace(id)` reader, and the wave UI does not use
either. Active-run rows are not clickable and do not attach to sessions.

### Key abstractions

- **Control run**: one wave-owned unit of work in `runs`, carrying task, branch,
  PR, status, and terminal-session relationships.
- **Trace**: all nested `lf` processes sharing one `run_id` in `run_events`.
- **Span**: one process in a trace, keyed by `process_id`.
- **Boundary**: a skill or terminal row with cumulative usage; `own_spend`
  turns it into an additive reading.
- **Audit surface**: a product view over authoritative readers. It owns
  navigation and explanation, not another persistence path.

## What exists

- The post-057 ledger contract is implemented and guarded by CHECK constraints.
- Cost, duration, provider, model, skill boundaries, and process lineage are
  represented in `run_events`.
- CLI readers and their JSON DTOs have focused unit coverage. Swift DTO mirrors
  have decoding tests.
- The Mac Telemetry window is a real consumer of the same additive usage rows
  the CLI prints; it does not recompute attribution.
- The main wave surface already shows objective, projects, active runs, open
  PRs, backlog, chat, live loop state, and live operational frames.
- A drifted-database fixture protects the destructive 057 identity cutover.
- Exact rendered prompt files already survive worktree deletion under
  `~/.lf/logs`, and `ContextBreakdown` already computes per-source and
  per-document token weights before launch.

## What the real ledger says

On 2026-07-10 the long-lived ledger held 1,613 rows, 277 traces, and 767
processes. `lf doctor` failed lineage: three parent ids affect 23 processes.
Coverage warned because only 27 of 56 agent-bearing processes carried tokens
(48%). All 1,613 `context` fields were empty, and no `escalated` event had ever
been written. Only seven control runs carried `snapshot_pr`.

The last 30 days of `lf usage` produced 62 additive boundaries across four
repos, with Claude and Codex attribution. That is enough for dogfood trends,
but not for cost-per-landed-change or intervention rates: delivery and
escalation records do not exist.

The prompt and conversation evidence is asymmetric. Prompt bytes are durable,
but the computed `ContextBreakdown` is displayed and discarded. The normalized
harness event stream exists, while the output-log append primitive is unfed;
this machine currently has zero `~/.lf/output/*.log` files. `lf trace` can point
at prompts but cannot show everything the user, agent, and tools said during a
run.

## Tensions

- **Two run identities**: control runs and traces are both real concepts, but
  their relationship is implicit and mostly absent. Conflating them would lose
  the distinction; leaving them unlinked blocks audit drill-down.
- **Trace versus Auditability**: Telemetry is a product surface, but its data
  contract and correctness belong to Trace. Auditability should compose the
  reader into wave navigation, not own the ledger.
- **History versus live state**: SSE `op` frames are live-only by design; the
  ledger is history. The UI needs one fold without turning SSE into a second
  journal.
- **Outcome versus process success**: a completed `lf` process does not prove a
  landed change. Delivery and human-intervention evidence are prerequisites for
  useful longitudinal dogfood evaluation.
- **Roadmap versus code**: the Trace task descriptions predate migrations
  056-058 and the dashboard. They still list shipped gaps while omitting current
  failures.

## Observations

### Complexity

The trace implementation is concentrated but large: journal, four readers,
storage, Swift mirrors, and the dashboard total several thousand lines. The
highest-risk logic is the identity/lineage contract and cumulative-to-own spend
conversion. Both have focused tests and should retain one implementation.

The product path currently crosses two DTO families (`RunSnapshot` and
`RunLedgerEntry`/`TraceSpan`) without a joining type. Adding UI before naming
that join would hard-code an implementation accident into the product model.

### Quality

Ledger vocabulary, absolute repo identity, terminal attribution, and additive
spend are explicit and tested. The dashboard comments clearly state its source
of truth. `lf doctor` failing on the actual ledger is useful evidence that the
monitor is not decorative.

The auditability surface is shallower. Wave rows show status but not its reason;
active sessions are inert cards; recent ledger history is unused; there is no
run-detail view; and summaries/planning claims have no receipt type.

### Potential

Most product auditability work can reuse shipped readers. The small missing API
pieces are an explicit control-run-to-trace link and `RegistryQuery.trace(id)`.
Once those exist, wave -> run -> trace -> live session can be one product path.

The wave can evaluate its work as a cadence over this evidence: cite a failing
run before a context change, then review comparable follow-up runs. No separate
Evals project or synthetic harness is required.

## Open questions

- Should a control run store a distinct `trace_id`, or should placed execution
  guarantee that its existing id is the root trace id? The concepts remain
  distinct even if their UUIDs intentionally match.
- What is the smallest durable delivery record: a `run_events` node/event, a
  separate delivery table, or an explicit link from control run to merged PR?
- What counts as an escalation: pass-budget exhaustion, an attention item,
  explicit agent escalation, or each boundary separately?
- Project-file claim receipts need a stable claim identity before a UI can link
  them. That should not be inferred from Markdown line numbers.

## Storage sizing

Measured by file metadata on 2026-07-10, without reading session contents:

- Codex sessions: 2.09 GB across 2,816 JSONL files; median 458 KB, p95
  2.33 MB, max 42.8 MB.
- Claude project data: 865 MB across 3,445 files. Its 2,188 JSONL transcripts
  account for 824 MB; median file 30 KB, p95 773 KB, max 101 MB.
- Existing Loopflow prompt logs: 1.56 GB across 31,746 files. Their top ten
  files account for 44.6% of bytes, including one 385 MB outlier.
- Codex plus Claude session directories compress from 2.98 GB to 998 MB with
  gzip level 1, a 3.0x reduction.
- New files in the last 30 days total 1.29 GB, about 16 GB/year raw or
  5 GB/year compressed at that rate. The unusually active last seven days
  total 866 MB, an upper projection of 45 GB/year raw or 15 GB/year compressed.
- The machine currently has 201 GiB free.

Storage is not the near-term constraint. Copying provider stores byte-for-byte
would duplicate data and inherit unstable formats. Prefer a durable Loopflow
core — exact provider-facing prompts, context manifest, normalized conversation
and tool events, usage, and artifact identities — compressed and content
addressed where assets repeat. Keep the raw Codex/Claude session as an optional
pointer. If the provider removes it, the product should say the raw artifact
expired while retaining the normalized trace. Large raw tool artifacts are the
first evictable cache; the small explanatory core should not rotate silently.

## Recommendations

### Make Trace own evidence and Auditability own navigation

**Observation**: The current architecture already follows this split, while
the roadmap mixes dashboard behavior with ledger work.

**Cost**: Task cleanup plus explicit interface wording.

**Benefit**: Each new feature has one owner and the product cannot fork trace
semantics.

**Verdict**: Do now.

### Repair real-ledger integrity before adding metrics

**Observation**: `lf doctor` is red and token coverage is 48%.

**Cost**: Small, separate writer repairs with real-ledger verification.

**Benefit**: Every downstream chart and dogfood conclusion becomes credible.

**Verdict**: Highest-priority Trace work.

### Link control runs to traces before building run detail

**Observation**: The two stores join for only 3/188 control runs.

**Cost**: A schema/API decision, migration, DTO mirrors, and focused tests.

**Benefit**: Unlocks the Auditability project's central wave -> run -> session
path without a parallel history model.

**Verdict**: Do before product drill-down.

### Persist one complete run artifact before adding more charts

**Observation**: Exact prompts, component weights, and normalized conversation
events already exist at runtime but do not meet in one durable record.

**Cost**: Wire existing launch and harness boundaries into a local artifact
bundle; store pointers and summary dimensions in `run_events`, not transcript
blobs.

**Benefit**: One trace answers what the model received, how each context asset
was weighted, and everything said or done during the run. Context-quality
review, replay, and asset-size graphs all build on the same evidence.

**Verdict**: After repairing lineage, this is the next substantive Context and
Trace slice.

### Retire Evals as a project

**Observation**: Real multi-repo dogfooding already supplies the feedback loop,
and Trace plus Context own every prerequisite. A separate project adds a third
place to describe the same work.

**Cost**: Fold weekly sampling into wave cadence and preserve controlled-harness
research as a dormant possibility rather than an active bet.

**Benefit**: Two legible Intelligence projects: Context defines the operating
contract; Trace monitors what launches received and what happened.

**Verdict**: Retire Evals without claiming its KRs were achieved.
