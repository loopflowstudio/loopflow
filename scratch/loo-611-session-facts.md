# LOO-611 — Keep only Session facts in the store, with one usage record per turn

Status: **implemented, awaiting gate** (2026-10-26). Product decisions are Jack
Heart's LOO-602 decisions 1–2 (accepted 2026-10-26, design in the LOO-602
checkout `scratch/loo-602-tracking.md`). This Task absorbs `5b0109`. The
inspection-Process expiry stays with LOO-615. Raw stream capture
(`runs/*/events.jsonl`, `context_usage.rs`) stays with the decision-3 Task.

## Problem

Three rounds of shrinking the store (LOO-390, LOO-570, LOO-572) each filtered a
payload after it had been written. The recorder still pipes every stream event
through `RetainedHistory::admit` → `retained_in_history` → a free-JSON
`observed` row, and the prune repeats the same rule in SQL (`RETAINED`). Usage
is still written twice. The store stays small only while the filter and the
prune agree.

## Demo

On a clone of Jack's store (`cp -c ~/.lf/loopflow.db $T/loopflow.db`, `LF_HOME=$T`):

```
lf usage --days 30 --json > before.json     # installed lf 0.14.15
<branch lf> usage --days 30 --json > after.json
# per-provider tokens equal; Claude cost equal ($2,550.72 in October)
sqlite3 $T/loopflow.db "select count(*) from session_events where kind='observed'
  and json_extract(payload,'\$.evidence.type') in ('usage','provider_output','conversation','user_input','activity')
  and coalesce(json_extract(payload,'\$.evidence.event.type'),'') not in ('turn_completed','error')"   # 0
sqlite3 $T/loopflow.db "select sql from sqlite_master where name like 'store_revision_%session_events%' or name='store_revision_usage_insert'"
  # no 'events.jsonl', no transcript type names
```

A live headless Claude turn then shows its tokens in `lf usage --task <T>`
while it runs, from its single native row.

## Risk findings (probed read-only on Jack's store, 2026-10-26)

- **Claude's native receipt is not a usage record yet.** `claude_history.rs`
  writes `{"provider":"claude","usage":<raw result.usage>}` only at `result`:
  no cost, model, context window or peak request. The reader then decodes it
  with the Codex branch (`recover_native_usage`: `_ => "codex"`) and skips it,
  because the recorder copy is final. Today every Claude usage number comes
  from the stream copy. Dropping that copy without changing the writer would
  lose all Claude cost ($2,550.72 recorded in October) and all mid-turn usage.
- **Mid-turn usage is only in the stream copy for Claude.** The most recent
  stream-only rows are running turns: native `started`, no native `usage` until
  `result`. Codex (`thread/tokenUsage/updated` per request), Pi and OpenCode
  (per message) already write native rows during the turn.
- **Stream-only usage exists.** Of 62,907 Claude observation rows (1,931
  inputs), 2,200 rows in 143 inputs have no native usage turn ($283.20 of the
  cost). That covers running turns, plus 117 inputs with no native history at
  all (headless, 10-04 → 10-26). Codex has 242 rows in 4 inputs without a
  native turn. 1 Claude native row has no stream match. Every observed usage
  row except 4 has a `provider_session_observed` thread, so it can be placed
  on a native turn key.
- **All usage in the store is from October 2026.** Claude native receipts began
  2026-09-30 (#1296), so the parity window is the whole store.
- **Readers, traced:**

  | Reader | Reads | After |
  |---|---|---|
  | `lf usage` / `lf history usage` (`usage.rs` → `project_input_history` → `reduce_usage_events` + `recover_native_usage`) | stream copy first; native only when the stream lacks a final receipt | native rows only |
  | Desktop (`RegistryQuery.swift`: `lf usage --json`, `--context --json`) | same DTO | unchanged wire, native-fed |
  | `lf history show` (`SessionHistory.usage`, `providers[].usage`) | same reducer | native rows only |
  | `context_usage.rs` (`--context`) | `runs/<capture>/events.jsonl` **file**, never the store | unchanged here; decision-3 Task |
  | Graph step result (`graph.rs` `turn_events` → `input_events`) | observed `conversation` `turn_completed{interrupted}` / `error` | same rows, now typed facts |
  | `context_cost.rs` (`input_silences`) | `:silence:` facts | unchanged |
  | `usage` revision domain | trigger on both copies | trigger on `kind='usage'` |

- Native usage upserts are DELETE + INSERT (`session_events.rs` `record_event_in`), so an insert-only `usage` trigger still advances.
- Migrations are SQL only, with no Rust data step. Conversion of existing rows
  must be expressible in SQL.
- `v0.14.16`, which carries the LOO-570 prune, is unpublished (installed:
  0.14.15). Jack's store has not been pruned: `session_events` has 2.54M
  `events.jsonl` observation rows. A full-scan probe over them takes about 55 s.

## Chosen approach

**1. The recorder cannot write transcript content (by type).** The
`SessionRecorder` thread keeps only its `events.jsonl` file append (decision-3
Task removes it) and loses its store handle. Facts are written synchronously
by the `SessionCapture` method that produces them, through one typed store
call:

```rust
// session_record.rs
enum CaptureFact {            // closed set; no text, no ConversationEvent
    AttemptStarted { provider, model, account_id, attempt_key },
    AttemptFinished { attempt_key, outcome },
    AccountSelected { attempt_key, account_id },
    ProviderSession { attempt_key, agent_session },
    TurnEnded(TurnEnd),       // Completed{turn_id,status} | Error{code,message,evidence}
    Silence { since },
}
store.record_capture_fact(&session, input, seq, at, &fact)
```

`CaptureFact` serializes to the row shape already stored (`events.jsonl:{seq}`
keys with the `EventEnvelope` JSON, and `silence:{seq}` for silences), so
existing readers and released rows need no second reader. `TurnEnd`
serializes as today's `{"type":"conversation","event":{…}}`. Silence is
tracked in `SessionCapture::append_event` (previous event time) and written
there. The `manifest.json` row (with `exec` nulled) is written once at capture
start by the same capture code, not by the recorder thread. `RetainedHistory`,
`retained_in_history` and the recorder's `observe`/`retain` closures are
deleted. `SessionCapture::append_usage` stops reaching the store.

**2. One usage record: the native row, one per provider turn.**
- Claude: `claude_history.rs` writes `{"provider":"claude","usage":TurnUsage,"final":bool}`.
  This is Pi's shape. It writes on each assistant message (cumulative, `final:false`)
  and at `result` (`final:true`, cost, model, context window, peak). Both upsert
  the turn's one row through `record_event_in`. The normalization moves out
  of `claude_mapping.rs` into one small `ClaudeUsage` state struct, which the
  mapping (live `UsageCheckpoint` display) and the history writer both use.
- Codex, OpenCode, Pi: unchanged writers.
- Reader: `recover_native_usage` becomes `native_usage(history) -> Vec<TurnUsage stream>`.
  It dispatches on payload shape, not provider name: a `usage` object is a
  `TurnUsage` (Claude, Pi, converted rows); `total`/`last` is Codex cumulative
  with the existing baseline math; `message.tokens` is OpenCode.
  `reduce_usage_events` reduces native streams only. `RecordedAttempt` provider
  records carry outcome and timing, and no usage. The recorder-vs-native
  correlation (`matching`, `matches_native`, thread/attempt joins) is deleted.

**3. Draft migration `session_facts_only` converts every store once.**
Set-based SQL, in order:
1. Claude native rows matched to a stream copy (same session; the
   `provider_session_observed` thread of the observation's attempt =
   `provider_thread`; `turn_key = provider_turn`): payload becomes
   `{"provider":"claude","usage":<observation usage>,"final":<final_receipt>}`.
   The observation used is the final receipt, else the largest
   `observation_seq`.
2. Remaining Claude native rows (raw `result.usage`): converted in SQL to
   `TurnUsage`. `total_input_tokens = input + cache_read + cache_creation`;
   cost and model stay unknown.
3. Stream-only usage with a known thread: INSERT one `kind='usage'` row per
   (session, thread, turn), with the canonical `usage`-object payload and its
   `captured_event`. The 4 rows without a thread are dropped (recorded in
   questions.md).
4. Silence backfill, `exec` nulling and deletion of every `events.jsonl`
   observation that is not a `CaptureFact`, which is today's `prune_capture`
   as one statement partitioned by `captured_event`. This also deletes all
   observed `usage` rows.
5. Recreate `store_revision_session_events_{insert,update,delete}` as
   `WHEN NEW.kind!='usage'` (update: `NEW.kind!='usage' OR OLD.kind!='usage'`),
   and `store_revision_usage_insert` as `WHEN NEW.kind='usage'`.

Releasing this migration is what "once stores are converted" means. No
released lf ever needs the online capture pass afterwards.

**4. Retention prune loses its capture pass.** In `store/sqlite/retention.rs`,
delete `prune_captures`, `prune_capture`, `RETAINED`, `CAPTURE_SETTLE_SECONDS`,
the `.retention-cursor` file (deleted when found beside the database),
`compact_input_usage` and the `stream_rows` report field. Keep
`compact_native_turn` (native-only) and its `usage` cursor,
`close_dead_processes`, `expire_processes` (LOO-615) and `reclaim`.

## Alternatives rejected

- **Keep the stream copy for Claude only.** That leaves two representations,
  and violates decision 2.
- **Drop observed usage without folding.** It loses $2,550 of Claude cost and
  every Claude peak and context-window reading.
- **Keep converting online in the prune instead of a migration.** It keeps
  `RETAINED` alive and lets a lagging prune leave transcript rows behind,
  which is the failure class this Task removes.
- **Normalize Codex to `TurnUsage` at write time.** Correct direction, but
  that is decision 7 (`ProviderFact` decoders). Shape dispatch in one reader
  keeps one row kind and one row per turn now.
- **Read native turn end for Graph routing.** Claude's native `completed` has
  no `interrupted`. The typed `TurnEnded` fact is small and keeps Graph
  behavior; decision 7 can unify it.

## Wild failure, pre-empted

- Migration takes minutes on an unpruned 10 GB store and blocks `lf install`.
  Gate measures it on Jack's clone (budget: < 120 s). If it is over budget,
  land after v0.14.16 publishes and its online prune has converted the store;
  the migration then touches about 200k rows.
- Claude per-assistant upserts contend with other writers. That is the same
  cadence as today's observation writes, on the same store. A failed write
  warns once and the turn continues, as the recorder does now.
- A still-running pre-upgrade relay writes old-shape transcript rows after the
  migration. The new readers ignore them, and they bump only the `sessions`
  revision. This lasts until the relay restarts and is accepted.

## Scope

**User-visible outcome.** `lf usage`, `lf history show` and Desktop usage
report the same October totals, with Claude cost intact. Claude tokens update
during a running turn from one row. The store stops gaining transcript rows.

**Source of truth.** Native `session_events` rows of kind `usage`, one per
(session, provider thread, provider turn [, OpenCode message]). `SessionUsage`
and `ProviderHistory.usage` are derived from them.

**Absent states.** A turn with no native usage row has unknown usage, counted
in `evidence_gaps` the same way as today. A converted Claude row without a
stream match has unknown cost and model.

**Wire/DTO.** `SessionHistory`, `SessionUsage` and `ProviderHistory` keep
their fields. `RecordedAttempt.usage` is empty. No fixture change, unless the
DTO fixture carries attempt usage; if it does, update the fixture and every
language's test.

**Machine copies.** `machine_records` rows imported from a rental are read
as-is. Rentals run the same lf build, so their copies have the new shape
(assumption in questions.md).

**Exclusions.** `runs/*/events.jsonl` and `context_usage.rs` (decision 3).
Process expiry and the `records_process` denylist (LOO-615). Transcript
readers and retention opt-in (decision 4). `ProviderFact` decoders (decision 7).

## Delete — do not maintain

- `session_record.rs`: `RetainedHistory`, `retained_in_history`, `usage_identity`,
  `merge_usage_observation`, and the store half of `SessionRecorder::start`
  (`history`, `observe`, `retain`, the manifest observation). In
  `recover_native_usage`, delete the recorder correlation and the `_ => "codex"`
  fallback. In `project_provider_history`, delete the recorded-attempt usage
  reduction.
- `sessions.rs` `retain_history_in`: the usage-merge branch (a conflicting
  observation is an error again).
- `retention.rs`: `RETAINED`, `prune_captures`, `prune_capture`,
  `CAPTURE_SETTLE_SECONDS`, `compact_input_usage`, `RetentionReport.stream_rows`,
  the retention `Cursor`, and their exclusive tests
  (`prune_keeps_facts_records_silences…`: keep its Process assertions;
  `prune_merges_usage_recorded_per_observation…`).
- `claude_history.rs` raw `usage` receipt.
- Tests that seed observed `usage` rows (`session_events.rs`, `revisions.rs`
  around the `input:events.jsonl:*` fixtures, and `session_record.rs` reducer
  tests over recorder usage). Each one either becomes a native-row test or is
  deleted.

Must survive: attempt/account/provider-session/turn-end/silence facts and
their readers, `input_final_answer`, `provider_session_from_history`, Codex
baseline math, OpenCode per-message keys, Pi recovery, and `compact_native_turn`.

## Forbidden outcomes

- Any store write from the recorder thread, or a store API that takes a
  `CaptureEvent`/`ConversationEvent`.
- An observed `usage` row written after migration, or a reader that falls back
  to one.
- A reader branch on `provider == "claude"` raw `result.usage` beyond the
  one-time SQL conversion.
- Triggers that name `events.jsonl` or transcript event types.

## Internal slices (one PR)

All five slices are written: `ClaudeUsage` shared by mapping and history;
`native_usage` + `reduce_usage` over native rows only; `CaptureFact` written
synchronously by `SessionCapture` (recorder thread is file-only); draft
`session_facts_only.sql` with fixture `session_facts_only_released.sql`;
retention capture pass deleted. Remaining: gate's clone probe (migration time,
`lf usage` parity) and the full affected suites.

## Done when

- No code path from `SessionRecorder` or `CaptureEvent` reaches SQLite. The
  only recorder-side store call takes a `CaptureFact`.
- Every usage number comes from `kind='usage'` rows, and the store holds no
  observed usage after migration.
- The four revision triggers mention neither `events.jsonl` nor transcript
  event types.
- `lf usage --days 30 --json` on Jack's clone matches 0.14.15 per provider
  (tokens and Claude cost).

## Acceptance at gate (headless, inherited `LF_*` unset, `< /dev/null`)

- `cargo test -p loopflow --lib session_record claude_history session_events retention revisions`:
  the facts-only writer, one Claude row per turn with cost, and native-only
  reduction.
- `cargo test -p loopflow --test integration` for the migration upgrade
  fixture: converted payloads, inserted stream-only turns, zero transcript
  rows, and new trigger SQL.
- Clone probe on Jack's store (read-only source, `cp -c` copy): branch
  migration time recorded (< 120 s or sequence after v0.14.16), then the demo
  queries above, with before/after `lf usage --json` totals equal.

Check: `cargo clippy --all-targets -D warnings` clean; `cargo test --lib -- session_record session_events revisions retention claude_history session_facts_only` pass (2026-10-26). Deferred to gate: clone probe (migration time, `lf usage` parity), integration suite incl. `tests/linear/work_watch.rs`. `pi_turns_resume…` failed once under machine load, passed alone.
