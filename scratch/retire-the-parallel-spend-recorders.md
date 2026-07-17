# Retire the parallel spend recorders

## Problem

Loopflow records token spend twice, from two independent parser stacks, into two
stores at two grains:

| | `run_events` | `agent_turns` |
|---|---|---|
| Parser | `StreamEvent` (`engine/stream.rs`) | `ConversationEvent` (`harness/*_mapping.rs`) |
| Sink | `journal::PendingUsage` thread-local, drained at run/skill boundary | `trace.rs` `TraceCapture`, flushed at turn boundary |
| Grain | per-process boundary | per-launch, per-turn |
| Attribution | **last-writer-wins per process** | per launch |

PR #1022 unified the *grain* (per-boundary deltas, one aggregation rule) but
deleted no store, because coverage looked asymmetric. Measuring the live ledger
shows the asymmetry is real but points the other way from the received story,
and that both stores share one much larger hole.

Who benefits: anyone asking "what did this cost?" — today `lf usage` answers
from the *wrong* store and omits the dominant workload.

## Measured state (live ledger, `~/.lf/loopflow.db`, 2026-07-16)

Totals over the whole store:

| | rows | input | output | cache read | cost |
|---|---|---|---|---|---|
| `run_events` (usage-bearing) | 94 | 204,318,502 | 1,079,068 | 59,699,090 | $64.41 |
| `agent_turns` | 1,040 | 226,442,134 | 2,619,049 | 536,556,720 | $60.68 |

Three findings that reshape the task:

**1. Task/Project Session spend is recorded NOWHERE — not in either store.**
`task/runner.rs` and `project_session/runner.rs` drive the vendor CLI in-process
through a `Harness` they own (`task/runner.rs:154`, `project_session/runner.rs:110`).
They receive `ConversationEvent::TurnUsage` with correct token counts and
pattern-match it into a no-op arm (`task/runner.rs:949`,
`project_session/runner.rs:479`). Neither constructs a `CaptureHandle`, so no
`agent_launches` row and no `agent_turns` row is ever created — and there is no
child `lf` process under which the spend hides (the subprocess is Anthropic's
`claude` binary, which knows nothing of our ledger). It is not "capture only,
never the ledger"; it is lost. This is most of the fleet's spend today.

**2. `run_events` provider attribution is wrong on multi-launch processes.**
`journal::record_agent` sets provider/model per process, last writer wins.
Process `2eb82575` launched two agents (claude, then opencode). `run_events`
records its 5,197 output tokens and $0.99 as `provider=opencode`,
`model=opencode/glm-5.2`. `agent_turns` attributes the same spend correctly to
the claude launch, and leaves the opencode launch's turn empty. So the current
`lf usage` per-provider table is not merely coarser — it is incorrect. 57
processes (of 358) launched more than one agent; one launched 94.

**3. opencode usage is parsed correctly, then destroyed.** Not "missing".
`engine/stream.rs` parses opencode's usage into `StreamEvent::Usage` — proven by
the real numbers in `run_events`. `trace.rs` consumes *both* stacks, and they
disagree on semantics:

```rust
StreamEvent::Usage { .. }  => { self.usage.input_tokens += ...; }   // accumulate
ConversationEvent::TurnUsage { usage, .. } => { self.usage = usage.clone(); } // CLOBBER
```

`opencode_mapping::complete_turn` (`opencode_mapping.rs:118-135`) *always* pushes
`TurnUsage { usage: usage.unwrap_or_default() }` — and `map_turn_usage` reads
`properties.usage` off the `session.status` idle event, which never carries it.
So a zeroed `TurnUsage` overwrites the real accumulated usage at every opencode
turn end. All 8 opencode turns have NULL/zero tokens.

Reconciliation of what a column drop would lose *today*: of 70 processes with
`run_events` spend, **0** lack turn coverage and **0** have zero-valued turns.
The residual gap is value-level (~4.3% on claude cost), concentrated in launches
whose capture never cleanly finished (`capture_status`: 935 complete, 75
capturing, 10 partial, 10 prompt_only; turn `status`: 75 still `running`).

## The demo

Run a Task body, then:

```
lf usage
```

The Task's own tokens appear in the table — attributed to the provider that
actually spent them — where today they appear nowhere at all. Then
`sqlite3 ~/.lf/loopflow.db "SELECT count(*) FROM pragma_table_info('run_events')
WHERE name IN ('input_tokens','output_tokens','cache_read_tokens','cost_usd',
'duration_secs','provider','model')"` returns `0`, and `lf usage` still answers.

## Approach

Two serial PRs. The split is not incrementalism: PR1 is a user-visible win on
its own (Task spend becomes visible for the first time), and PR2's central
verification — that turn-derived totals reconcile against the ledger — is only
*meaningful* once PR1 has been recording for real runs. Cutting readers over to
a store that is still missing the dominant workload would ship a regression and
call it a merger.

### PR1 — close the coverage holes (writers)

1. **Session runners capture turns.** Give `task/runner.rs` and
   `project_session/runner.rs` a `CaptureHandle`, following the
   `flowloop/wave.rs:675-691` pattern (both already call the same
   `prepare_harness_turn`; wave.rs simply wraps it in capture and they omit it).
   Route their `ConversationEvent` stream — `TurnUsage` included — into
   `capture.record_conversation`, replacing the no-op arm.
2. **Stop the opencode clobber at its source.** `complete_turn` takes
   `Option<TurnUsage>`; only push `ConversationEvent::TurnUsage` when it is
   `Some`. A defaulted `TurnUsage` is a lie — it asserts "the provider reported
   zero" when the provider reported nothing. `TurnUsage` becomes
   authoritative-and-complete by contract.
3. **Pin both writers in tests, and sabotage both.** Assert values, not shape —
   twice, because the two holes are equally load-bearing and the bigger one is
   the Session runners.
   - *Session runners:* an automated test proving a Task/Project Session
     runner's turn usage reaches `agent_turns` with real token values under the
     spending launch. **Sabotage step:** restore the no-op `TurnUsage` arm
     (`task/runner.rs:949`) and confirm the test goes red. If it stays green it
     is pinning a fixture, not the writer.
   - *opencode:* the existing conformance test (`conformance_tests.rs:345`)
     asserts only the *shape* `Some(ConversationEvent::TurnUsage { .. })` —
     which is exactly why the zeroes survived for months, while codex's
     equivalent (`:249`) asserts concrete numbers. Assert real tokens end-to-end
     into `agent_turns` for all 8 turns.

   Both writers are the whole substance of PR1; neither may rest on a manual
   demo, which cannot fail in CI.

### PR2 — cut the readers over and delete the recorder

4. **Readers answer from `agent_turns ⋈ agent_launches`.** `lf runs` already
   does exactly this (`runs.rs:985-1001`) — the join is proven, and `lf runs`
   stays untouched. Re-source `boundary_spans`/`trace_spans` (`runs.rs:1062-1202`)
   from turns; `usage.rs` and `top.rs` consume them unchanged.
5. **Keep the `SpanDto` wire shape byte-identical.** The Mac dashboard decodes
   it field-for-field (`RegistryQuery.swift:543-593`). Every spend field there is
   already Optional, so nulls decode; the non-optional set — `runId`,
   `processId`, `seq`, `node`, `startedAt`, `status` — is fully supplied by the
   join (`launch.run_id`, `launch.process_id`, `turn.ordinal`,
   `skill.is_some() ? "skill" : "run"`, `turn.started_at`, `turn.status`).
   **No Swift change ships in this task.**
6. **Delete the recorder.** `PendingUsage`, `record_usage`, `record_result`,
   `record_agent`, `drain_usage`, `clear_usage` (`journal/mod.rs:110-230`) and
   `record_stream_usage` (`engine/agent.rs:1135-1150`).
7. **Migration `0.11.027`** drops the seven columns and updates
   `validate_run_events_schema` (`sqlite.rs:436-442`). **Not `0.11.026`** — PR
   #1028 already claims it (see De-risking).
8. **Invert the doctor check.** `check_capture`'s `uncaptured_spend`
   (`doctor.rs:191-205`) asks "a `run_events` row reports spend but has no
   launch" — unaskable once the columns are gone. It becomes a coverage check on
   turns: a launch whose turns report no usage, and a turn left `running` past a
   threshold. That is the same defect the old check guarded, asked of the
   surviving store.

## Absent and error states (reader boundary)

Once readers answer from turns, "which turns are spend?" needs one rule. It is
the exact translation of today's gate, and the measurement says it is free:

**A turn contributes to `lf usage`/`lf top`/`lf trace` iff
`provider_input_tokens IS NOT NULL`.** This mirrors `boundary_spans`' existing
`input_tokens.is_some()` gate (`runs.rs:1163`). Measured: 634 of 1,057 turns
pass, and their totals are *identical* to summing all 1,057 (input 227,196,444 /
output 2,661,626 / $60.68) — the 423 excluded turns contribute exactly zero. The
gate changes row count, never totals.

**Do not filter on `status`.** The tempting `status = 'completed'` silently
deletes money: 12 `failed` turns carry real usage — 22,917 output tokens and
**$3.68, 6% of the store's total cost**. A turn that failed still spent what it
spent. Usage presence is the gate; status is not.

`running` (75), `partial` (20) and `interrupted` (9) turns all currently have
NULL usage, so the usage-presence gate excludes them with no special case. That
is a property of today's data, not a rule — if a `running` turn ever reports
usage, it counts, and that is correct.

**NULL is not 0.** NULL = the provider reported nothing (unknown). 0 = the
provider reported zero. Today the opencode clobber conflates them by writing
zeros for "unknown"; PR1's source fix is what makes the distinction honest, and
`aggregate_spend` (`usage.rs:344-347`) already skips all-zero rows.

**A launch with no turns** contributes nothing to usage and *is* the defect the
inverted doctor check reports. Zero exist today.

**Doctor threshold: a turn left `running` with `started_at` older than 1h is an
orphaned capture.** Measured: 74 of 75 `running` turns are already older than 1h
(oldest 54.3h); exactly one is live — the session writing this. So 1h separates
orphans from live turns cleanly, and the inverted check finds 74 real defects on
day one. That is the check earning its keep, not a false alarm to tune away.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Is `run_events` coverage really "complete: every provider, every engine run"? | No. Task/Project Session spend — the dominant workload — is in neither store. Both runners drive the harness in-process with no `CaptureHandle`. | PR1 exists. This is the task's real work, and it is additive. |
| Does `agent_turns` "miss opencode entirely"? | No. 8 opencode launches, 8 turns. The rows exist and are zeroed by the `TurnUsage` clobber. Usage *is* parsed (`engine/stream.rs` → real numbers in `run_events`). | Fix is 1 line at `opencode_mapping.rs:133` + a value-asserting test, not a new capture path. |
| Would dropping the columns lose data? | Of 70 processes with `run_events` spend, 0 lack turns; 0 have zero-valued turns. Residual is value-level (~4.3% claude cost), from launches that never finished capture. | Drop is safe for whole-process coverage. Residual is disclosed, not hidden. |
| Can `lf usage` output be **identical** before and after (Done-When #4)? | **No — and it must not be.** Grain changes (94 boundaries → per-turn rows); totals differ ~4.3%; and where they differ `run_events` is *wrong* (misattributes provider on 57 multi-launch processes). Reproducing it bit-for-bit would mean reproducing the bug. | Done-When #4 reframed to a reconciliation with every divergence attributed. Recorded in `scratch/questions.md`. |
| Does a verify-then-drop migration work? | It would refuse forever: the totals it checks provably do not match, for the reasons above. A migration that can refuse is an operational hazard on every future `lf` start. | Verification runs pre-land, offline, on a ledger copy. The migration only drops. |
| Is the migration ordinal safe? | `0.11.026` was claimed by then-open PR #1028 while local `ls` showed max `0.11.025` — the wave MEMORY hazard, live. #1028 has since merged; main now tops out at `0.11.026_lineage_boundary.sql` and **no open PR ships any migration**, so `0.11.027` is uncontested. | Take `0.11.027`. Re-scan open PRs at land time regardless, per wave MEMORY — the ordinal was contested once already, and the scan is what settled it both times. |
| Does SQLite support `DROP COLUMN`? | Yes — needs ≥3.35; rusqlite 0.40 `bundled` ships 3.51. No `DROP COLUMN` precedent in our migrations yet. | Plain `ALTER TABLE ... DROP COLUMN`, seven of them. |
| Does the Mac dashboard break? | Only if `SpanDto`'s shape changes. All spend fields on Swift `TraceSpan` are already Optional, and `RegistryQueryTests.swift:255` pins that a null-spend span decodes. | Preserve the shape exactly; ship no Swift change. That fixture test **is** the proof — booting the dashboard adds nothing and is not a Done-When (see below). |
| Is `lf runs` affected? | No. It reads `run_events` only for `process_parents` lineage (`runs.rs:942-945`); every spend field already comes from `agent_turns`/`agent_launches`. | Requirement "lf runs unchanged" holds by construction. |
| Is codex cost lost? | `run_events` records `0.0`; turns record NULL. Codex is subscription — cost is genuinely unknown, so NULL is the truthful value. `usage.rs`'s table path ignores `cost_usd` entirely; Swift sums `costUsd ?? 0`. | No visible change. NULL is more honest than a fabricated 0.0. |
| Will net LOC be negative? | Deleting `PendingUsage` + drain + `record_stream_usage` + the doctor branch ≈ 150–250 lines; PR1 adds ≈ 60–100 (mostly reusing `CaptureHandle`). Reader re-sourcing is ~neutral. | Plausible across both PRs; measured at PR2, not asserted per-PR. PR1 alone is net positive and that is correct. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Keep `run_events` as the spend store; delete `agent_turns` columns | Fewer writers to fix; complete-looking coverage | Rejected by recorded decision, and the measurement independently vindicates it: per-process attribution is wrong on multi-launch processes and can't carry reasoning/cache-write/context-pressure. |
| Drop the columns now; fix writers later | Immediate LOC win | Ships a regression: Task spend is in neither store, so `lf usage` would go from wrong to emptier. Writers first is not optional ordering. |
| Backfill `agent_turns` from historical `run_events` before dropping | Makes totals reconcile by construction | Synthesizes per-turn rows from per-process boundaries — inventing grain we never observed, and importing the misattribution we're deleting. Worse: it would make the wrong numbers permanent and unfalsifiable. |
| Merge (not clobber) `TurnUsage` into accumulated stream usage in `trace.rs` | Tolerates defaulted `TurnUsage` from any provider | Treats the symptom. A provider emitting a defaulted `TurnUsage` is the bug; merging makes every future mapping gap silent instead of loud. Fix the source; keep clone semantics honest. |
| One PR for everything | "Complete over incremental" | The two halves are genuinely independent deliverables, and PR2's verification is only meaningful after PR1 has recorded real runs. Landing both blind would make the reconciliation untestable. |

## Key decisions

- **The recorded decision stands, and now has evidence.** `agent_turns` wins not
  only for grain and richness but for *correctness*: `run_events` misattributes
  provider on multi-launch processes. This was not the stated reason; it is a
  stronger one.
- **"Identical before/after" is retired as a success criterion.** It is
  unachievable and, more importantly, undesirable — it would require preserving a
  known misattribution bug. Replaced by an attributed reconciliation. Surfaced in
  `scratch/questions.md` rather than silently satisfied or silently failed.
- **The migration never refuses.** Verification is a pre-land activity on a
  copy. Shipping a migration that gates on a condition known to be false would
  brick `lf` startup.
- **Ship no Swift change.** The wire shape is the contract; holding it fixed is
  what makes the store swap invisible to the dashboard. The existing fixture test
  proves that — the running app is not asked to.
- **Fix parsers at the source, not the sink.** The zeroed `TurnUsage` is deleted
  where it is minted.
- **Two parallel *parsers* is the deeper duplication.** This task retires the
  parallel *store*; `StreamEvent` and `ConversationEvent` both surviving is a
  real follow-up, deliberately out of scope (see below).

## Scope

**In scope**
- Session runners (task, project_session) construct captures and record turn usage.
- opencode stops emitting defaulted `TurnUsage`; conformance test asserts values.
- `usage.rs`, `top.rs`, `trace_spans`/`boundary_spans` re-sourced to `agent_turns ⋈ agent_launches`.
- Delete `PendingUsage` + drain machinery + `record_stream_usage`.
- Migration `0.11.027` drops 7 columns; `validate_run_events_schema` updated.
- `lf doctor` capture check inverted to turn coverage.
- Reconciliation on a copy of the production ledger.

**Out of scope**
- Unifying `StreamEvent` and `ConversationEvent` into one parser stack — the
  deeper duplication, and the root cause of the opencode clobber. Filed as
  `92e0f253-f551-47d8-b74d-21a37e5dd551` (retiring the parallel usage parsers),
  which must not start before both of this task's PRs land.
- The wave journal's per-turn narration — a log, not a query source. Stays.
- Any Swift/dashboard change.
- `lf runs` / `lf execs` behavior.
- Repairing the 75 `capturing` / 10 `partial` historical launches.

## Done when

1. **An automated test** proves a Task/Project Session runner's turn usage
   reaches `agent_turns` with real token values, attributed to the spending
   launch. Today: absent from every store. The test must go red when the no-op
   `TurnUsage` arm is restored — sabotage it and confirm, or it is pinning a
   fixture. The demo above is how a human *sees* this; the test is how it stays
   true.
2. An opencode turn carries real tokens end-to-end into `agent_turns` for all 8
   turns (conformance test asserts values, not shape).
3. `PRAGMA table_info(run_events)` lists none of the seven spend columns after
   migration `0.11.027`; `lf usage`, `lf top`, `lf trace` still answer; `lf runs`
   output is byte-identical.
4. **Reframed:** on a migrated copy of the production ledger, `lf usage` totals
   before and after reconcile, with every divergence attributed to a named cause
   (multi-launch misattribution, unfinished capture) and none unexplained. Not
   bit-identity — see De-risking.
5. `cargo test -p loopflow` green to completion (per wave MEMORY: a red result
   still needs attribution — `flowloop::wave` tests flake under parallel load).
6. Net LOC negative across both PRs.

The Mac dashboard is deliberately **not** a Done-When. `SpanDto` stays
byte-identical, every Swift spend field is already Optional, and
`RegistryQueryTests.swift:255` already pins that a null-spend span decodes — so
booting the app to compare two numbers proves nothing that fixture test doesn't,
cannot run headlessly (wave MEMORY: the UI suite hangs `xcodebuild`, exit 65;
populated Waves are unavailable locally), and resolves to a human eyeballing
totals. This Project's first KR is that avoidable human-in-the-loop steps fall to
zero. "Ship no Swift change" stays a decision; the fixture test is the proof.

## Measure

Baseline captured 2026-07-16 on `~/.lf/loopflow.db` (copy at `/tmp/ledger-probe.db`):

- `run_events` usage-bearing rows: 94 — input 204,318,502 / output 1,079,068 / cache 59,699,090 / $64.41
- `agent_turns`: 1,040 — input 226,442,134 / output 2,619,049 / cache 536,556,720 / $60.68
- Multi-launch processes (attribution risk): 70 of 358 have >1 launch
- opencode turns with usage: 0 of 8 → target 8 of 8
- Launches with no turn rows: 0
- Task Session spend visible in `lf usage`: **$0 / 0 tokens** → target: nonzero
- Turns passing the usage-presence gate: 634 of 1,057 — totals identical to all
  1,057, so the gate costs nothing
- Spend a `status='completed'` filter would delete: 12 failed turns, 22,917
  output tokens, **$3.68 (6% of cost)** → must stay at $0 lost
- Orphaned `running` turns (>1h old): 74 of 75 → what the inverted doctor check
  should report on day one

"Better" = a Task run's spend is queryable; opencode turns are non-zero; the
seven columns are gone; totals reconcile with every delta attributed.

## Wave alignment

Serves infrastructure's "the system is legible" objective: one store answers
"what did this cost?", and the answer stops being wrong. Advances Developer
Efficiency's *"Avoidable human-in-the-loop setup or repair steps found in agent
runs fall to zero"* — a spend question that requires knowing which of two stores
lies is exactly that tax. Deleting the recorder also serves the wave's standing
"would deleting code make the system more true?" review question.

**New risk introduced, and its gate (settled):** PR1 puts a `CaptureHandle` on
the Task/Project Session path, adding ledger writers to a store that is
*actively killing bodies* — wave MEMORY records the fleet-wide SQLite contention
incident (2026-07-16), and W2-284, W2-285 and W2-287 are parked failed on
`database is locked` right now. ENG-7 owns deterministic concurrent ledger
writes and is in kickoff.

**Decision: PR1 does not land until ENG-7 lands.** Not "unless contention looks
resolved" — settled. The one-UPDATE-per-turn-boundary measurement (the rate
`flowloop/wave.rs` already sustains across 1,030 launches) argues the pressure is
additive rather than multiplicative, but that is still an assumption about a
store that is currently stranding Sessions, and the cost of being wrong is more
dead bodies. Implement and review PR1 freely; the gate is on landing only.

**Do not arm auto-merge on PR1.** Wave MEMORY records that `lf pr land` arms
GitHub auto-merge, which answers only to required checks — it would sail straight
past this gate the moment CI goes green. Use `lf pr publish` for PR1 and land it
by hand once ENG-7 has merged.
