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

Steers after kickoff, what remains and open choices: `loo-822.md`.
