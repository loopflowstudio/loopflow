# LOO-613 — Drop raw stream capture

Status: **all slices implemented**; main merged at e636ab63 (#1652) (2026-10-26).
Remaining: gate, publish, land.
Product decision accepted by Jack Heart in LOO-602 (decision 3, 2026-10-26):
delete `runs/*/events.jsonl` and `lf history show --events`. The two real
readers moved to decoder facts first. A debug capture comes back only for a
named purpose.

## Problem

Every headless step writes the full provider stream to
`~/.lf/runs/<ab>/<capture>/events.jsonl`: each stdout line, each normalized
`ConversationEvent`, cumulative usage, submitted prompt text, and CPU samples
every 15 s. The provider already keeps the conversation in its own transcript.
Loopflow gets two things from this file:

1. **Prompt cost per step**: `lf history usage --context` and
   `lf history show <input> --context` (`context_usage.rs::read_capture`).
   From the file it takes the first and peak provider request tokens (Claude
   `assistant` usage, deduplicated by message id), the count and cl100k size of
   tool results, Claude `compact_boundary` count and `pre_tokens`, and
   `usage.peak_input_tokens` for the other providers.
2. **Running/Stalled for a Task step**: `lf task status` calls
   `session_record/activity.rs::read`, which reads the file's last line,
   either a CPU `Observation` or the time of the last event, and samples CPU
   again itself.

Everything else in the file is unread or reads as a debug dump (`--events`).

## Demo

```
lf -b task run LOO-613 -a claude …        # any headless step
ls ~/.lf/runs/<ab>/<capture>/              # manifest.json context.json terminal.json (no events.jsonl)
lf history usage --context --task LOO-613  # carried, tool output, compaction and peak are filled from the store
lf history show <input> --events           # error: unexpected argument '--events'
lf task status LOO-613                     # Running through a silent 10-min cargo build;
                                           # Stalled 5 min after `kill -STOP <agent pid>`
```

## State (2026-10-26)

- **Context:** `ProviderFact::Request`/`Compacted`; `FactWriter` writes one
  `context` object on each `completed` receipt; `context_usage` reads
  `SqliteStore::input_turn_context`.
- **Activity:** draft `session_activity_cpu`; the capture's `Observer` writes
  CPU samples through its attachment; `activity::read` judges from
  `session_liveness`.
- **`--events`** is gone; `check_cost.py`, `replay_history.py` deleted;
  `context_ablation.py` reads transcripts.
- **Recorder cut (after #1698):** `SessionRecorder`, `RecorderMessage`,
  `CaptureEvent`, `EventEnvelope`, `record_raw`/`record_input`/`append_usage`,
  `append_json_line`, `sync_telemetry`, the capture's `turn_key`, the raw
  provider channel (`RawProviderEvent`, `Harness::set_raw_provider_sender`) and
  `StreamFormat` (only `Raw` was ever read; nothing read the color bool) are
  deleted. `record_conversation` keeps turn-end facts; every other event takes
  a stream position, which ticks the silence clock and the activity clock.
  File-only capture tests are deleted; the account test reads stored facts.
- Stored facts keep the `events.jsonl:{seq}` source label: receipt keys,
  indexes, released migrations and #1698's draft all match on it. Renaming it
  needs a data migration; not worth one.

## Current → target

| Concern | Before | Now |
|---|---|---|
| Provider stdout | harness `raw_provider` → `CaptureHandle::record_raw` → file | `raw_provider` feeds only `--stream raw` printing; capture never sees it |
| Normalized stream | `record_conversation` → file (+ turn key, TurnEnd fact) | keeps only the turn key, the `TurnEnd` fact and the silence clock; nothing is appended |
| Usage stream | `CaptureEvent::Usage` → file | gone (the native `usage` receipt is the record, per LOO-611) |
| Submitted prompt | `CaptureEvent::UserInput` → file | gone (`context.json`/manifest stay in `runs/`) |
| CPU samples | `CaptureEvent::Activity` → file, every 15 s | `session_activity.cpu_sampled_at/cpu_quiet_since`, updated in place |
| Context per step | parse file at read time | `context` object on each turn's `completed` receipt, folded by `FactWriter` |
| Silences | gap between consecutive capture events | same rule, clock ticked by every provider message the capture hears |
| `lf history show --events` | prints file, else retained rows | deleted |

### Context facts (slice 1)

- The `completed` receipt payload `FactWriter` writes at `TurnCompleted` gains
  `context`: `{first_request_tokens, peak_request_tokens, tool_results,
  tool_output_tokens, compactions, compaction_tokens}`. Each field is
  nullable, and a field is null when the provider did not report it. This adds
  no SessionEventKind, no row and no schema change. The row already exists
  for every turn, and its attribution to inputs is the one `lf history usage`
  already uses.
- New `ProviderFact` variants, emitted without turn identity:
  `Request { input_tokens }` and `Compacted { pre_tokens: Option<u64> }`.
  Claude emits `Request` once per main-conversation assistant `message.id`
  (input + cache read + cache write, skipping lines that carry
  `parent_tool_use_id`) and `Compacted` on `system/compact_boundary`. Codex
  emits `Request` from each `token_count` event's last-request input. OpenCode
  and Pi emit `Request` only where their native message usage reports input.
  Otherwise the field stays null and the reader's existing gap line reports
  it.
- Tool results need no new fact. `FactWriter::record` already receives the
  whole decoded slice, and it tallies `Display(ItemCompleted)` outputs as
  results and cl100k tokens (`prompt::count_tokens`, inside `off_reactor`).
  Claude subagent tool results must not count: check whether the Claude
  decoder emits them as Display items, and test it.
- The tally is kept per native thread and taken into that thread's next
  `TurnCompleted` detail.
- `context_usage::read_capture` keeps reading `context.json`. The provider
  half becomes `SqliteStore::input_turn_context(input) -> Vec<TurnContext>`,
  summed per step. `first` is the first turn's, `peak` the max, and counts
  and tokens are summed.
- A step recorded before this change has no `context` field. It reports the
  gap "provider context was not recorded for this step" instead of zeros. Old
  `events.jsonl` files are not read, migrated or deleted. Rotation by age
  removes them.

### Activity (slice 2)

- Draft migration `session_activity_cpu`
  (`uv run python scripts/new_migration.py session_activity_cpu`). It adds the
  nullable `cpu_sampled_at INTEGER` and `cpu_quiet_since INTEGER`.
  `store_revision_session_activity_update` enumerates its columns and does not
  list these, so 15 s samples never bump the `sessions` revision or wake
  Desktop. A test asserts that.
- The `Observer` stays in the launching process (`agent/mod.rs` headless loop,
  `SAMPLE_INTERVAL`). Each sample runs `UPDATE session_activity SET … WHERE
  session_id=? AND attachment_token=?` through the harness owner's
  attachment. It is update-only, so a Session that has not spoken yet has no
  row. The sampler never fabricates a provider reading, so
  `next_quiet_available` is unaffected.
- `activity::read(store, session)` becomes a synchronous store read with no
  re-sample:
  - **Running**: `observed_at` (last provider signal, ≤ 5 s lag) or a fresh
    `cpu_quiet_since` falls within `STALL_WINDOW` (300 s).
  - **Stalled**: the CPU sample is fresh (≤ `MAX_SAMPLE_GAP`, 45 s), and both
    times are older than 300 s.
  - **Running** also when there is no row yet and the step Process started
    under 300 s ago.
  - **Unknown** otherwise.
  `task_execution.rs` maps input to Session and stops passing `lf_home_dir()`.
  The PID-reuse and descendant guards in `Observer::observe` stay as they are.

## Delete — do not maintain

Nothing remains: every name the plan listed is gone (see State). Docs that
described the file (`docs/architecture/data.md`, TESTING.md's drain
paragraph, `FailureEvidence`'s comment) now describe the store.

**Ported, not deleted:**
- `scripts/context_ablation.py` reads its commands from
  `lf history show <session> --transcript --json` (`Turn.items`,
  `ConversationItem::Command.command`) and its final text from
  `lf history show <session> --input <input> --final`. Plain `--json` is
  the summary view, not the turns. Transcripts expire with the provider,
  so a census of old captures reports "transcript unavailable" for those
  steps instead of empty commands.
- `scripts/benchmarks/skill-invocation/request_mapping.py` drops its stream
  read (it only printed provider errors); stderr carries failures.
- `python/tests/test_resource_envelope.py`'s fixture file becomes
  `context.json`.

**Untouched:** `.lf/journal/traces/*/events.jsonl` (`journal/mod.rs`) and the
Swift and Python test-report `events.jsonl`. They are different files. Applied
migrations under `store/migrations/` are shipped records.

## Forbidden outcomes

- Any new per-request or per-tool row: context is one object on an existing
  per-turn row.
- A replacement raw file, an opt-in capture flag, or a "debug" mode without a
  named reader.
- Re-reading provider transcripts at query time for `--context`, since Claude
  expires them at 30 days and cost history must outlive that.
- CPU samples that bump store revisions, or a fabricated provider reading in
  `session_activity`.
- A reader that falls back to `events.jsonl` for old steps.

## Risks and findings

- **Silences ride on the capture clock.** LOO-611's `next_position` measures
  gaps between capture events, and most of those events are stream appends
  that are going away. The clock must keep ticking on every provider message
  the capture hears, or silences inflate. Covered by a test: a stream with a
  700 s gap yields exactly one silence fact.
- **Claude subagent traffic.** Today `parent_tool_use_id` lines are excluded
  from request and tool tallies. The decoder must keep that rule for
  `Request`, and the Display-based tool tally must not count subagent results.
- **Codex `peak` source.** Today it comes from `UsageCheckpoint.peak_input_tokens`,
  which the decoder already computes. `Request` must reproduce the same peak
  on `codex_normal_turn.jsonl`.
- **Older installed relays** keep writing `events.jsonl` until restarted.
  This is harmless: nothing reads the file and rotation deletes it.

## Done when

- A headless step on this branch leaves no `events.jsonl` in its capture
  directory, and nothing under `rust/loopflow/src` writes one. Stored facts
  keep the `events.jsonl:{seq}` source label (see State).
- `lf history usage --context` shows the same first, peak, tool output and
  compaction numbers as before for a recorded Claude trace and a Codex trace,
  read from the store.
- `lf task status` reports Running through a silent long tool and Stalled
  after the agent body stops, and Desktop's `sessions` revision does not
  advance on CPU samples.
- `lf history show --events` is rejected by Clap.

## Acceptance at gate (headless; inherited `LF_*` unset, `< /dev/null`)

- `cargo test -p loopflow --lib fact_writer context_usage session_record activity harness::`:
  - the Claude `claude_multi_tool.ndjson` trace and the Codex
    `codex_normal_turn.jsonl` trace yield `completed.context` equal to what
    today's `read_capture` computes from the same lines. Write the expected
    values down before deleting the old reader.
  - 700 s gap → one silence.
  - the Stalled/Running/Unknown table above.
  - a CPU-column update leaves `store_revisions.sessions` unchanged.
- `cargo test -p loopflow --test session_launch_contract headless_step_keeps_prompt_context`:
  a stand-in Claude step reports first 1000 / peak 4100 from the store and
  leaves no `events.jsonl`. `session_cli_tests` proves `--events` exits 2.
- `uv run pytest python/tests/test_context_ablation.py python/tests/test_resource_envelope.py`.
- `UPDATE_CLI_REFERENCE=1 cargo test -p loopflow --test integration reference_matches_the_compiled_cli`, then `git diff --exit-code docs/lf-reference.md` passes on the second run.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.

## Exclusions

- LOO-611's store and usage reshaping, and LOO-614's transcript reading and
  retention view.
- Process-row retention (LOO-602 decision 5).
- `lf disk` budget changes.
- Deleting existing `events.jsonl` files. Rotation removes them.

Transcripts on main leave `ConversationItem::Command.duration_ms` unset
for every provider, so `check_cost.py`'s command durations still have no
source. The questions entry stands.

Review (2026-10-26): a Codex `thread/resume` snapshot replays earlier turns'
completions; `FactWriter` skips an already retained completion and keeps the
live tally (`session_turn_completed`).

Check (compress, after the recorder cut): `cargo test --lib -- session_record fact_writer context_usage activity harness:: agent::` 264 passed; `--test session_launch_contract` 24 passed; clippy and fmt clean.

Concept review (2026-10-26, after the recorder cut): the implemented concepts
match the accepted design. Context is one object on each turn's `completed`
receipt, CPU liveness is two columns on `session_activity`, and
`activity::read` judges from that row alone. No unused concept or second
reader is left. The `events.jsonl:{seq}` label still names a file that is no
longer written. That is the recorded reversible choice (see questions), not an
open product question. Nothing to change.
