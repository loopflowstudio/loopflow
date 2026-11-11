# LOO-822 docs assumptions

- Live raw `claude`/`codex` conversations list in plain `lf session list` for
  their checkout, marked `raw`, without creating a Session.
- Raw rows carry no actions and empty `open_argv`; they join an unpaged list,
  and `--task` (their checkout's Task) and `--available` (turn not running),
  but not `--orphan`, `--blocked`, `--on`, `--search`, `--history`, `--page`.
- History's `Turn` is unchanged on the wire (it is in neither
  `server_schema.json` nor Swift; only `lf history show --transcript --json`
  emits it). Runs are a second list from the same reader, not fields on
  `Turn`.
- "Waiting on you" from a transcript uses the existing `availability:
  available` plus one new `turn.state`, `asking`; no new field.
- Transcript-sourced rows are dated when the run ended, not when written, and
  are left out of the "last recorded turn" a live reading prefers.
- A live completion counts for a transcript run when it carries the run's id,
  or was observed within 2 s after a run that began at or after its own turn.
- `lf` ancestors are recognized by name: `lf`, `lf-*`, `*-lf`. A provider
  started by another app (Paseo, Codex Desktop) lists as raw.
- The exit read writes rows for every provider lf launched, with no check
  that a Task or Wave claims the directory. Such a launch already has a
  Session and, with hooks, the same rows from the live stream; its Task is
  decided once, at launch (`counted_task`), so "only managed work" is a
  filter on the Session's Task when rows are read, and a gate at the write
  would make transcript rows disagree with live ones for the same launch.

Steers after kickoff, what remains and open choices: `loo-822.md`.

Sync resolution: retain these Task assumptions; use `docs/dev/server-first.md`
for the parent plan after its obsolete scratch plans were removed.
