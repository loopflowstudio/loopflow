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
- Claude's cost is kept on the stream's `completed` row as `cost_usd`; its
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
