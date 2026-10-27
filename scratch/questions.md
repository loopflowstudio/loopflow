# LOO-613 open questions and assumptions

- **check_cost.py deleted (assumption, reversible).** Its command time by skill
  came from `events.jsonl` item timestamps. Neither the store nor LOO-614's
  `Turn` carries per-item start and end times. Its context half duplicates
  `lf history usage --context`. If command time matters, it needs a named
  fact. That would be a new Task, not a resurrected stream.
- **Context lives on the `completed` receipt, not in a new kind.** This keeps
  "one row per turn" and needs no schema change. A turn that never completes
  (crash) loses its tally. Today that turn's partial stream would still have
  been readable. Accepted as truthful absence.
- **The stall reader drops its own CPU re-sample** and trusts the sampler's
  ≤45 s freshness. Detection can now lag by up to one sample interval (15 s)
  compared with today.
- **Claude subagent lines are no longer displayed as main-conversation items**
  (assumption, reversible). The Claude decoder emitted `ItemStarted`/
  `ItemCompleted` and text for subagent (`parent_tool_use_id`) lines, so the
  Display-based tool tally would have counted them. The decoder now skips
  display and `Request` for those lines; activity signals and usage receipts
  still include them.
- **Edits count as tool results with no output tokens.** Claude Write/Edit
  results decode as `File` items, which carry no output. The old reader counted
  their raw text; the new tally counts the result but not its few tokens. The
  old reader also double-counted Claude Bash output (raw line plus normalized
  `item_completed`); the new numbers do not.
- **OpenCode reports no `Request`.** Its decoder replays message snapshots on
  every poll, so per-message input would leak into later turns' `first`. Its
  steps keep the existing "no per-request input" gap.
- **One context tally per FactWriter**, taken by the next `TurnCompleted`
  (attributed or not), rather than per native thread. A writer serves one
  Session's stream.
- **Account selection has no CLI reader after `--events`** (assumption,
  reversible). `docs/subscriptions.md` used `lf history show --events` to show
  `provider_account_selected`. The rows stay in the store (`input_events`,
  Graph routing, retention). If people need to see them, the summary view can
  print the selected account per attempt; that is a separate Task.
- **`context_ablation.py` reads the whole Session's transcript** for a step's
  commands (assumption, reversible). Transcript turns carry no input key;
  headless steps run one input per Session, so this matches today's cohort.
- **Proposed memory update:** `.lf/wave/intelligence/MEMORY.md:17` says
  `scripts/check_cost.py` collects check costs; it no longer exists. Left for a
  Graph that maintains that Wave's memory.
- **Stored facts keep the `events.jsonl:{seq}` source label** (assumption,
  reversible). Receipt keys, indexes, released migrations and #1698's draft
  match on `:events.jsonl:`; renaming it needs a data migration for no reader
  benefit. The file itself is no longer written.
- **`StreamFormat` deleted with `Raw`.** No CLI flag selected `Raw`, and no
  code read `Human`'s color flag, so the enum carried no behavior.
