## Try it!

Run a small headless probe, then inspect its trace:

```bash
env -u LF_RUN_ID -u LF_PROCESS_ID lf -b : "Reply with one sentence, then run pwd"
lf runs
lf trace <run-id>
lf trace <run-id> --events
lf context --days 14 --wave intelligence
lf doctor --json
```

`lf trace` shows separate launches and turns, exact prompt paths, supplied context, provider usage, capture state, and vendor receipt. `--events` reads the normalized user/assistant/tool exchange. `lf context --json` emits joinable turn, asset, and decision rows without opening prompt bodies.

On a release-profile 18,868-token probe, context gathering took 13 ms, exact attribution took 39 ms, and durable pre-launch persistence took 64 ms. Warm `lf context --json` queries over both 30 days and full local history took about 10 ms.

On the current long-lived ledger, doctor's new capture check is green (`9 launches, 9 turns, 117 assets`). The command still exits non-zero because it also exposes three historical lineage failures that predate this branch.

## Intent

Make context quality inspectable from Loopflow's own durable evidence. Every new provider launch records the exact prompts supplied, where each context byte came from, what Loopflow deliberately omitted or transformed, everything observable in the provider exchange, usage, lifecycle, and optional vendor-session identity. The record survives worktree deletion and does not depend on vendor transcript retention.

## Assumptions

- This is personal-machine, local-only evidence; capture may contain secrets or sensitive tool output.
- `run_events` remains the process lifecycle/spend ledger. Agent launches and turns add detail rather than replacing it.
- `cl100k_base` is Loopflow's stable supplied-context measure, not a claim about a provider's private tokenizer.
- Interactive TUI/IDE sessions become prompt-only once Loopflow hands control to the vendor.
- Existing historical rows are not backfilled. Coverage starts at the activation epoch.

## Key decisions

- Keep large bodies as private files below `~/.lf/traces`; keep searchable relationships and aggregates in SQLite.
- Fail before provider spend if core capture cannot be established. Mark mid-run loss partial and make doctor red without terminating the provider.
- Model traces, processes, launches, turns, assets, and decisions separately instead of overloading `process_id`.
- Store relative traversal-safe artifact paths and use a two-phase filesystem publish before the SQLite transaction.
- Share one exact token-accounting pass across totals, prefix attribution, and isolated asset counts; fall back to direct calculation if tokenizer-boundary validation ever disagrees.
- Repair the already-observed partial 062 migration forward in 063 instead of editing migration history.

## Not included

- Mac trace UI, historical import, vendor-directory copies, retention/compression/redaction, remote telemetry, or a results server.
- Synthetic evals or semantic judging of prompt quality.
- Historical lineage repair and the separate PM/Linear project retirement noted in `scratch/questions.md`.
