# Open questions and assumptions (e2cbe1)

- **Sequencing with #1813.** `lf server describe` and the `JsonSchema` derive
  on `uuid_id!` exist only on #1813 (open on 2026-11-06). Assumed: implement on
  main, then rebase onto a main containing #1813 before gate to regenerate
  `server_schema.json` and run `ServerProtocolTests`. Alternative: stack this
  Task on #1813's branch. Reversible.
- **Wire key removal.** Assumed acceptable: agent rows drop always-null
  `via_agent`/`caller_session_id`/`caller_agent_process_id`, lf rows drop
  always-null `agent_session_id`. Released Swift decodes them as optional.
- **`--parent <AgentProcess>`** filters to the lf processes that agent issued
  (`caller_agent_process_id`) instead of returning nothing. Assumed; small.
- **Explore (2026-11-06, headless).** `scratch/explore.html` shows three
  shapes: (A) field rename only, (B) tagged `ProcessEntry`, (C) separate agent
  inventory with a staged migration. Recommended B, the kickoff plan; it meets
  every Done-when line without a migration. Choice left open; implement
  proceeds with B. C not filed.
- **Swift label for agent rows.** `TaskWorkView` shows agent inventory rows as
  "Agent" beside "Process". Reversible copy choice.
