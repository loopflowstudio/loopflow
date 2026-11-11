# LOO-824 open choices

Exploration choices (1b, 2a, 3a, 4a) are answered; the quotes and the
direction built from them are in `holder.md`. Options as offered:
`a18d6a915:scratch/questions.md`.

## Assumptions, reversible

- Desktop Task shells stay on their own `session serve` owner until LOO-830.
- No snapshot on graceful stop: rebuild is ring replay plus a nudge;
  terminal-mode recovery after a gap is unverified (slice 0).
- Ring 2 MiB per PTY; holder exits 60 s after its last child.
- Store names are `held_terminals` / `held_terminal_id`: `processes.terminal_id`
  already holds Desktop pane placement. The docs say "terminal" throughout.
- Plain CLI views and the held child's control requests reach the registry
  through `held.sock` directly; the `session.attach` ticket bridge serves
  clients that speak only the server protocol (Desktop).
- `bridged` and the attach ticket carry both `owner_lf_process_id` and
  `terminal_id`, one of them null, instead of two shapes under one name.
- A lost `session.start` holder reply answers `outcome_unknown`; the caller
  repeats the same `request`.
- `session.attach` for a pending holder's terminal answers `unavailable` with
  `retry: true`: the holder is a dependency not answering, not the server at
  capacity. No client reads `retry` yet.
- A replaced view (another `held.sock` revision) does not carry `--moved` or
  `--session-json` into the installed `lf`.
- Nothing yet lists which conversations a killed holder took.
