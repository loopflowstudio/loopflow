# LOO-822 docs assumptions

- Live raw `claude`, `codex`, `pi` and `opencode` conversations list in plain
  `lf session list` for their checkout, marked `raw`, without creating a
  Session.
- Raw rows carry no actions and empty `open_argv`; they join an unpaged list,
  and `--task` (their checkout's Task) and `--available` (turn not running),
  but not `--orphan`, `--blocked`, `--on`, `--search`, `--history`, `--page`.
- History's `Turn` is in neither `server_schema.json` nor Swift; only
  `lf history show --transcript --json` emits it. Its shape changed once on
  2026-11-11: a turn has `id`, `state`, `started_at`, `ended_at`, `model`,
  `usage` and `items` (`{"request":…}` or `{"agent":…}`).
- A Session with no status report from its agent process is `available`
  when its turn ended or was interrupted; "has a status report" is a
  `session_activity` row for its current agent process.
- Exit `completed` rows are dated when the turn ended, not when written.
- A live completion counts for a transcript turn at exit when it carries the
  turn's id, or was observed within 2 s after a turn that began at or after
  its own turn. The live line matches by id only.
- Claude's cost is kept on the stream's `completed` row as `total_cost_usd`
  (the process's running total, Claude's name for it); its
  usage rows have none. Open for a person: keep it there, or leave it
  unrecorded. Pi and OpenCode usage rows carry the turn's `cost_usd` from
  their transcripts.
- OpenCode is read from its SQLite store, not `opencode session export`;
  open whether reading its internal schema is acceptable long-term.
- A request that woke nothing (a local command's output, a notification no
  model call followed) is an item of the turn before it.
- `lf` ancestors are recognized by name: `lf`, `lf-*`, `*-lf`. A provider
  started by another app (Paseo, Codex Desktop) lists as raw.
- The exit read writes rows for every provider lf launched, with no check
  that a Task or Wave claims the directory. Such a launch already has a
  Session; its Task is decided once, at launch (`counted_task`), so "only
  managed work" is a filter on the Session's Task when rows are read.

Steers after kickoff, what remains and open choices: `loo-822.md`.

Sync resolution: retain these Task assumptions; use `docs/dev/server-first.md`
for the parent plan after its obsolete scratch plans were removed.

- Session-list baseline, 2026-11-11: measure the existing branch without
  implementing an optimization. Use release `lf`, warm filesystem caches,
  the current checkout's default repository scope, and JSON for machine
  validation. Provider transcripts remain in their normal homes, read-only
  to the listing; the benchmark provider owns its normal transcript writes.

- Session-list optimization plan, 2026-11-11: no final latency target was
  supplied. Use 15% lower default CLI median and 25% lower served median as
  provisional targets for the indexed-Task-lookup experiment, not a promise
  or permission to change outputs. Keep the smaller query fixes separately
  measurable; leave the unreproduced 75 s stall open.

- Optimization measurement, 2026-11-11: freeze wall-clock time with a private
  DYLD interposer, leaving monotonic clocks alone, so board timestamps and
  availability deadlines participate in full-JSON comparisons. Use the same
  executable/home paths for all sides, a separate migrated copy per commit,
  and a discarded warm-up before each timed CLI or served checkpoint. Copy
  provider transcript directories and the OpenCode database without credentials;
  ambient OS processes remain live and are a stated limit on attribution.

- Process-inventory measurement, iteration 2: fresh read-only online backup of
  the Machine store; every measured command uses `/tmp/loo822-process-measure`.
  Keep iteration 1’s private provider transcript copies and unchanged timing
  harness; OS/Git facts remain ambient. Test a non-overlapping local planning
  latency improvement, with full JSON unchanged, without inventing a target.
