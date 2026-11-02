# Open questions (LOO-708)

- `lf session resume ID --json` printed the Session record after the
  conversation ended. Once the viewer and launcher exec, nothing remains to
  print it; `--prepare --json` (what Desktop uses) is unchanged. Assumed
  acceptable; flag if a script relied on the post-exit JSON.
- The exec'd launcher's ledger row stays open while the provider runs and the
  owner completes it from the provider's exit (0 succeeded, 130 interrupted,
  else failed); the provider's own exit lands on the AgentProcess row and the
  attachment_exit event, as before. Assumed acceptable.
- A provider that exits before publishing history used to fail `lf session
  connect` with "exited … before becoming resumable" from the connect's poll
  loop. The connect now execs into the launcher, so the launcher's own error
  (for OpenCode, "private server failed to start") is what the caller sees.
  Assumed acceptable; the lifecycle test asserts the new wording.
- The owner runs `PlanningSync` for the launch's repository once it holds the
  exit record, as the launcher did while it waited. Its `git` children are
  transient and never `lf`.
