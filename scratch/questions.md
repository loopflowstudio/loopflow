# LOO-572 assumptions

Decisions made headless; each is reversible.

- **Both usage sources stay, one row per turn each.** Recorder envelopes carry
  what native rows lack: the attempt (and so its account), model, final
  receipt, and they exist for every captured run. Native rows are the only
  usage for interactive Sessions with no capture. Dropping either loses
  measured turns, so each collapses instead: envelopes to one row per usage
  stream at its first event's position (`events.jsonl:<first seq>`), native
  receipts to one row per turn (OpenCode: per assistant message), keyed by
  turn instead of a payload hash.
- **Row identity, found on the clone.** A recorder row is one provider's
  stream within one attempt's turn: Codex continues a `usage_stream_id` into
  the next turn, so keying by stream alone moved usage between turns (+841k
  tokens in one MTG-97 step). A native receipt that arrives after the
  thread's next turn started keeps its own row (`:after:<start seq>`): the
  next turn's baseline reads the previous turn's usage from before it began
  (7 turns in 4 Sessions on Jack's store).
- **Merged rows keep what the reducer read from every row.** Envelopes take
  each counter's maximum (the reducer already reduced a stream that way).
  Codex native rows keep the first receipt's baseline (`total - last`), each
  total's maximum, the largest request as `peakInputTokens`, and `regressed`
  when a total ever fell (counted as an evidence gap). Lost: per-observation
  gap counts inside a recorder stream (a falling per-request peak used to
  count a gap; it was never missing evidence).
- **Compaction is in the retention prune, not a migration.** A `.usage-cursor`
  beside the database means later passes read only new rows. No schema change.
- **Dead Processes follow `settle_agent_process`:** `completed_at` set, outcome
  NULL (exit not observed). `completed_at` is the boot time when the Process
  began before it, otherwise when the prune found it dead. A PID no process
  holds proves exit even without a recorded start time; `lf` rows now record
  `os_started_at` so later checks can tell a reused PID.
- **Unrecorded commands are `Cli::records_process`** (next to
  `may_launch_agent` in `lf/mod.rs`): `__context-block`, hooks an earlier lf
  left (`__planning-check`, `__session-title`), `task file|files|diff`, and the
  reads that were already effect-free (`--context`, `--explain`, Task location
  and file watching, Wave show/workflow reads, Desktop view and layout reads).
  `__transport-serve` stays recorded: Sessions reference it. The prune applies
  the same predicate to stored argv, at any age, to finished unreferenced rows.

## Clone probe (2026-10-24, `cp -c` of Jack's store, same snapshot for both)

LOO-570 prune only: 546 MB, `session_events` 341,852 rows (378 MB with indexes), `processes` 85,477 rows (74 MB), 3,892 open.
This branch: 235 MB, `session_events` 101,353 rows (117 MB), usage rows 11,042 (from ~248k), `processes` 30,929 (34 MB); every open row has a live PID.
`lf history usage --weekly --json`: all five weeks byte-identical to the base. Release prune first pass 357–390 s (1,327 s under load); next pass 27 ms.
Remaining `session_events` bulk: `provider_output` rows of captures still inside LOO-570's 24 h settle window, and retained turn facts. The 50 MB target is not met.
Checks: fmt and clippy `--all-targets -D warnings` clean; lib suites session_events, session_record, retention, journal, context_cost, disk, revisions, harness pass; integration global_commands and integration (context_block, task_diff, history, preview, process, wave_show) pass.
