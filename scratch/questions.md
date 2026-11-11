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
  their checkout, marked `raw`, without creating a Session. Open: list them by
  default, or only behind a flag such as `--raw`?
- Session JSON gains `turn` (`state` running|ended|interrupted, `source`
  hooks|transcript, `started_at`, `ended_at`, `model`, `usage`) and `raw`.
  Shape written into `docs/surfaces.md`; change it there if implementation
  needs another.
- Raw rows carry no actions and empty `open_argv`; they join only an
  unfiltered, unpaged list (no `--task`, `--available`, `--orphan`,
  `--blocked`, `--on`, `--search`, `--history`, `--page`).
- `lf` ancestors are recognized by name: `lf`, `lf-*`, `*-lf`. A provider
  started by another app (Paseo, Codex Desktop) lists as raw.
- Turn usage sums the turn's model calls; `input_tokens` includes cached
  input for both providers (Claude's cache reads and writes are added in).
- Tail window starts at 128 KiB and grows fourfold to 8 MiB until it holds
  the turn's start. Reconcile 30 s; an event makes a reread due in 1 s.
- Changed docs: `docs/sessions.md#see-what-each-one-is-doing`,
  `docs/surfaces.md` (Session record), `docs/dev/server-first.md` (Telemetry,
  Codex question answered and removed from Open).

## Steers after kickoff (2026-11-10, open)

LOO-732: transcript reading "must keep producing those same per-turn facts in
the store, for bare and raw conversations too, since provider transcripts
expire." This branch writes no `session_events` rows yet. One live transcript
each (Claude 2.1.322, Codex 0.167.0):

- Claude: per-message `usage`, `message.model`, `stop_reason`,
  `isApiErrorMessage`, row `version`, `turn_duration.durationMs` (wall).
  No `cost_usd` or `duration_api_ms`: live `result` only.
- Codex: `token_count` totals, `token_usage_record`, `turn_context.model`,
  `task_complete.duration_ms`/`started_at`/`completed_at`, `turn_aborted`,
  `cli_version`. No cost, no API time.

LOO-819 plan (b1792e6e3): "Extend that reader; a second one beside it is the
wrong shape." Delete — do not maintain: `transcript/live.rs` `claude()` and
`codex()`. Their boundaries, model and usage move into `claude::turns` /
`codex::turns`. Open before cutting: history's `Turn` starts at every request,
injected ones included; a live turn is one agent run (Claude `end_turn`,
Codex `task_started`..`task_complete`). `Turn` is on the wire
(`server_schema.json`, Swift `RegistryQuery`). Also from that steer: the read
at a bare run's exit writes its usage/completed rows; "waiting on you" for
bare/raw comes from an unanswered question or hand-back in the transcript;
keep Task attribution in one place.
