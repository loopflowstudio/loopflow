# LOO-819 implementation assumptions

Explore is open; no vision has been selected. See `explore.html` and the
Explore section of `universal-server.md` for questions 1 and 2. The kickoff's
following choices are proposals:

- Keep the existing `lf server run` installation and LF-owned PTYs. Native Rex
  launch remains an investigation; no program/icon change is promised.
- Start with four projection workers, 256 queued keys and 128 client slots;
  measure them against the stated freshness and whole-process CPU targets.
- Keep thirty-second filesystem reconciliation and three-second degraded
  watching until measurements justify another value.
- Treat the CPU/latency numbers in `universal-server.md` as proposed completion
  targets, not measured improvements or quoted product requirements.

## LOO-822 docs assumptions

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

Steers after kickoff, what remains and open choices: `loo-822.md`.
