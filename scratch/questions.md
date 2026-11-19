# Open choices (LOO-986 kickoff, headless)

- Tool directories are **appended** to `PATH` in lf's `main`, not prepended as
  `supervised_path` and Swift do. Appending never shadows a person's own `PATH`
  order; prepending would let `/opt/homebrew/bin` win over `/usr/bin` under
  launchd. Reversible in one line.
- The fix lives in lf's `main` for every lf process, not only where a provider's
  `Command` is built (the brief's idea), so `gh`, `git` and the holder resolve too.
- `lf open` passes `--agent` only from the invocation's `-a` or
  `LF_AGENT_OVERRIDE`. An agent caller started with `LF_AGENT_OVERRIDE` would
  still move the conversation; new Sessions already clear it
  (`journal/request.rs` `PROCESS_CONTEXT_ENV`), so this should not arise.
